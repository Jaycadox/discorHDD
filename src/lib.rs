pub mod bridge;
pub mod config;
pub mod discord_drive;
#[cfg(test)]
pub mod test_bridge;

use std::{ops::DerefMut, sync::Arc};
use tokio::sync::Mutex;

use nbdkit::*;

/// DiscordNBDKit drive adapter
struct DiscordNBD {
    /// A DiscordDrive
    drive: Arc<Mutex<discord_drive::DiscordDrive<bridge::DiscordBridge>>>,

    /// A ChunkManager
    chunk_manager: Arc<Mutex<discord_drive::ChunkManager>>,

    /// An async runtime
    rt: tokio::runtime::Runtime,
}

/// Default is implemented due to nbdkit-rust requiring it
impl Default for DiscordNBD {
    fn default() -> Self {
        // Create a tokio runtime
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Get the config
        let config = config::get_config();

        // Attempt to create the DiscordDrive from the config
        let drive = rt
            .block_on(async move { discord_drive::DiscordDrive::new(config).await })
            .unwrap();

        // Create the ChunkManager
        let chunk_manager = discord_drive::ChunkManager::from_file_or_new(1000).unwrap();

        Self {
            chunk_manager: Arc::new(Mutex::new(chunk_manager)),
            drive: Arc::new(Mutex::new(drive)),
            rt,
        }
    }
}

impl Server for DiscordNBD {
    fn get_size(&self) -> Result<i64> {
        // By default the ChunkManager is much larger (1GB), but the FAT12 file system can't store
        // that much
        Ok(50_000_000)
    }

    /// The name which the nbdkit driver would use
    fn name() -> &'static str
    where
        Self: Sized,
    {
        "discorhddnbdkit"
    }

    /// Create a default DiscordNBD
    fn open(readonly: bool) -> Result<Box<dyn Server>>
    where
        Self: Sized,
    {
        let _ = readonly;
        Ok(Box::<DiscordNBD>::default())
    }

    /// Synchronous wrapper around ChunkManager
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<()> {
        let len = buf.len();

        // Spawn runtime
        let bytes = self
            .rt
            .block_on(async move {
                // Call ChunkManager::read
                self.chunk_manager
                    .lock()
                    .await
                    .read(offset as usize, len, self.drive.lock().await.deref_mut())
                    .await
            })
            .map_err(|_e| nbdkit::Error::new(1, "Failed to read"))?;

        // Set data in given buffer
        for (old, new) in buf.iter_mut().zip(bytes.iter()) {
            *old = *new;
        }

        Ok(())
    }

    fn can_write(&self) -> Result<bool> {
        Ok(true)
    }

    /// Synchronous wrapper around ChunkManager
    fn write_at(&self, buf: &[u8], offset: u64, flags: Flags) -> Result<()> {
        let _ = flags;

        // Spawn runtime
        self.rt
            .block_on(async move {
                // Call ChunkManager::write
                self.chunk_manager
                    .lock()
                    .await
                    .write(offset as usize, buf, self.drive.lock().await.deref_mut())
                    .await
            })
            .map_err(|_e| nbdkit::Error::new(1, "Failed to write"))?;

        Ok(())
    }

    fn thread_model() -> Result<ThreadModel>
    where
        Self: Sized,
    {
        // Other threading models might be possible, but I'm just playing it safe
        Ok(ThreadModel::SerializeConnections)
    }
}

// Create plugin and provide optional functions
plugin!(DiscordNBD {
    write_at,
    can_write,
    thread_model
});
