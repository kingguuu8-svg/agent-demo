use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};

use crate::{
    error::{AgentError, Result},
    model::{ChatRequest, ChatResponse},
};

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse>;
}

pub struct DeepSeekClient {
    client: Client,
    api_key: String,
    endpoint: String,
    retries: usize,
}

impl DeepSeekClient {
    pub fn new(api_key: impl Into<String>, base_url: impl AsRef<str>) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(660))
            .build()?;
        Ok(Self {
            client,
            api_key: api_key.into(),
            endpoint: format!(
                "{}/chat/completions",
                base_url.as_ref().trim_end_matches('/')
            ),
            retries: 2,
        })
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .map_err(|_| AgentError::Config("DEEPSEEK_API_KEY is not set".into()))?;
        let base = std::env::var("DEEPSEEK_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com".into());
        Self::new(api_key, base)
    }
}

#[async_trait]
impl LlmClient for DeepSeekClient {
    async fn complete(&self, request: &ChatRequest) -> Result<ChatResponse> {
        for attempt in 0..=self.retries {
            let response = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(request)
                .send()
                .await;

            match response {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await?;
                    if status.is_success() {
                        return serde_json::from_str(&body).map_err(AgentError::from);
                    }
                    if attempt < self.retries && retryable(status) {
                        tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await;
                        continue;
                    }
                    return Err(AgentError::Api {
                        status: status.as_u16(),
                        body,
                    });
                }
                Err(error)
                    if attempt < self.retries && (error.is_connect() || error.is_timeout()) =>
                {
                    tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!("retry loop always returns")
    }
}

fn retryable(status: StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 500 | 503)
}
