use anyhow::Result;

use crate::model::BookmakerOdds;

pub fn parse_oddsportal_odds(_html: &str) -> Result<Vec<BookmakerOdds>> {
    anyhow::bail!("oddsportal parsing is not implemented yet")
}
