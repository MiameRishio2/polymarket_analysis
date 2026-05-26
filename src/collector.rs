use anyhow::Result;

use crate::cli::CollectArgs;

pub async fn collect(_args: CollectArgs) -> Result<()> {
    anyhow::bail!("collection is not implemented yet")
}
