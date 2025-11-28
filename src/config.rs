use alloy::{
    primitives::{Address, U256},
    rpc::client::BuiltInConnectionString,
    signers::local::PrivateKeySigner,
};
use eyre::{Result, WrapErr, eyre};
use serde::Deserialize;
use std::{env::VarError, fs, path::Path, str::FromStr};

const DEFAULT_BIDS_FILE: &str = "bids.toml";

#[derive(Debug)]
pub struct Config {
    pub transport: BuiltInConnectionString,
    pub signer: PrivateKeySigner,
    pub bids: Vec<BidParams>,
}

#[derive(Debug, Clone)]
pub struct BidParams {
    pub max_bid: U256,
    pub amount: u128,
    pub owner: Address,
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

        let bids = load_bids(owner)?;

        Ok(Self {
            transport,
            bids,
            signer,
        })
    }
}

fn load_bids(default_owner: Address) -> Result<Vec<BidParams>> {
    let path = Path::new(DEFAULT_BIDS_FILE);
    let contents = fs::read_to_string(path)
        .wrap_err(format!("failed to read bids config at {}", path.display()))?;
    let file: BidFile =
        toml::from_str(&contents).wrap_err("failed to parse bids config (expected TOML format)")?;
    if file.bids.is_empty() {
        return Err(eyre!(
            "bids config must include at least one [[bids]] entry"
        ));
    }

    file.bids
        .into_iter()
        .map(|bid| bid.into_params(default_owner))
        .collect()
}

#[derive(Debug, Deserialize)]
struct BidFile {
    bids: Vec<BidSpec>,
}

#[derive(Debug, Deserialize)]
struct BidSpec {
    max_bid: String,
    amount: String,
    owner: Option<String>,
}

impl BidSpec {
    fn into_params(self, default_owner: Address) -> Result<BidParams> {
        let max_bid = U256::from_str(self.max_bid.trim())
            .map_err(|_| eyre!("bid entry max_bid is not a valid U256: {}", self.max_bid))?;

        let amount = u128::from_str(self.amount.trim())
            .map_err(|_| eyre!("bid entry amount is not a valid u128: {}", self.amount))?;

        let owner = match self.owner {
            Some(raw) => Address::parse_checksummed(raw.trim(), None)
                .map_err(|_| eyre!("bid entry owner is not a valid checksummed address: {raw}"))?,
            None => default_owner,
        };

        Ok(BidParams {
            max_bid,
            amount,
            owner,
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
