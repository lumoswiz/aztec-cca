mod app;
mod auction;
mod bids;
mod blocks;
mod config;
mod logging;
mod registry;
mod transaction;
mod validate;

use crate::{app::AuctionBot, config::Config, logging::init_logging};
use alloy::{providers::ProviderBuilder, sol};
use eyre::Result;

sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    contract CCA {
        struct Tick {
            uint256 next;
            uint256 currencyDemandQ96;
        }

        function floorPrice() external view returns (uint256);
        function tickSpacing() external view returns (uint256);
        function MAX_BID_PRICE() external view returns (uint256);
        function endBlock() external view returns (uint64);
        function ticks(uint256 price) external view returns (Tick memory tick);
        function submitBid(
            uint256 maxPrice,
            uint128 amount,
            address owner,
            bytes hookData
        ) external payable returns (uint256);
        function submitBid(
            uint256 maxPrice,
            uint128 amount,
            address owner,
            uint256 prevTickPrice,
            bytes hookData
        ) external payable returns (uint256);
    }
}

sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    contract ValidationHook {
        function CONTRIBUTOR_PERIOD_END_BLOCK() external view returns (uint256);
        function MAX_PURCHASE_LIMIT() external view returns (uint256);
        function totalPurchased(address sender)
            external
            view
            returns (uint256 totalPurchased);
    }
}

sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    contract Soulbound {
        function hasAnyToken(address _addr) external view returns (bool);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    let config = Config::from_env()?;
    let provider = ProviderBuilder::new()
        .wallet(config.signer.clone())
        .connect_with(&config.transport)
        .await?;
    AuctionBot::build_with_provider(provider, config)
        .await?
        .run()
        .await
}
