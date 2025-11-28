use crate::{
    auction::{Auction, AuctionParams},
    blocks::BidContext,
    config::BidParams,
    transaction::TxConfig,
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
        bids: Vec<PlannedBid>,
        signer: PrivateKeySigner,
        cca_addr: Address,
    ) -> Result<Self> {
        let window = AuctionWindow {
            contributor_period_end_block: params.contributor_period_end_block,
            end_block: params.end_block,
        };

        let tracked = bids
            .into_iter()
            .map(|planned| {
                let bid_params = planned.params;
                let context = BidContext::new(
                    auction.clone(),
                    params.clone(),
                    bid_params.clone(),
                    signer.clone(),
                    planned.tx_config,
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

#[derive(Debug, Clone)]
pub struct PlannedBid {
    pub params: BidParams,
    pub tx_config: Option<TxConfig>,
}

impl PlannedBid {
    pub fn new(params: BidParams) -> Self {
        Self {
            params,
            tx_config: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_tx_config(mut self, tx_config: TxConfig) -> Self {
        self.tx_config = Some(tx_config);
        self
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
