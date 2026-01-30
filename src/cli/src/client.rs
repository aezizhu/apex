//! HTTP client for communicating with the Apex API server.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// API response wrapper matching the server's ApiResponse format.
#[derive(Debug, serde::Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub error_code: Option<String>,
}

/// HTTP client for the Apex API.
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    /// Create a new API client pointing at the given base URL.
    pub fn new(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Return the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Perform a GET request and deserialize the response data.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {} failed", url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }

        let api_resp: ApiResponse<T> = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse response from {}", url))?;

        if api_resp.success {
            api_resp
                .data
                .ok_or_else(|| anyhow::anyhow!("API returned success but no data"))
        } else {
            Err(anyhow::anyhow!(
                "API error: {}",
                api_resp.error.unwrap_or_else(|| "Unknown error".into())
            ))
        }
    }

    /// Perform a POST request with a JSON body and deserialize the response.
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {} failed", url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }

        let api_resp: ApiResponse<T> = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse response from {}", url))?;

        if api_resp.success {
            api_resp
                .data
                .ok_or_else(|| anyhow::anyhow!("API returned success but no data"))
        } else {
            Err(anyhow::anyhow!(
                "API error: {}",
                api_resp.error.unwrap_or_else(|| "Unknown error".into())
            ))
        }
    }

    /// Perform a DELETE request and deserialize the response.
    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .with_context(|| format!("DELETE {} failed", url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }

        let api_resp: ApiResponse<T> = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse response from {}", url))?;

        if api_resp.success {
            api_resp
                .data
                .ok_or_else(|| anyhow::anyhow!("API returned success but no data"))
        } else {
            Err(anyhow::anyhow!(
                "API error: {}",
                api_resp.error.unwrap_or_else(|| "Unknown error".into())
            ))
        }
    }

    /// Perform a raw GET request and return the full JSON value (for health endpoint).
    pub async fn get_raw(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {} failed", url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }

        resp.json()
            .await
            .with_context(|| format!("Failed to parse response from {}", url))
    }
}
