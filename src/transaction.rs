use crate::{CCA, auction::SubmitBidParams};
use alloy::{
    network::TransactionBuilder,
    primitives::{Address, Bytes, U256},
    providers::Provider,
    rpc::types::{eth::TransactionRequest, transaction::AccessList},
    signers::local::PrivateKeySigner,
    sol_types::SolCall,
};
use eyre::Result;

#[derive(Debug, Clone)]
pub struct FeeOverrides {
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub enum AccessListConfig {
    #[default]
    None,
    Provided(AccessList),
    Generate,
}

#[derive(Debug, Clone, Default)]
pub struct TxConfig {
    pub fees: Option<FeeOverrides>,
    pub access_list: AccessListConfig,
}

#[allow(dead_code)]
impl TxConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fee_overrides(
        mut self,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Self {
        self.fees = Some(FeeOverrides {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        });
        self
    }

    pub fn with_access_list(mut self, config: AccessListConfig) -> Self {
        self.access_list = config;
        self
    }

    pub fn generate_access_list(self) -> Self {
        self.with_access_list(AccessListConfig::Generate)
    }

    pub fn provided_access_list(mut self, list: AccessList) -> Self {
        self.access_list = AccessListConfig::Provided(list);
        self
    }
}

pub struct TxBuilder<P>
where
    P: Provider + Clone,
{
    provider: P,
    signer: PrivateKeySigner,
    cca: Address,
    config: Option<TxConfig>,
}

impl<P> TxBuilder<P>
where
    P: Provider + Clone,
{
    pub fn new(
        provider: P,
        signer: PrivateKeySigner,
        cca: Address,
        config: Option<TxConfig>,
    ) -> Self {
        Self {
            provider,
            signer,
            cca,
            config,
        }
    }

    pub async fn build_submit_bid_request(
        &self,
        bid: &SubmitBidParams,
    ) -> Result<TransactionRequest> {
        let calldata = self.bid_calldata(bid);
        let value = U256::from(bid.amount);
        let tx = self.build_base_request(calldata, value);
        self.apply_config(tx).await
    }

    fn bid_calldata(&self, bid: &SubmitBidParams) -> Bytes {
        Bytes::from(
            CCA::submitBid_1Call {
                maxPrice: bid.max_price,
                amount: bid.amount,
                owner: bid.owner,
                prevTickPrice: bid.prev_tick_price,
                hookData: Bytes::new(),
            }
            .abi_encode(),
        )
    }

    fn build_base_request(&self, calldata: Bytes, value: U256) -> TransactionRequest {
        TransactionRequest::default()
            .with_from(self.signer.address())
            .with_to(self.cca)
            .with_input(calldata)
            .with_value(value)
    }

    async fn apply_config(&self, tx: TransactionRequest) -> Result<TransactionRequest> {
        let Some(cfg) = &self.config else {
            return Ok(tx);
        };

        let tx = if let Some(fees) = &cfg.fees {
            tx.with_max_fee_per_gas(fees.max_fee_per_gas)
                .with_max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
        } else {
            tx
        };

        let tx = match &cfg.access_list {
            AccessListConfig::None => tx,
            AccessListConfig::Provided(list) => tx.with_access_list(list.clone()),
            AccessListConfig::Generate => self.generate_access_list(tx).await?,
        };

        Ok(tx)
    }

    async fn generate_access_list(&self, tx: TransactionRequest) -> Result<TransactionRequest> {
        let res = self.provider.create_access_list(&tx).await?;
        Ok(tx.with_access_list(res.access_list))
    }
}
