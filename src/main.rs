mod auction;
mod blocks;
mod config;
mod registry;
mod ticks;
mod transaction;
mod validate;

use crate::{
    auction::Auction,
    blocks::{BlockConsumer, BlockProducer, Completion, ShutdownReason},
    config::Config,
    registry::{BidOutcomeState, BidRegistry, BidSummary, PlannedBid},
    ticks::align_price_to_tick,
    validate::PreflightValidator,
};
use alloy::{
    primitives::{Address, address},
    providers::ProviderBuilder,
    sol,
};
use eyre::Result;
use futures_util::StreamExt;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt};

sol!(
    #[sol(rpc)]
    #[derive(Debug)]
    CCA,
    "abi/cca.json"
);

sol!(
    #[sol(rpc)]
    #[derive(Debug)]
    ValidationHook,
    "abi/validation_hook.json"
);

sol!(
    #[sol(rpc)]
    #[derive(Debug)]
    Soulbound,
    "abi/soulbound.json"
);

const CCA_ADDRESS: Address = address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee");
const HOOK_ADDRESS: Address = address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D");
const SOULBOUND_ADDRESS: Address = address!("0xBf3CF56c587F5e833337200536A52E171EF29A09");

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("aztec_cca=info".parse()?))
        .init();

    let config = Config::from_env()?;
    info!(bids = config.bids.len(), "Configuration loaded");

    let provider = ProviderBuilder::new()
        .connect_with(&config.transport)
        .await?;

    let auction = Auction::new(
        provider.clone(),
        CCA_ADDRESS,
        HOOK_ADDRESS,
        SOULBOUND_ADDRESS,
    );
    let signer_address = config.signer.address();
    let params = auction.load_params(signer_address).await?;

    PreflightValidator::new(&params, &config.bids).run()?;

    let planned_bids: Vec<PlannedBid> = config
        .bids
        .iter()
        .cloned()
        .map(|mut bid| {
            let aligned = align_price_to_tick(bid.max_bid, &params);
            if aligned != bid.max_bid {
                warn!(
                    owner = ?bid.owner,
                    original = %bid.max_bid,
                    adjusted = %aligned,
                    "max bid adjusted to nearest tick"
                );
                bid.max_bid = aligned;
            }
            PlannedBid::new(bid)
        })
        .collect();

    let registry = BidRegistry::new(
        auction,
        params.clone(),
        planned_bids,
        config.signer.clone(),
        CCA_ADDRESS,
    )?;

    let mut block_producer = BlockProducer::new(provider.clone(), &config.transport).await?;
    let mut block_consumer = BlockConsumer::new(registry);

    while let Some(result) = block_producer.next().await {
        match result {
            Ok(header) => match block_consumer.handle_block(&header).await? {
                Completion::Pending => {}
                Completion::Finished { summary, reason } => {
                    log_summary(&summary, &reason);
                    break;
                }
            },
            Err(err) => {
                error!(?err, "block stream terminated");
                break;
            }
        }
    }

    Ok(())
}

fn log_summary(summary: &BidSummary, reason: &ShutdownReason) {
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
            BidOutcomeState::Failed { error } => error!(
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
