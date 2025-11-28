use crate::{
    auction::{Auction, AuctionParams},
    blocks::BidContext,
    config::BidParams,
    transaction::TxConfig,
    validate::PreflightValidator,
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::Provider,
    signers::local::PrivateKeySigner,
};
use eyre::Result;

#[derive(Debug)]
pub struct BidRegistry<P>
where
    P: Provider + Clone,
{
    bids: Vec<TrackedBid<P>>,
    window: AuctionWindow,
}

impl<P> BidRegistry<P>
where
    P: Provider + Clone,
{
    pub fn new(
        auction: Auction<P>,
        params: AuctionParams,
        bids: Vec<BidParams>,
        signer: PrivateKeySigner,
        tx_config: Option<TxConfig>,
        cca_addr: Address,
    ) -> Result<Self> {
        for bid in &bids {
            PreflightValidator::new(&params, bid).run()?;
        }

        let window = AuctionWindow {
            contributor_period_end_block: params.contributor_period_end_block,
            end_block: params.end_block,
        };

        let tracked = bids
            .into_iter()
            .map(|bid_params| {
                let context = BidContext::new(
                    auction.clone(),
                    params.clone(),
                    bid_params.clone(),
                    signer.clone(),
                    tx_config.clone(),
                    cca_addr,
                );
                TrackedBid {
                    bid_params,
                    context,
                    state: BidState::Pending,
                }
            })
            .collect();

        Ok(Self {
            bids: tracked,
            window,
        })
    }

    pub fn window(&self) -> &AuctionWindow {
        &self.window
    }

    pub fn bids_mut(&mut self) -> &mut [TrackedBid<P>] {
        &mut self.bids
    }

    pub fn all_submitted(&self) -> bool {
        self.bids
            .iter()
            .all(|bid| matches!(bid.state, BidState::Submitted { .. }))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuctionWindow {
    pub contributor_period_end_block: U256,
    pub end_block: U256,
}

#[derive(Debug)]
pub struct TrackedBid<P>
where
    P: Provider + Clone,
{
    bid_params: BidParams,
    context: BidContext<P>,
    state: BidState,
}

impl<P> TrackedBid<P>
where
    P: Provider + Clone,
{
    pub fn state(&self) -> &BidState {
        &self.state
    }

    pub fn bid_params(&self) -> &BidParams {
        &self.bid_params
    }

    pub fn context_mut(&mut self) -> &mut BidContext<P> {
        &mut self.context
    }

    pub fn mark_submitted(&mut self, tx_hash: B256) {
        self.state = BidState::Submitted { tx_hash };
    }

    pub fn mark_failed(&mut self, error: String) {
        self.state = BidState::Failed { error };
    }
}

#[derive(Debug)]
pub enum BidState {
    Pending,
    Submitted { tx_hash: B256 },
    Failed { error: String },
}
