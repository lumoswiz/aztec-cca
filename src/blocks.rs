use crate::{
    auction::{Auction, AuctionParams, SubmitBidParams},
    config::BidParams,
    registry::{BidRegistry, BidSummary, RetryStatus, TrackedBid},
    transaction::{TxBuilder, TxConfig},
};
use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use alloy::{
    network::BlockResponse,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::{
        client::BuiltInConnectionString,
        types::{TransactionRequest, eth::Header},
    },
    signers::local::PrivateKeySigner,
};
use eyre::{Result, eyre};
use futures_util::{Stream, StreamExt, stream::BoxStream};
use serde::Serialize;
use tokio::time::sleep;
use tracing::{error, info, info_span, instrument, warn};

pub struct BlockProducer<P>
where
    P: Provider + Clone + Unpin,
{
    stream: BoxStream<'static, Result<Header>>,
    _marker: PhantomData<P>,
}

impl<P> BlockProducer<P>
where
    P: Provider + Clone + Send + Sync + Unpin + 'static,
{
    pub async fn new(provider: P, endpoint: &BuiltInConnectionString) -> Result<Self> {
        let stream = match endpoint {
            BuiltInConnectionString::Ws(_, _) | BuiltInConnectionString::Ipc(_) => {
                let sub = provider.subscribe_blocks().await?;
                sub.into_stream().map(|header| Ok(header)).boxed()
            }
            BuiltInConnectionString::Http(_) => {
                align_polling(&provider).await?;
                let mut watcher = provider.watch_full_blocks().await?;
                watcher.set_poll_interval(Duration::from_secs(12));
                watcher
                    .into_stream()
                    .map(|res| match res {
                        Ok(block) => Ok(block.header().clone()),
                        Err(err) => Err(eyre!(err)),
                    })
                    .boxed()
            }
            _ => {
                return Err(eyre!(
                    "unsupported transport for block production, enable HTTP, WS, or IPC"
                ));
            }
        };

        Ok(Self {
            stream,
            _marker: PhantomData,
        })
    }
}

impl<P> Stream for BlockProducer<P>
where
    P: Provider + Clone + Unpin,
{
    type Item = Result<Header>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        Pin::new(&mut this.stream).poll_next(cx)
    }
}

#[derive(Debug)]
pub struct BidContext<P>
where
    P: Provider + Clone,
{
    auction: Auction<P>,
    params: AuctionParams,
    bid_params: BidParams,
    signer: PrivateKeySigner,
    tx_config: Option<TxConfig>,
    cca_addr: Address,
}

impl<P> BidContext<P>
where
    P: Provider + Clone,
{
    pub fn new(
        auction: Auction<P>,
        params: AuctionParams,
        bid_params: BidParams,
        signer: PrivateKeySigner,
        tx_config: Option<TxConfig>,
        cca_addr: Address,
    ) -> Self {
        Self {
            auction,
            params,
            bid_params,
            signer,
            tx_config,
            cca_addr,
        }
    }

    pub async fn prepare_submit_bid(&self) -> Result<SubmitBidParams> {
        self.auction
            .prepare_submit_bid(&self.bid_params, &self.params, self.bid_params.owner)
            .await
    }

    pub async fn build_transaction(&self, submit: &SubmitBidParams) -> Result<TransactionRequest> {
        let builder = TxBuilder::new(
            self.auction.provider.clone(),
            self.signer.clone(),
            self.cca_addr,
            self.tx_config.clone(),
        );
        builder.build_submit_bid_request(submit).await
    }

    pub async fn simulate_transaction(&self, tx: &TransactionRequest) -> Result<()> {
        self.auction.provider.call(tx.clone()).await?;
        Ok(())
    }

    pub async fn send_transaction(&self, tx: TransactionRequest) -> Result<B256> {
        let pending = self.auction.provider.send_transaction(tx).await?;
        let receipt = pending.get_receipt().await?;
        info!(tx = ?receipt.transaction_hash, "bid submitted");
        Ok(receipt.transaction_hash)
    }
}

pub struct BlockConsumer<P>
where
    P: Provider + Clone,
{
    registry: BidRegistry<P>,
}

impl<P> BlockConsumer<P>
where
    P: Provider + Clone,
{
    pub fn new(registry: BidRegistry<P>) -> Self {
        Self { registry }
    }

    #[instrument(skip_all, fields(block = header.number))]
    pub async fn handle_block(&mut self, header: &Header) -> Result<Completion> {
        let window = self.registry.window();
        let block_number = U256::from(header.number);

        if block_number < window.contributor_period_end_block {
            info!(
                end_block = %window.contributor_period_end_block,
                "contributor track active"
            );
            return Ok(Completion::Pending);
        }

        if block_number >= window.end_block {
            let summary = self.registry.summary();
            let pending = summary.pending;
            if pending > 0 {
                warn!(
                    pending,
                    block = header.number,
                    "auction ended with pending bids"
                );
            } else {
                info!(block = header.number, "auction ended");
            }
            return Ok(Completion::Finished {
                summary,
                reason: if pending == 0 {
                    ShutdownReason::AllBidsProcessed
                } else {
                    ShutdownReason::AuctionEndedWithPending
                },
            });
        }

        for tracked in self.registry.bids_mut().iter_mut() {
            if !tracked.is_pending() {
                continue;
            }

            info!(
                owner = ?tracked.bid_params().owner,
                amount = tracked.bid_params().amount,
                attempt = tracked.attempts() + 1,
                max_retries = tracked.max_retries(),
                "submitting bid"
            );

            match submit_bid(tracked).await {
                Ok(tx_hash) => tracked.mark_submitted(tx_hash),
                Err(err) => match tracked.record_failure(format!("{err:?}")) {
                    RetryStatus::Retrying(attempts) => warn!(
                        owner = ?tracked.bid_params().owner,
                        attempts,
                        max_retries = tracked.max_retries(),
                        error = ?err,
                        "bid retry scheduled"
                    ),
                    RetryStatus::Exhausted => error!(
                        owner = ?tracked.bid_params().owner,
                        attempts = tracked.attempts(),
                        max_retries = tracked.max_retries(),
                        error = ?err,
                        "bid failed permanently"
                    ),
                },
            }
        }

        if self.registry.all_done() {
            let summary = self.registry.summary();
            Ok(Completion::Finished {
                summary,
                reason: ShutdownReason::AllBidsProcessed,
            })
        } else {
            Ok(Completion::Pending)
        }
    }
}

#[derive(Debug)]
pub enum Completion {
    Pending,
    Finished {
        summary: BidSummary,
        reason: ShutdownReason,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum ShutdownReason {
    AllBidsProcessed,
    AuctionEndedWithPending,
}

async fn submit_bid<P>(tracked: &mut TrackedBid<P>) -> Result<B256>
where
    P: Provider + Clone,
{
    let span = info_span!(
        "bid",
        owner = ?tracked.bid_params().owner,
        amount = tracked.bid_params().amount,
        attempt = tracked.attempts() + 1
    );
    let _enter = span.enter();

    let context = tracked.context_mut();
    let submit_bid_params = context.prepare_submit_bid().await?;
    info!("prepared submit params");
    let tx_request = context.build_transaction(&submit_bid_params).await?;
    info!("built transaction request");
    context.simulate_transaction(&tx_request).await?;
    info!("simulation succeeded");
    context.send_transaction(tx_request).await
}

async fn align_polling<P>(provider: &P) -> Result<()>
where
    P: Provider,
{
    let start = provider.get_block_number().await?;
    loop {
        let current = provider.get_block_number().await?;
        if current > start {
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }
    sleep(Duration::from_millis(250)).await;
    Ok(())
}
