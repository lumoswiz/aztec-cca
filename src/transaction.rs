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
pub struct TxConfig {
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub access_list: Option<AccessList>,
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
        Ok(self.build_request(calldata, value))
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

    fn build_request(&self, calldata: Bytes, value: U256) -> TransactionRequest {
        let tx = TransactionRequest::default()
            .with_to(self.cca)
            .with_value(value)
            .with_input(calldata);

        if let Some(cfg) = &self.config {
            let tx = tx
                .with_max_fee_per_gas(cfg.max_fee_per_gas)
                .with_max_priority_fee_per_gas(cfg.max_priority_fee_per_gas);
            if let Some(access_list) = &cfg.access_list {
                return tx.with_access_list(access_list.clone());
            }
            return tx;
        }

        tx
    }
}
