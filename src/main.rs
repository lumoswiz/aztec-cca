mod auction;
mod blocks;
mod config;
mod transaction;
mod validate;

use crate::{
    auction::Auction,
    blocks::{BidContext, BlockConsumer, BlockProducer},
    config::Config,
    validate::PreflightValidator,
};
use alloy::{primitives::address, providers::ProviderBuilder, sol};
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

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    let provider = ProviderBuilder::new()
        .connect_with(&config.transport)
        .await?;

    let cca_addr = address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee");
    let hook_addr = address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D");
    let soulbound_addr = address!("0xBf3CF56c587F5e833337200536A52E171EF29A09");

    let auction = Auction::new(provider.clone(), cca_addr, hook_addr, soulbound_addr);
    let signer_address = config.signer.address();
    let params = auction.load_params(signer_address).await?;
    PreflightValidator::new(&params, &config.bid_params).run()?;

    let bid_context = BidContext::new(
        auction,
        params,
        config.bid_params.clone(),
        config.signer.clone(),
        None,
        cca_addr,
    );

    let mut block_producer = BlockProducer::new(provider.clone(), &config.transport).await?;
    let mut block_consumer = BlockConsumer::new(bid_context);

    while let Some(result) = block_producer.next().await {
        match result {
            Ok(header) => block_consumer.handle_block(&header).await?,
            Err(err) => {
                eprintln!("block stream terminated: {err:?}");
                break;
            }
        }
    }

    Ok(())
}
