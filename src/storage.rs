use anyhow::Result;

use crate::cli::ExportArgs;

pub async fn export_match(_args: ExportArgs) -> Result<()> {
    anyhow::bail!("export is not implemented yet")
}
