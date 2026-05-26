use anyhow::Result;

use crate::model::PolymarketPrice;

pub fn parse_polymarket_market(_body: &str) -> Result<Vec<PolymarketPrice>> {
    anyhow::bail!("polymarket parsing is not implemented yet")
}
