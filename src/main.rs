mod auction;
mod blocks;
mod config;
mod registry;
mod transaction;
mod validate;

use crate::{
    auction::Auction,
    blocks::{BlockConsumer, BlockProducer, Completion},
    config::Config,
    registry::{BidOutcomeState, BidRegistry, BidSummary, PlannedBid},
    validate::PreflightValidator,
};
use alloy::{
    primitives::{Address, address},
    providers::ProviderBuilder,
    sol,
};
use eyre::Result;
use futures_util::StreamExt;

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
    let config = Config::from_env()?;

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

    let planned_bids: Vec<PlannedBid> = config.bids.iter().cloned().map(PlannedBid::new).collect();

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
                Completion::Finished(summary) => {
                    log_summary(&summary);
                    break;
                }
            },
            Err(err) => {
                eprintln!("block stream terminated: {err:?}");
                break;
            }
        }
    }

    Ok(())
}

fn log_summary(summary: &BidSummary) {
    println!(
        "Bid summary -> submitted: {}, failed: {}, pending: {}",
        summary.submitted, summary.failed, summary.pending
    );

    for outcome in &summary.outcomes {
        match &outcome.state {
            BidOutcomeState::Submitted { tx_hash } => println!(
                "  owner {:?} amount {} submitted (tx {:?})",
                outcome.owner, outcome.amount, tx_hash
            ),
            BidOutcomeState::Failed { error } => println!(
                "  owner {:?} amount {} failed: {}",
                outcome.owner, outcome.amount, error
            ),
            BidOutcomeState::Pending {
                attempts,
                max_retries,
                last_error,
            } => println!(
                "  owner {:?} amount {} pending ({}/{} attempts, last_error={:?})",
                outcome.owner, outcome.amount, attempts, max_retries, last_error
            ),
        }
    }
}
