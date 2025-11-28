use crate::{auction::AuctionParams, config::BidParams};
use alloy::primitives::U256;
use eyre::{Result, eyre};

pub struct PreflightValidator<'a> {
    params: &'a AuctionParams,
    bids: &'a [BidParams],
}

impl<'a> PreflightValidator<'a> {
    pub fn new(params: &'a AuctionParams, bids: &'a [BidParams]) -> Self {
        Self { params, bids }
    }

    pub fn run(&self) -> Result<()> {
        for (idx, bid) in self.bids.iter().enumerate() {
            self.ensure_amount_positive(idx, bid)?;
            self.ensure_max_price_within_bounds(idx, bid)?;
            self.ensure_tick_alignment(idx, bid)?;
        }
        self.ensure_within_purchase_limit()?;
        // self.ensure_has_soulbound_token()?;
        Ok(())
    }

    fn ensure_amount_positive(&self, idx: usize, bid: &BidParams) -> Result<()> {
        if bid.amount == 0 {
            let bid_no = idx + 1;
            return Err(eyre!(
                "bid #{bid_no} (owner {}) amount must be greater than zero",
                bid.owner
            ));
        }
        Ok(())
    }

    fn ensure_max_price_within_bounds(&self, idx: usize, bid: &BidParams) -> Result<()> {
        if bid.max_bid > self.params.max_bid_price {
            let bid_no = idx + 1;
            return Err(eyre!(
                "bid #{bid_no} (owner {}) MAX_BID_PRICE ({}) exceeds cap ({})",
                bid.owner,
                bid.max_bid,
                self.params.max_bid_price
            ));
        }
        Ok(())
    }

    fn ensure_tick_alignment(&self, idx: usize, bid: &BidParams) -> Result<()> {
        let bid_no = idx + 1;
        self.params.ensure_tick_aligned(bid.max_bid).map_err(|err| {
            eyre!(
                "bid #{bid_no} (owner {}) not tick-aligned: {err}",
                bid.owner
            )
        })
    }

    fn ensure_within_purchase_limit(&self) -> Result<()> {
        let mut running_total = self.params.total_purchased;

        for (idx, bid) in self.bids.iter().enumerate() {
            running_total += U256::from(bid.amount);
            if running_total > self.params.max_purchase_limit {
                let bid_no = idx + 1;
                return Err(eyre!(
                    "bids exceed allocation: bid #{bid_no} (owner {}) pushes total {} over cap {}",
                    bid.owner,
                    running_total,
                    self.params.max_purchase_limit
                ));
            }
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
