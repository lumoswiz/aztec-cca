# Aztec CCA Bid Bot

Bot for submitting bids to Aztec’s Uniswap Continuous Clearing Auction (CCA).

## Configure

- Environment variables in `.env`. Example [here](.env.example).
- Bids in `bids.toml`. Example [here](bids.toml.example)


## Running the Bot

### Local
```bash
cargo run --release
```

### Docker

#### Local

```bash
docker build -t aztec-cca:local .
```

```bash
docker run --rm --name aztec-cca \
  --env-file .env \
  -v "$PWD/bids.toml:/app/bids.toml:ro" \
  aztec-cca:local
```

#### Prebuilt Image

```bash
docker run --rm --name aztec-cca \
  --env-file .env \
  -v "$PWD/bids.toml:/app/bids.toml:ro" \
  ghcr.io/lumoswiz/aztec-cca:0.1.0
```

## How it Works

1. **Configuration loading** – [`src/config.rs`](./src/config.rs)  
   Reads environment variables, builds the provider with a signer, and turns `bids.toml` into structured bid specs.

2. **Bid validation & planning** – [`src/validate.rs`](./src/validate.rs), [`src/bids.rs`](./src/bids.rs)  
   Checks every bid amount, aligns prices into ticks (if not already), and buckets them into `PlannedBid`s.

3. **Auction snapshot** – [`src/auction.rs`](./src/auction.rs)  
   Fetches the auction snapshot, tick list, and eligibility data once so all bids share the same context.

4. **Execution pipeline** – [`src/blocks.rs`](./src/blocks.rs), [`src/registry.rs`](./src/registry.rs), [`src/transaction.rs`](./src/transaction.rs)  
   Streams headers, feeds pending bids through **prepare → simulate → send**, and retries up to three times per failure.

5. **Logging & summary** – [`src/logging.rs`](./src/logging.rs)  
   Prints a final summary once every bid has succeeded/failed or the auction window closes.

## Safety

This software is provided on an “as is” and “as available” basis. We make no warranties and accept no liability for losses arising from its use.
