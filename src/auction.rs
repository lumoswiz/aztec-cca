use crate::{
    CCA::CCAInstance, Soulbound::SoulboundInstance, ValidationHook::ValidationHookInstance,
    config::BidParams,
};
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use eyre::{Result, eyre};

#[derive(Debug, Clone)]
pub struct Auction<P>
where
    P: Provider + Clone,
{
    pub provider: P,
    pub cca: CCAInstance<P>,
    pub validation_hook: ValidationHookInstance<P>,
    pub soulbound: SoulboundInstance<P>,
}

impl<P> Auction<P>
where
    P: Provider + Clone,
{
    pub fn new(
        provider: P,
        cca_addr: Address,
        hook_addr: Address,
        soulbound_addr: Address,
    ) -> Self {
        let cca = CCAInstance::new(cca_addr, provider.clone());
        let validation_hook = ValidationHookInstance::new(hook_addr, provider.clone());
        let soulbound = SoulboundInstance::new(soulbound_addr, provider.clone());

        Self {
            provider,
            cca,
            validation_hook,
            soulbound,
        }
    }

    pub async fn load_params(&self, signer_address: Address) -> Result<AuctionParams> {
        let multicall = self
            .provider
            .multicall()
            .add(self.validation_hook.CONTRIBUTOR_PERIOD_END_BLOCK())
            .add(self.validation_hook.MAX_PURCHASE_LIMIT())
            .add(self.cca.floorPrice())
            .add(self.cca.tickSpacing())
            .add(self.cca.MAX_BID_PRICE())
            .add(self.cca.endBlock())
            .add(self.validation_hook.totalPurchased(signer_address))
            .add(self.soulbound.hasAnyToken(signer_address));

        let (
            contributor_period_end_block,
            max_purchase_limit,
            floor_price,
            tick_spacing,
            max_bid_price,
            end_block_raw,
            total_purchased,
            has_any_token,
        ) = multicall.aggregate().await?;

        let end_block = U256::from(end_block_raw);

        Ok(AuctionParams {
            contributor_period_end_block,
            max_purchase_limit,
            floor_price,
            tick_spacing,
            max_bid_price,
            end_block,
            total_purchased,
            has_any_token,
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
        let prev_tick_price = self.compute_prev_tick_price(params, cfg.max_bid).await?;
        Ok(SubmitBidParams {
            max_price: cfg.max_bid,
            amount: cfg.amount,
            owner: resolved_owner,
            prev_tick_price,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AuctionParams {
    pub contributor_period_end_block: U256,
    pub max_purchase_limit: U256,
    pub floor_price: U256,
    pub tick_spacing: U256,
    pub max_bid_price: U256,
    pub end_block: U256,
    pub total_purchased: U256,
    pub has_any_token: bool,
}

#[derive(Debug)]
pub struct SubmitBidParams {
    pub max_price: U256,
    pub amount: u128,
    pub owner: Address,
    pub prev_tick_price: U256,
}
