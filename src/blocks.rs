use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use alloy::{
    network::BlockResponse, providers::Provider, rpc::client::BuiltInConnectionString,
    rpc::types::eth::Header,
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

pub struct BlockConsumer;

impl BlockConsumer {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle_block(&self, header: &Header) -> Result<()> {
        println!("Received block number: {}", header.number);
        Ok(())
    }
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
        sleep(Duration::from_millis(200)).await;
    }
    sleep(Duration::from_millis(200)).await;
    Ok(())
}
