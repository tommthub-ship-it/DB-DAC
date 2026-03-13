use anyhow::Result;
use colored::Colorize;

use crate::client::DacClient;

pub async fn run(client: &DacClient) -> Result<()> {
    let resp = client.health().await?;
    println!("{} 서버 응답: {}", "✓".green().bold(), resp);
    Ok(())
}
