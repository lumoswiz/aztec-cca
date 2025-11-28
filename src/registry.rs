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

const DEFAULT_MAX_RETRIES: u8 = 3;

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
                let PlannedBid {
                    params: bid_params,
                    tx_config,
                    max_retries,
                } = planned;
                let context = BidContext::new(
                    auction.clone(),
                    params.clone(),
                    bid_params.clone(),
                    signer.clone(),
                    tx_config,
                    cca_addr,
                );
                TrackedBid {
                    bid_params,
                    context,
                    state: BidState::Pending,
                    attempts: 0,
                    max_retries,
                    last_error: None,
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

    pub fn all_done(&self) -> bool {
        self.bids.iter().all(|bid| bid.is_complete())
    }

    pub fn summary(&self) -> BidSummary {
        let mut submitted = 0;
        let mut failed = 0;
        let mut pending = 0;

        let outcomes = self
            .bids
            .iter()
            .map(|bid| {
                let state = match &bid.state {
                    BidState::Pending => {
                        pending += 1;
                        BidOutcomeState::Pending {
                            attempts: bid.attempts,
                            max_retries: bid.max_retries,
                            last_error: bid.last_error.clone(),
                        }
                    }
                    BidState::Submitted { tx_hash } => {
                        submitted += 1;
                        BidOutcomeState::Submitted { tx_hash: *tx_hash }
                    }
                    BidState::Failed { error } => {
                        failed += 1;
                        BidOutcomeState::Failed {
                            error: error.clone(),
                        }
                    }
                };

                BidOutcome {
                    owner: bid.bid_params.owner,
                    amount: bid.bid_params.amount,
                    state,
                }
            })
            .collect();

        BidSummary {
            submitted,
            failed,
            pending,
            outcomes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlannedBid {
    pub params: BidParams,
    pub tx_config: Option<TxConfig>,
    pub max_retries: u8,
}

impl PlannedBid {
    pub fn new(params: BidParams) -> Self {
        Self {
            params,
            tx_config: None,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }

    #[allow(dead_code)]
    pub fn with_tx_config(mut self, tx_config: TxConfig) -> Self {
        self.tx_config = Some(tx_config);
        self
    }

    #[allow(dead_code)]
    pub fn with_max_retries(mut self, max_retries: u8) -> Self {
        self.max_retries = max_retries;
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
    attempts: u8,
    max_retries: u8,
    last_error: Option<String>,
}

impl<P> TrackedBid<P>
where
    P: Provider + Clone,
{
    pub fn bid_params(&self) -> &BidParams {
        &self.bid_params
    }

    pub fn attempts(&self) -> u8 {
        self.attempts
    }

    pub fn max_retries(&self) -> u8 {
        self.max_retries
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, BidState::Pending)
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self.state,
            BidState::Submitted { .. } | BidState::Failed { .. }
        )
    }

    pub fn context_mut(&mut self) -> &mut BidContext<P> {
        &mut self.context
    }

    pub fn mark_submitted(&mut self, tx_hash: B256) {
        self.state = BidState::Submitted { tx_hash };
        self.last_error = None;
    }

    pub fn record_failure(&mut self, error: String) -> RetryStatus {
        self.attempts = self.attempts.saturating_add(1);
        self.last_error = Some(error.clone());
        if self.attempts >= self.max_retries {
            self.state = BidState::Failed { error };
            RetryStatus::Exhausted
        } else {
            RetryStatus::Retrying(self.attempts)
        }
    }
}

#[derive(Debug)]
pub enum BidState {
    Pending,
    Submitted { tx_hash: B256 },
    Failed { error: String },
}

#[derive(Debug)]
pub enum RetryStatus {
    Retrying(u8),
    Exhausted,
}

#[derive(Debug, Clone)]
pub struct BidSummary {
    pub submitted: usize,
    pub failed: usize,
    pub pending: usize,
    pub outcomes: Vec<BidOutcome>,
}

#[derive(Debug, Clone)]
pub struct BidOutcome {
    pub owner: Address,
    pub amount: u128,
    pub state: BidOutcomeState,
}

#[derive(Debug, Clone)]
pub enum BidOutcomeState {
    Pending {
        attempts: u8,
        max_retries: u8,
        last_error: Option<String>,
    },
    Submitted {
        tx_hash: B256,
    },
    Failed {
        error: String,
    },
}
