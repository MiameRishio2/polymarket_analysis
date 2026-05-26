use anyhow::Result;

use crate::model::MatchIdentity;

pub fn resolve_from_text(_text: &str) -> Result<MatchIdentity> {
    anyhow::bail!("match resolution is not implemented yet")
}
