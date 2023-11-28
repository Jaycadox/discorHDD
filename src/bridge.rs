use std::{borrow::Cow, sync::Arc};

use anyhow::{anyhow, Result};
use serenity::{
    async_trait,
    model::{channel::AttachmentType, gateway::GatewayIntents, id::ChannelId},
    Client,
};

#[async_trait]
pub trait ChatServiceBridge: Sized {
    /// The chunk size of each file uploaded to discord (in bytes). A smaller values handle small
    /// reads/writes faster, and larger values handle larger reads/writes faster. Note that discord
    /// does have a file size limits for bots (25MB on non-boosted servers).
    const MAX_FILE_SIZE: usize;

    /// Create a new bridge given a token and message channel
    async fn new(token: &str, channel: u64) -> Result<Self>;

    /// Send a message of bytes and get the resulting message id, if possible
    async fn send_message(&mut self, content: &[u8]) -> Result<u64>;

    /// From a message id, get the bytes uploaded prior
    async fn get_message(&self, id: u64) -> Result<Vec<u8>>;
}

pub struct DiscordBridge {
    /// The discord message channel to upload and download from
    channel: ChannelId,

    /// Http object to send requests to-and-fro discord
    http: Arc<serenity::http::client::Http>,
}

#[async_trait]
impl ChatServiceBridge for DiscordBridge {
    // 1MB message chunk size
    const MAX_FILE_SIZE: usize = 1_000_000;

    async fn new(token: &str, channel: u64) -> Result<Self> {
        // Allow bot to read server messages
        let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

        // Create discord client with config token and intents
        let client = Client::builder(token, intents).await?;
        Ok(Self {
            http: client.cache_and_http.http.clone(),
            channel: ChannelId(channel),
        })
    }

    async fn send_message(&mut self, content: &[u8]) -> Result<u64> {
        // Make sure the content that we are sending is not larger than the maximum file size
        if content.len() > Self::MAX_FILE_SIZE {
            return Err(anyhow!("Data was larger than max file size"));
        }

        // Turn attachment into serenity supported Bytes attachment
        let attachment = AttachmentType::Bytes {
            data: Cow::from(content),
            filename: "0.txt".to_owned(),
        };

        // Send a message with the attachment and no text content
        let message = self
            .channel
            .send_files(&self.http, vec![attachment], |m| m.content(""))
            .await?;

        // Return the id of the message
        Ok(message.id.0)
    }

    async fn get_message(&self, id: u64) -> Result<Vec<u8>> {
        // Get the message from the id
        let message = self.http.get_message(self.channel.into(), id).await?;

        // Get the attachment from the message
        let attachment = message
            .attachments
            .get(0)
            .ok_or(anyhow!("message must have an attachment"))?;

        // Get the bytes from the message attachment
        let attachment_data = attachment.download().await?;

        Ok(attachment_data)
    }
}
