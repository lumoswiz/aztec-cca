use alloy::{
    primitives::{Address, U128, U256},
    signers::local::PrivateKeySigner,
};
use eyre::{Result, WrapErr, eyre};
use std::{env::VarError, str::FromStr};

#[derive(Debug)]
pub struct BidParams {
    pub max_bid: U256,
    pub amount: U128,
    pub owner: Address,
}

#[derive(Debug)]
pub struct Config {
    pub rpc_url: String,
    pub signer: PrivateKeySigner,
    pub bid_params: BidParams,
}

impl BidParams {
    pub fn from_env_with_owner(owner: Address) -> Result<Self> {
        let max_bid_str = dotenvy::var("MAX_BID_PRICE")
            .wrap_err("missing MAX_BID_PRICE (max bid price, in wei)")?;
        let max_bid: alloy::primitives::Uint<256, 4> = U256::from_str(&max_bid_str)
            .map_err(|_| eyre!("MAX_BID_PRICE is not a valid U256: {max_bid_str}"))?;

        let amount_str =
            dotenvy::var("BID_AMOUNT").wrap_err("missing BID_AMOUNT (bid amount, in wei)")?;
        let amount = U128::from_str(&amount_str)
            .map_err(|_| eyre!("BID_AMOUNT is not a valid U128: {amount_str}"))?;

        Ok(Self {
            max_bid,
            amount,
            owner,
        })
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let rpc_url =
            dotenvy::var("ETH_RPC_URL").wrap_err("missing ETH_RPC_URL (Ethereum RPC URL)")?;

        let pk = dotenvy::var("PRIVATE_KEY")
            .wrap_err("missing PRIVATE_KEY (hex private key, 0x-prefixed)")?;
        let signer = PrivateKeySigner::from_str(&pk)
            .map_err(|_| eyre!("PRIVATE_KEY is not a valid hex private key"))?;

        let owner = match dotenvy::var("OWNER") {
            Ok(s) => Address::parse_checksummed(&s, None)
                .map_err(|_| eyre!("OWNER is not a valid checksummed address: {s}"))?,
            Err(dotenvy::Error::EnvVar(VarError::NotPresent)) => signer.address(),
            Err(e) => return Err(e).wrap_err("failed to read OWNER"),
        };

        let bid_params = BidParams::from_env_with_owner(owner)?;

        Ok(Self {
            rpc_url,
            bid_params,
            signer,
        })
    }
}
