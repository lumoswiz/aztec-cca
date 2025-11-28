use crate::{auction::AuctionParams, config::BidParams};
use alloy::primitives::U256;
use eyre::{Result, eyre};

pub struct PreflightValidator<'a> {
    params: &'a AuctionParams,
    bid: &'a BidParams,
}

impl<'a> PreflightValidator<'a> {
    pub fn new(params: &'a AuctionParams, bid: &'a BidParams) -> Self {
        Self { params, bid }
    }

    pub fn run(&self) -> Result<()> {
        self.ensure_amount_positive()?;
        self.ensure_max_price_within_bounds()?;
        self.ensure_within_purchase_limit()?;
        // self.ensure_has_soulbound_token()?;
        self.params.ensure_tick_aligned(self.bid.max_bid)?;
        Ok(())
    }

    fn ensure_amount_positive(&self) -> Result<()> {
        if self.bid.amount == 0 {
            return Err(eyre!("BID_AMOUNT must be greater than zero"));
        }
        Ok(())
    }

    fn ensure_max_price_within_bounds(&self) -> Result<()> {
        if self.bid.max_bid > self.params.max_bid_price {
            return Err(eyre!(
                "MAX_BID_PRICE ({}) exceeds auction cap ({})",
                self.bid.max_bid,
                self.params.max_bid_price
            ));
        }
        Ok(())
    }

    fn ensure_within_purchase_limit(&self) -> Result<()> {
        let total_after_bid = self.params.total_purchased + U256::from(self.bid.amount);
        if total_after_bid > self.params.max_purchase_limit {
            return Err(eyre!(
                "bid amount ({}) exceeds remaining allocation (purchased {}, cap {})",
                self.bid.amount,
                self.params.total_purchased,
                self.params.max_purchase_limit
            ));
        }
        Ok(())
    }

    fn ensure_has_soulbound_token(&self) -> Result<()> {
        if !self.params.has_any_token {
            return Err(eyre!("sender ineligible: missing required soulbound token"));
        }
        Ok(())
    }
}
