mod auction;
mod config;
mod transaction;

use crate::{auction::Auction, config::Config};
use alloy::{
    primitives::address,
    providers::{Provider, ProviderBuilder, WsConnect},
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

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    let ws = WsConnect::new(&config.rpc_url).with_max_retries(20);
    let provider = ProviderBuilder::new()
        .wallet(config.signer.clone())
        .connect_ws(ws)
        .await?;

    let cca_addr = address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee");
    let hook_addr = address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D");

    let auction = Auction::new(provider.clone(), cca_addr, hook_addr);
    let params = auction.load_params().await?;
    let submit_params = auction
        .prepare_submit_bid(&config.bid_params, &params, config.bid_params.owner)
        .await?;

    let sub = provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    while let Some(header) = stream.next().await {
        println!("Latest block number: {}", header.number);
    }

    Ok(())
}
