use CCA::CCAInstance;
use ValidationHook::ValidationHookInstance;
use alloy::{
    primitives::{U256, address},
    providers::{Provider, ProviderBuilder, WsConnect},
    sol,
};
use eyre::Result;
use futures_util::StreamExt;
use std::str::FromStr;

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
    dotenvy::dotenv()?;

    let rpc_url = dotenvy::var("ETH_RPC_URL")?;
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let _bid = U256::from_str(&dotenvy::var("MAX_BID_PRICE")?)?;

    let _cca = CCAInstance::new(
        address!("0x608c4e792C65f5527B3f70715deA44d3b302F4Ee"),
        &provider,
    );

    let _validation_hook = ValidationHookInstance::new(
        address!("0x2DD6e0E331DE9743635590F6c8BC5038374CAc9D"),
        &provider,
    );

    let sub = provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    while let Some(header) = stream.next().await {
        println!("Latest block number: {}", header.number);
    }

    Ok(())
}
