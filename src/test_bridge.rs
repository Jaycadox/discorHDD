use crate::{bridge::ChatServiceBridge, config::Config};
use anyhow::Result;
use serenity::async_trait;

pub struct TestBridge {
    data: Vec<Vec<u8>>,
}

#[async_trait]
impl ChatServiceBridge for TestBridge {
    const MAX_FILE_SIZE: usize = 1_000_000;

    async fn new(token: &str, channel: u64) -> Result<Self> {
        let _ = token;
        let _ = channel;

        Ok(Self {
            data: Vec::with_capacity(1_000),
        })
    }

    async fn send_message(&mut self, content: &[u8]) -> Result<u64> {
        let idx = self.data.len();
        self.data.push(content.to_vec());

        return Ok(idx as u64);
    }

    async fn get_message(&self, id: u64) -> Result<Vec<u8>> {
        Ok(self
            .data
            .get(id as usize)
            .unwrap_or(&vec![0; Self::MAX_FILE_SIZE])
            .clone())
    }
}

pub fn test_config() -> Config {
    Config {
        token: "".to_owned(),
        channel: 0,
    }
}
