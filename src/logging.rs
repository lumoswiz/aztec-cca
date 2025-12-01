use crate::{
    blocks::ShutdownReason,
    registry::{BidOutcomeState, BidSummary},
};
use eyre::{Result, WrapErr};
use serde::Serialize;
use std::{
    fs::File,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{error, info, warn};
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
        ShutdownReason::BlockStreamError => error!(
            submitted = summary.submitted,
            failed = summary.failed,
            pending = summary.pending,
            "bid summary (block stream error)"
        ),
        ShutdownReason::BlockStreamErrorWithPending => error!(
            submitted = summary.submitted,
            failed = summary.failed,
            pending = summary.pending,
            "bid summary (block stream error with pending bids)"
        ),
        ShutdownReason::BlockStreamEnded => warn!(
            submitted = summary.submitted,
            failed = summary.failed,
            pending = summary.pending,
            "bid summary (block stream ended)"
        ),
        ShutdownReason::BlockStreamEndedWithPending => warn!(
            submitted = summary.submitted,
            failed = summary.failed,
            pending = summary.pending,
            "bid summary (block stream ended with pending bids)"
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

#[derive(Serialize)]
struct PersistedSummary {
    reason: ShutdownReason,
    summary: BidSummary,
}

pub fn persist_summary(summary: &BidSummary, reason: &ShutdownReason) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before UNIX_EPOCH")?
        .as_secs();
    let path = PathBuf::from(format!("cca-summary-{timestamp}.json"));
    let mut file = File::create(&path).wrap_err("failed to create summary file")?;
    let payload = PersistedSummary {
        reason: reason.clone(),
        summary: summary.clone(),
    };
    serde_json::to_writer_pretty(&mut file, &payload).wrap_err("failed to write summary file")?;
    Ok(path)
}
