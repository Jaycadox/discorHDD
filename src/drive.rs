use anyhow::{anyhow, Result};
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serenity::{
    model::{
        channel::{AttachmentType, Channel, GuildChannel},
        gateway::GatewayIntents,
    },
    Client,
};
use std::{
    borrow::Cow,
    io::Write,
    sync::{atomic::AtomicUsize, Arc},
};
use tokio::sync::Mutex;

use crate::config::Config;

/// A DiscordDrive is used to upload and download array's of bytes to-and-fro discord. Chunking is
/// handled automatically but the read/write interface is non-standard. Use ChunkManager to obtain
/// a standard read/write interface with a DiscordDrive.
pub struct DiscordDrive {
    /// The discord message channel to upload and download from
    channel: GuildChannel,

    /// Http object to send requests to-and-fro discord
    http: Arc<serenity::http::client::Http>,
}

impl DiscordDrive {
    /// The chunk size of each file uploaded to discord (in bytes). A smaller values handle small
    /// reads/writes faster, and larger values handle larger reads/writes faster. Note that discord
    /// does have a file size limits for bots (25MB on non-boosted servers).
    const MAX_FILE_SIZE: usize = 1_000_000;

    /// Attempt to create a DiscordDrive from a config. Config needs to contain a valid bot token
    /// and valid message channel which the bot can read and write from.
    pub async fn new(config: Config) -> Result<Self> {
        // Allow bot to read server messages
        let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

        // Create discord client with config token and intents
        let client = Client::builder(&config.token, intents).await?;

        // Extract a channel from the configuration
        let channel = client
            .cache_and_http
            .http
            .get_channel(config.channel)
            .await?;

        // Extract GuildChannel from Channel
        let channel = if let Channel::Guild(x) = channel {
            Ok(x)
        } else {
            Err(anyhow!("Only guild channels are supported at the moment."))
        }?;

        Ok(Self {
            http: client.cache_and_http.http.clone(),
            channel,
        })
    }

    /// Uploads bytes in slice to discord and returns a list of message ids which contain the bytes.
    /// The result of this function can be fed into DiscordDrive::download_bytes to read the data
    /// back
    async fn upload_bytes(&mut self, bytes: &[u8]) -> Result<Vec<u64>> {
        // Chunk the data into MAX_FILE_SIZE sized chunks
        let file_chunks = bytes.chunks(Self::MAX_FILE_SIZE);

        // For every chunk, turn it into a Bytes attachment with ascending file names
        let attachments = file_chunks.enumerate().map(|(i, c)| AttachmentType::Bytes {
            data: Cow::from(c),
            filename: format!("{i}.txt"),
        });

        // Create a zeroed vector with size of attachments, used to store the message ids returned
        // to us by discord
        let ids = Arc::new(Mutex::new(Vec::with_capacity(attachments.len())));
        for _ in 0..attachments.len() {
            ids.lock().await.push(0);
        }

        // For every attachment
        futures::future::try_join_all(attachments.enumerate().map(|(i, a)| {
            // Copy the ids vector Arc
            let ids2 = ids.clone();

            let i = AtomicUsize::new(i);
            // Send an empty message containing only the attachment
            self.channel
                .send_files(&self.http, vec![a], |m| m.content(""))
                .then(|m| async move {
                    // After the message was sent, extract it
                    match m {
                        Ok(m) => {
                            // Set index i in the ids vector to the message id of the message
                            ids2.lock().await[i.load(std::sync::atomic::Ordering::Relaxed)] =
                                m.id.0;
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                })
        }))
        .await?;

        // Extract ids out of Arc<Mutex<_>>
        let ids = Arc::try_unwrap(ids).unwrap();
        let ids = ids.into_inner();
        Ok(ids)
    }

    /// Download bytes from list of message ids returned from Self::upload_bytes
    async fn download_bytes(&mut self, ids: &[u64]) -> Result<Vec<u8>> {
        // For each message id, get the message from discord
        let messages = ids
            .iter()
            .map(|id| self.http.get_message(self.channel.id.0, *id))
            .collect::<Vec<_>>();

        // Await all "get message" futures
        let messages = futures::future::try_join_all(messages).await?;

        // Extract single attachment from each message onto the vector
        let mut attachments = Vec::with_capacity(messages.len());
        for message in messages {
            if message.attachments.len() != 1 {
                return Err(anyhow!("message didn't have a singular attachment"));
            }
            attachments.push(message.attachments[0].clone());
        }

        // Download attachment bytes from all messages
        let attachments =
            futures::future::try_join_all(attachments.iter().map(|a| a.download())).await?;

        // Flatten bytes to 1D vector
        let output = attachments.iter().flatten().cloned().collect::<Vec<_>>();

        Ok(output)
    }
}

/// A ChunkManager provides a standard read/write interface for a DiscordDrive.
#[derive(Serialize, Deserialize)]
pub struct ChunkManager {
    /// Table which maps chunk indexes to discord messasge ids
    chunk_table: Vec<Option<u64>>,
}

impl ChunkManager {
    /// Attempts to create a new ChunkManager or uses the one already found on disk
    pub fn from_file_or_new(new_size: usize) -> Result<ChunkManager> {
        let path = Self::get_file_path();
        if std::path::Path::exists(&path) {
            let content = std::fs::read_to_string(&path)?;
            return Ok(serde_json::from_str(&content)?);
        }

        let mut chunk_table = Vec::with_capacity(new_size);
        for _ in 0..new_size {
            chunk_table.push(None);
        }
        Ok(Self { chunk_table })
    }

    /// Saves current ChunkManager to file
    fn save_to_file(&self) -> Result<()> {
        let path = Self::get_file_path();

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)?;
        file.write_all(serde_json::to_string(&self)?.as_bytes())?;
        Ok(())
    }

    /// The common file path in which the Chunkmanager is stored
    fn get_file_path() -> std::path::PathBuf {
        "/etc/discorhdd/chunks.json".into()
    }

    /// From a starting offset and size, return all of chunk indexes that hold the data
    fn get_chunk_indexes(&self, start: usize, len: usize) -> Result<Vec<usize>> {
        let start_idx = start / DiscordDrive::MAX_FILE_SIZE;
        let end_idx = (start + len) / DiscordDrive::MAX_FILE_SIZE;

        Ok((start_idx..=end_idx).collect())
    }

    /// Returns the entirety of every chunk which holds a portion of the data
    async fn read_entire_chunks(
        &self,
        start: usize,
        len: usize,
        drive: &mut DiscordDrive,
    ) -> Result<Vec<u8>> {
        // Get chunk indexes from start and length and then get the corresponding message ids that
        // they are stored in
        let chunk_ids = self
            .get_chunk_indexes(start, len)?
            .iter()
            .map(|x| self.chunk_table[*x])
            .collect::<Vec<_>>();

        let mut buf = Vec::with_capacity(len);

        // For every chunk id/message id
        for chunk_id in chunk_ids {
            match chunk_id {
                // If there is a corresponding discord message id which contains the data
                Some(id) => {
                    // Download the data in the message
                    let mut down = drive.download_bytes(&[id]).await?;
                    // Pad the data so it has length MAX_FILE_SIZE
                    while down.len() != DiscordDrive::MAX_FILE_SIZE {
                        down.push(0);
                    }
                    buf.append(&mut down);
                }
                None => {
                    // Fill buffer with zero's. There's no need to query discord if we know the
                    // chunk hasn't been uploaded
                    buf.append(&mut vec![0; DiscordDrive::MAX_FILE_SIZE]);
                }
            }
        }
        Ok(buf)
    }

    /// Reads given a start/offset and length. Length of returned vector should always match the len argument
    pub async fn read(
        &self,
        start: usize,
        len: usize,
        drive: &mut DiscordDrive,
    ) -> Result<Vec<u8>> {
        let buf = self.read_entire_chunks(start, len, drive).await?;
        Ok(buf
            [(start % DiscordDrive::MAX_FILE_SIZE)..((start % DiscordDrive::MAX_FILE_SIZE) + len)]
            .to_vec())
    }

    /// Writes given a start/offset and length
    pub async fn write(
        &mut self,
        start: usize,
        data: &[u8],
        drive: &mut DiscordDrive,
    ) -> Result<()> {
        let len = data.len();

        // Download all the chunks which contain the data that needs to be written to
        let mut old_data = self.read_entire_chunks(start, len, drive).await?;

        // Copy the bytes from data into the old data vector at the proper position
        for (read_idx, data_idx) in ((start % DiscordDrive::MAX_FILE_SIZE)
            ..((start % DiscordDrive::MAX_FILE_SIZE) + len))
            .enumerate()
        {
            old_data[data_idx] = data[read_idx];
        }

        // Split the data back into MAX_FILE_SIZE sized chunks
        let new_data_chunks = old_data
            .chunks(DiscordDrive::MAX_FILE_SIZE)
            .collect::<Vec<_>>();

        // Get the indexes of the chunks which contain the message ids
        let chunk_idxs = self.get_chunk_indexes(start, len)?;

        // There should be the same number of new chunks as there were chunks originally because
        // the data wasn't resized
        if chunk_idxs.len() != new_data_chunks.len() {
            return Err(anyhow!("chunk mismatch"));
        }

        // Zip the new chunk data with the chunk index
        for (chunk_data, chunk_idx) in new_data_chunks.iter().zip(chunk_idxs) {
            // Upload the chunk to discord and get the message id(s)
            let message_ids = drive.upload_bytes(chunk_data).await?;

            // Because we're uploading singular chunks of size MAX_FILE_SIZE, it should be stored
            // in a singular message
            if message_ids.len() != 1 {
                return Err(anyhow!("currently chunk size should match, but it doesn't"));
            }
            // Get the message id from the single message
            let message_id = message_ids[0];

            // Set the chunk table at the chunk index to the new message id
            self.chunk_table[chunk_idx] = Some(message_id);
        }

        // Save the changes to file
        self.save_to_file()?;
        Ok(())
    }
}

impl Drop for ChunkManager {
    // Attempt to save the chunk manager when it is dropped
    fn drop(&mut self) {
        self.save_to_file()
            .unwrap_or_else(|e| println!("failed to save chunks to disk: {e}"));
    }
}
