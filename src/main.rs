use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use eyre::Result;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;

    let rpc_url = dotenvy::var("ETH_RPC_URL")?;
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let sub = provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    while let Some(header) = stream.next().await {
        println!("Latest block number: {}", header.number);
    }

    Ok(())
}
