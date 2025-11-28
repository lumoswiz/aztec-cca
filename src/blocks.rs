use crate::{
    auction::{Auction, AuctionParams, SubmitBidParams},
    config::BidParams,
    registry::{BidRegistry, BidState, TrackedBid},
    transaction::{TxBuilder, TxConfig},
};
use std::{
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
use tokio::time::sleep;

pub struct BlockProducer {
    stream: BoxStream<'static, Result<Header>>,
}

impl BlockProducer {
    pub async fn new<P>(provider: P, endpoint: &BuiltInConnectionString) -> Result<Self>
    where
        P: Provider + Clone + Send + Sync + 'static,
    {
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

        Ok(Self { stream })
    }
}

impl Stream for BlockProducer {
    type Item = Result<Header>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
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
        println!(
            "Bid submitted in transaction: {:?}",
            receipt.transaction_hash
        );
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

    pub async fn handle_block(&mut self, header: &Header) -> Result<()> {
        let window = self.registry.window();
        let block_number = U256::from(header.number);

        if block_number < window.contributor_period_end_block {
            println!(
                "Contributor track active (current block {}, public bidding opens at {})",
                header.number, window.contributor_period_end_block
            );
            return Ok(());
        }

        if block_number >= window.end_block {
            println!(
                "Auction has ended (current block {}, end block {})",
                header.number, window.end_block
            );
            return Ok(());
        }

        for tracked in self.registry.bids_mut().iter_mut() {
            if !matches!(tracked.state(), BidState::Pending) {
                continue;
            }

            println!(
                "Submitting bid for owner {:?} with amount {}",
                tracked.bid_params().owner,
                tracked.bid_params().amount
            );

            match submit_bid(tracked).await {
                Ok(tx_hash) => tracked.mark_submitted(tx_hash),
                Err(err) => {
                    eprintln!("bid failed: {err:?}");
                    tracked.mark_failed(err.to_string());
                }
            }
        }

        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.registry.all_submitted()
    }
}

async fn submit_bid<P>(tracked: &mut TrackedBid<P>) -> Result<B256>
where
    P: Provider + Clone,
{
    let context = tracked.context_mut();
    let submit_bid_params = context.prepare_submit_bid().await?;
    let tx_request = context.build_transaction(&submit_bid_params).await?;
    context.simulate_transaction(&tx_request).await?;
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
