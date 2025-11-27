mod auction;
mod config;
mod transaction;

use crate::{auction::Auction, config::Config, transaction::TxBuilder};
use alloy::{primitives::address, providers::ProviderBuilder, sol};
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

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    let provider = ProviderBuilder::new()
        .connect_with(&config.transport)
        .await?;

    let cca_addr = address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee");
    let hook_addr = address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D");

    let auction = Auction::new(provider.clone(), cca_addr, hook_addr);
    let params = auction.load_params().await?;
    let submit_bid_params = auction
        .prepare_submit_bid(&config.bid_params, &params, config.bid_params.owner)
        .await?;
    let _submit_tx = TxBuilder::new(provider.clone(), config.signer, cca_addr, None)
        .build_submit_bid_request(&submit_bid_params)
        .await?;

    //let sub = provider.subscribe_blocks().await?;
    //let mut stream = sub.into_stream();
    //
    //while let Some(header) = stream.next().await {
    //println!("Latest block number: {}", header.number);
    //}

    Ok(())
}
