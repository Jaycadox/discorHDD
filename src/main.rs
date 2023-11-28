mod bridge;
mod config;
mod drive;
use crate::bridge::*;
use crate::config::*;
use crate::drive::*;
use anyhow::Result;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Make config
    let config = get_config();

    // Make runtime
    let rt = tokio::runtime::Runtime::new()?;

    // Call main
    rt.block_on(async move { async_main(config).await })?;

    Ok(())
}

async fn async_main(config: Config) -> Result<()> {
    // Create DiscordDrive and ChunkManager
    let mut drive = DiscordDrive::<DiscordBridge>::new(config).await?;
    let mut chunk_man = ChunkManager::from_file_or_new(100)?;

    // Perform test
    println!("Preparing to read...");
    let r = chunk_man.read(0, 10, &mut drive).await?;
    println!("Got data: {:?}. Now writing new data...", r);
    chunk_man.write(2, &[69; 5], &mut drive).await?;

    println!("Reading again...");
    let r = chunk_man.read(0, 10, &mut drive).await?;
    println!("Got data: {:?}", r);

    Ok(())
}
