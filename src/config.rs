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
        let max_bid = parse_env("MAX_BID_PRICE", "max bid price (wei)", |value| {
            U256::from_str(value).map_err(|_| eyre!("MAX_BID_PRICE is not a valid U256: {value}"))
        })?;

        let amount = parse_env("BID_AMOUNT", "bid amount (wei)", |value| {
            u128::from_str(value).map_err(|_| eyre!("BID_AMOUNT is not a valid u128: {value}"))
        })?;

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

        let signer = parse_env("PRIVATE_KEY", "hex private key", |value| {
            PrivateKeySigner::from_str(value)
                .map_err(|_| eyre!("PRIVATE_KEY is not a valid private key"))
        })?;

        let owner = match optional_env("OWNER", |value| {
            Address::parse_checksummed(value, None)
                .map_err(|_| eyre!("OWNER is not a valid checksummed address: {value}"))
        })? {
            Some(address) => address,
            None => signer.address(),
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
    parse_env("RPC_ENDPOINT", "HTTP/WS URL or IPC path", |value| {
        value
            .parse::<BuiltInConnectionString>()
            .map_err(|err| eyre!(err))
    })
}

fn parse_env<T, F>(key: &str, desc: &str, parser: F) -> Result<T>
where
    F: FnOnce(&str) -> Result<T>,
{
    let raw = dotenvy::var(key).wrap_err(format!("missing {key} ({desc})"))?;
    let value = raw.trim();
    if value.is_empty() {
        return Err(eyre!("{key} cannot be empty ({desc})"));
    }

    parser(value)
}

fn optional_env<T, F>(key: &str, parser: F) -> Result<Option<T>>
where
    F: FnOnce(&str) -> Result<T>,
{
    match dotenvy::var(key) {
        Ok(raw) => {
            let value = raw.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                parser(value).map(Some)
            }
        }
        Err(dotenvy::Error::EnvVar(VarError::NotPresent)) => Ok(None),
        Err(err) => Err(err.into()),
    }
}
