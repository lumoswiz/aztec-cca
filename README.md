# Aztec CCA Bid Bot

Bot for submitting bids to Aztec’s Uniswap Continuous Clearing Auction (CCA).

## Highlights
- Multiple bids per run, each with independent state (`Pending`, `Submitted`, `Failed`).
- Automatic tick alignment to the nearest valid CCA tick with warnings when an entry is adjusted.
- Full preflight validation (amount > 0, capped MAX_BID_PRICE, allocation limits).
- Transaction build/simulate/send loop with up to three retries per bid before marking as failed.
- Graceful shutdown: exits once every bid is submitted/exhausted or when the auction window ends.

## Prerequisites
- [Rust](https://rustup.rs/) 1.75+ (edition 2021).
- Access to an Ethereum RPC endpoint (HTTP, WebSocket, or IPC).
- An Ethereum account/private key funded with enough ETH to cover the configured bids and gas.

## Configure Environment Variables

- `RPC_ENDPOINT` accepts HTTP or WS URLs, or an IPC path inferred by Alloy’s `ProviderBuilder`.
- `PRIVATE_KEY` must control the funds and will be used as the default `owner` when not supplied in `bids.toml`.

## Configure Bids

Add your bids to `bids.toml`: 

```toml
[[bids]]
max_bid = "19807042548578993971286201723"
amount = "2000000000000000000"            
owner = "0x1234567890aBCdEf1234567890abCDef12345678"  # optional

[[bids]]
max_bid = "784114545783786405144632"
amount = "500000000000000000"              
# Defaults to the PRIVATE_KEY address when owner omitted
```

- `max_bid` format is Q96.
- Use `max_bid = "19807042548578993971286201723"` for a market order.

## Running the Bot
```bash
cargo run --release
```

What happens on startup:
1. Loads environment variables, parses RPC transport, and instantiates the signer.
2. Reads bid entries from `bids.toml`, validates them, and aligns `max_bid` to the closest valid tick.
3. Fetches auction parameters and validates bids against them.
4. Subscribes/polls for new blocks and waits until the public auction begins.
5. Iterates over pending bids each block: prepare params, build the transaction, simulate, and then send it. Failures are retried (up to three attempts) before the bid is marked failed.
6. Stops automatically when all bids are submitted/exhausted or the auction window closes.

## Safety

This software is provided on an “as is” and “as available” basis. We make no warranties and accept no liability for losses arising from its use.
