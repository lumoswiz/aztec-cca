use crate::{
    blocks::ShutdownReason,
    registry::{BidOutcomeState, BidSummary},
};
use eyre::Result;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt};

pub fn init_logging() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("aztec_cca=info".parse()?))
        .init();
    Ok(())
}

pub fn log_summary(summary: &BidSummary, reason: &ShutdownReason) {
    match reason {
        ShutdownReason::AllBidsProcessed => info!(
            submitted = summary.submitted,
            failed = summary.failed,
            pending = summary.pending,
            "bid summary"
        ),
        ShutdownReason::AuctionEndedWithPending => warn!(
            submitted = summary.submitted,
            failed = summary.failed,
            pending = summary.pending,
            "bid summary (auction ended early)"
        ),
    }

    for outcome in &summary.outcomes {
        match &outcome.state {
            BidOutcomeState::Submitted { tx_hash } => info!(
                owner = ?outcome.owner,
                amount = outcome.amount,
                tx_hash = ?tx_hash,
                "bid submitted"
            ),
            BidOutcomeState::Failed { error } => warn!(
                owner = ?outcome.owner,
                amount = outcome.amount,
                error,
                "bid failed"
            ),
            BidOutcomeState::Pending {
                attempts,
                max_retries,
                last_error,
            } => info!(
                owner = ?outcome.owner,
                amount = outcome.amount,
                attempts,
                max_retries,
                last_error = ?last_error,
                "bid pending"
            ),
        }
    }
}
