mod app;
mod auction;
mod bids;
mod blocks;
mod config;
mod logging;
mod registry;
mod transaction;
mod validate;

use crate::{app::AuctionBot, config::Config, logging::init_logging};
use alloy::{providers::ProviderBuilder, sol};
use eyre::Result;

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

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    let config = Config::from_env()?;
    let provider = ProviderBuilder::new()
        .connect_with(&config.transport)
        .await?;
    AuctionBot::build_with_provider(provider, config)
        .await?
        .run()
        .await
}
