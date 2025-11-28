use crate::auction::AuctionParams;
use alloy::primitives::U256;

pub fn align_price_to_tick(price: U256, params: &AuctionParams) -> U256 {
    let floor = params.floor_price;
    let spacing = params.tick_spacing;
    let cap = params.max_bid_price;

    if price >= cap {
        return cap;
    }

    if price <= floor {
        return floor;
    }

    let offset = price - floor;
    let rem = offset % spacing;
    if rem.is_zero() {
        return price.min(cap);
    }

    let down = price - rem;
    let up = down + spacing;
    let choose_up = rem > spacing - rem;
    let candidate = if choose_up { up } else { down };
    candidate.min(cap)
}

#[cfg(test)]
mod tests {
    use super::align_price_to_tick;
    use crate::auction::AuctionParams;
    use alloy::primitives::U256;

    fn params() -> AuctionParams {
        use std::str::FromStr;
        AuctionParams {
            contributor_period_end_block: U256::ZERO,
            max_purchase_limit: U256::ZERO,
            floor_price: U256::from_str("753956294022871543408300").unwrap(),
            tick_spacing: U256::from_str("7539562940228715434083").unwrap(),
            max_bid_price: U256::from_str("217900404829510685459725614601655060836").unwrap(),
            end_block: U256::ZERO,
            total_purchased: U256::ZERO,
            has_any_token: true,
        }
    }

    #[test]
    fn keeps_aligned_prices() {
        let params = params();
        use std::str::FromStr;
        let aligned = vec![
            U256::from_str("19807042548578993971286201723").unwrap(),
            U256::from_str("784114545783786405144632").unwrap(),
            U256::from_str("1839653357415806565916252").unwrap(),
        ];
        for price in aligned {
            assert_eq!(align_price_to_tick(price, &params), price);
        }
    }

    #[test]
    fn snaps_to_nearest_tick() {
        let params = params();
        let spacing = params.tick_spacing;
        let floor = params.floor_price;
        let price = floor + spacing * U256::from(10) + spacing / U256::from(3);
        let expected = floor + spacing * U256::from(10);
        assert_eq!(align_price_to_tick(price, &params), expected);
    }

    #[test]
    fn clamps_above_cap() {
        let params = params();
        let price = params.max_bid_price + U256::from(1);
        assert_eq!(align_price_to_tick(price, &params), params.max_bid_price);
    }
}
