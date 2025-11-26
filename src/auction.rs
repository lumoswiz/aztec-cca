use crate::{
    CCA::{self, CCAInstance},
    ValidationHook::ValidationHookInstance,
    config::BidParams,
};
use alloy::{
    primitives::{Address, Bytes, U256},
    rpc::types::TransactionRequest,
};
use alloy::{providers::Provider, sol_types::SolCall};
use eyre::{Result, eyre};

#[derive(Debug)]
pub struct Auction<P>
where
    P: Provider + Clone,
{
    pub provider: P,
    pub cca: CCAInstance<P>,
    pub validation_hook: ValidationHookInstance<P>,
}

impl<P> Auction<P>
where
    P: Provider + Clone,
{
    pub fn new(provider: P, cca_addr: Address, hook_addr: Address) -> Self {
        let cca = CCAInstance::new(cca_addr, provider.clone());
        let validation_hook = ValidationHookInstance::new(hook_addr, provider.clone());

        Self {
            provider,
            cca,
            validation_hook,
        }
    }

    pub async fn load_params(&self) -> Result<AuctionParams> {
        let multicall = self
            .provider
            .multicall()
            .add(self.validation_hook.CONTRIBUTOR_PERIOD_END_BLOCK())
            .add(self.validation_hook.MAX_PURCHASE_LIMIT())
            .add(self.cca.floorPrice())
            .add(self.cca.tickSpacing());

        let (contributor_period_end_block, max_purchase_limit, floor_price, tick_spacing) =
            multicall.aggregate().await?;

        Ok(AuctionParams {
            contributor_period_end_block,
            max_purchase_limit,
            floor_price,
            tick_spacing,
        })
    }

    pub async fn compute_prev_tick_price(
        &self,
        params: &AuctionParams,
        bid_price: U256,
    ) -> Result<U256> {
        let floor_price = params.floor_price;

        if bid_price < floor_price {
            return Err(eyre!(
                "bid price {} is below floor price {}",
                bid_price,
                floor_price
            ));
        }

        let mut prev = floor_price;
        let mut tick = self.cca.ticks(prev).call().await?;
        let mut next = tick.next;

        while next < bid_price {
            prev = next;
            tick = self.cca.ticks(prev).call().await?;
            next = tick.next;
        }

        Ok(prev)
    }

    pub async fn prepare_submit_bid(
        &self,
        cfg: &BidParams,
        params: &AuctionParams,
        resolved_owner: Address,
    ) -> Result<SubmitBidParams> {
        params.ensure_tick_aligned(cfg.max_bid)?;
        let prev_tick_price = self.compute_prev_tick_price(params, cfg.max_bid).await?;
        Ok(SubmitBidParams {
            max_price: cfg.max_bid,
            amount: cfg.amount,
            owner: resolved_owner,
            prev_tick_price,
        })
    }

    pub fn build_submit_bid_calldata(&self, submit: &SubmitBidParams) -> Vec<u8> {
        CCA::submitBid_1Call {
            maxPrice: submit.max_price,
            amount: submit.amount,
            owner: submit.owner,
            prevTickPrice: submit.prev_tick_price,
            hookData: Bytes::new(),
        }
        .abi_encode()
    }
}

#[derive(Debug)]
pub struct AuctionParams {
    pub contributor_period_end_block: U256,
    pub max_purchase_limit: U256,
    pub floor_price: U256,
    pub tick_spacing: U256,
}

impl AuctionParams {
    pub fn ensure_tick_aligned(&self, bid_price: U256) -> Result<()> {
        if bid_price % self.tick_spacing != U256::from(0) {
            return Err(eyre!(
                "bid price {} is not aligned with tick spacing {}",
                bid_price,
                self.tick_spacing
            ));
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SubmitBidParams {
    pub max_price: U256,
    pub amount: u128,
    pub owner: Address,
    pub prev_tick_price: U256,
}
