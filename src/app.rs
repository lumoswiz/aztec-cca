use crate::{
    auction::Auction,
    bids::preprocess_bids,
    blocks::{BlockConsumer, BlockProducer, Completion, ShutdownReason},
    config::Config,
    logging::{log_summary, persist_summary},
    registry::{BidRegistry, BidSummary},
    validate::PreflightValidator,
};
use alloy::{
    primitives::{Address, address},
    providers::Provider,
};
use eyre::Result;
use futures_util::StreamExt;
use tracing::{error, info, instrument, warn};

const CCA_ADDRESS: Address = address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee");
const HOOK_ADDRESS: Address = address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D");
const SOULBOUND_ADDRESS: Address = address!("0xBf3CF56c587F5e833337200536A52E171EF29A09");

pub struct AuctionBot<P>
where
    P: Provider + Clone + Unpin,
{
    block_producer: BlockProducer<P>,
    block_consumer: BlockConsumer<P>,
}

impl<P> AuctionBot<P>
where
    P: Provider + Clone + Send + Sync + Unpin + 'static,
{
    pub async fn build_with_provider(provider: P, config: Config) -> Result<Self> {
        info!(bids = config.bids.len(), "configuration loaded");

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

        let block_producer = BlockProducer::new(provider.clone(), &config.transport).await?;
        let block_consumer = BlockConsumer::new(registry);

        Ok(Self {
            block_producer,
            block_consumer,
        })
    }

    #[instrument(skip_all)]
    pub async fn run(mut self) -> Result<()> {
        loop {
            match self.block_producer.next().await {
                Some(Ok(header)) => match self.block_consumer.handle_block(&header).await? {
                    Completion::Pending => {}
                    Completion::Finished { summary, reason } => {
                        self.record_summary(Some(summary), reason);
                        break;
                    }
                },
                Some(Err(err)) => {
                    error!(?err, "block stream terminated");
                    let reason = if self.block_consumer.has_pending_bids() {
                        ShutdownReason::BlockStreamErrorWithPending
                    } else {
                        ShutdownReason::BlockStreamError
                    };
                    self.record_summary(None, reason);
                    break;
                }
                None => {
                    warn!("block stream ended unexpectedly");
                    let reason = if self.block_consumer.has_pending_bids() {
                        ShutdownReason::BlockStreamEndedWithPending
                    } else {
                        ShutdownReason::BlockStreamEnded
                    };
                    self.record_summary(None, reason);
                    break;
                }
            }
        }
        Ok(())
    }

    fn record_summary(&mut self, summary: Option<BidSummary>, reason: ShutdownReason) {
        let summary = summary.unwrap_or_else(|| self.block_consumer.summary());
        log_summary(&summary, &reason);
        match persist_summary(&summary, &reason) {
            Ok(path) => info!(file = %path.display(), "bid summary persisted"),
            Err(err) => warn!(?err, "failed to persist bid summary"),
        }
    }
}
