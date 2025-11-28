use crate::{
    auction::Auction,
    bids::preprocess_bids,
    blocks::{BlockConsumer, BlockProducer, Completion},
    config::Config,
    logging::log_summary,
    registry::BidRegistry,
    validate::PreflightValidator,
};
use alloy::{
    primitives::{Address, address},
    providers::ProviderBuilder,
};
use eyre::Result;
use futures_util::StreamExt;
use tracing::{error, info};

const CCA_ADDRESS: Address = address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee");
const HOOK_ADDRESS: Address = address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D");
const SOULBOUND_ADDRESS: Address = address!("0xBf3CF56c587F5e833337200536A52E171EF29A09");

pub async fn run_bot(config: Config) -> Result<()> {
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
    let params = auction.load_params(config.signer.address()).await?;

    PreflightValidator::new(&params, &config.bids).run()?;

    let planned_bids = preprocess_bids(&config.bids, &params);

    let registry = BidRegistry::new(
        auction,
        params,
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
