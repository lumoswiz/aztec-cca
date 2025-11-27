use alloy::{
    primitives::{Address, U256},
    rpc::client::BuiltInConnectionString,
    signers::local::PrivateKeySigner,
};
use eyre::{Result, WrapErr, eyre};
use std::{env::VarError, str::FromStr};

#[derive(Debug)]
pub struct BidParams {
    pub max_bid: U256,
    pub amount: u128,
    pub owner: Address,
}

#[derive(Debug)]
pub struct Config {
    pub transport: BuiltInConnectionString,
    pub signer: PrivateKeySigner,
    pub bid_params: BidParams,
}

impl BidParams {
    pub fn from_env_with_owner(owner: Address) -> Result<Self> {
        let max_bid_str = dotenvy::var("MAX_BID_PRICE").wrap_err("missing MAX_BID_PRICE (wei)")?;
        let max_bid: alloy::primitives::Uint<256, 4> = U256::from_str(&max_bid_str)
            .map_err(|_| eyre!("MAX_BID_PRICE is not a valid U256: {max_bid_str}"))?;

        let amount_str = dotenvy::var("BID_AMOUNT").wrap_err("missing BID_AMOUNT (wei)")?;
        let amount = u128::from_str(&amount_str)
            .map_err(|_| eyre!("BID_AMOUNT is not a valid u128: {amount_str}"))?;

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

        let transport = provider_transport_from_env()?;

        let pk = dotenvy::var("PRIVATE_KEY").wrap_err("missing PRIVATE_KEY")?;
        let signer = PrivateKeySigner::from_str(&pk)
            .map_err(|_| eyre!("PRIVATE_KEY is not a valid private key"))?;

        let owner = match dotenvy::var("OWNER") {
            Ok(s) => Address::parse_checksummed(&s, None)
                .map_err(|_| eyre!("OWNER is not a valid checksummed address: {s}"))?,
            Err(dotenvy::Error::EnvVar(VarError::NotPresent)) => signer.address(),
            Err(e) => return Err(e).wrap_err("failed to read OWNER"),
        };

        let bid_params = BidParams::from_env_with_owner(owner)?;

        Ok(Self {
            transport,
            bid_params,
            signer,
        })
    }
}

fn provider_transport_from_env() -> Result<BuiltInConnectionString> {
    let raw =
        dotenvy::var("RPC_ENDPOINT").wrap_err("missing RPC_ENDPOINT (HTTP/WS URL or IPC path)")?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(eyre!(
            "RPC_ENDPOINT cannot be empty: provide an HTTP/WS  URL or IPC path"
        ));
    }

    trimmed
        .parse::<BuiltInConnectionString>()
        .map_err(|err| eyre!("invalid RPC_ENDPOINT: {err}"))
}
