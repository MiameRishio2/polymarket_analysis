//! HTTP 客户端模块
//! 
//! 封装 reqwest，提供统一的 HTTP 请求接口，支持代理配置和重试逻辑

use reqwest::Client;
use std::time::Duration;

use crate::config::AppConfig;

/// HTTP 客户端封装
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    pub oddsportal_url: String,
    pub polymarket_url: String,
}

impl HttpClient {
    /// 从配置创建新的 HTTP 客户端
    pub fn new(config: &AppConfig) -> Result<Self, HttpClientError> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

        // 配置代理
        if let Some(proxy) = config.proxy_url() {
            let proxy_url = reqwest::Proxy::http(&proxy)
                .or_else(|_| reqwest::Proxy::https(&proxy))
                .or_else(|_| reqwest::Proxy::all(&proxy));
            
            match proxy_url {
                Ok(p) => {
                    builder = builder.proxy(p);
                    tracing::info!("HTTP client configured with proxy: {}", proxy);
                }
                Err(e) => {
                    tracing::warn!("Failed to configure proxy: {}, continuing without proxy", e);
                }
            }
        }

        let client = builder.build().map_err(HttpClientError::BuildError)?;

        Ok(Self {
            client,
            oddsportal_url: config.oddsportal_url().to_string(),
            polymarket_url: config.polymarket_url().to_string(),
        })
    }

    /// GET 请求
    pub async fn get(&self, url: &str) -> Result<String, HttpClientError> {
        let response = self.client.get(url).send().await.map_err(HttpClientError::ReqwestError)?;
        let status = response.status();
        
        if !status.is_success() {
            return Err(HttpClientError::HttpError(format!(
                "GET {} failed with status: {}",
                url, status
            )));
        }

        let body = response.text().await.map_err(HttpClientError::ReqwestError)?;
        Ok(body)
    }

    /// HEAD 请求（检查 URL 可达性）
    pub async fn check_url(&self, url: &str) -> Result<bool, HttpClientError> {
        match self.client.head(url).send().await {
            Ok(response) => Ok(response.status().is_success() || response.status().as_u16() == 405),
            Err(_) => Ok(false),
        }
    }

    /// 带重试的 GET 请求
    pub async fn get_with_retry(&self, url: &str, retries: u8) -> Result<String, HttpClientError> {
        let mut last_error = HttpClientError::MaxRetriesExceeded(url.to_string());
        
        for i in 0..=retries {
            match self.get(url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("Attempt {} failed: {}", i + 1, e);
                    last_error = e;
                    if i < retries {
                        tokio::time::sleep(Duration::from_millis(500 * (i + 1) as u64)).await;
                    }
                }
            }
        }
        
        Err(last_error)
    }
}

/// HTTP 客户端错误类型
#[derive(Debug)]
pub enum HttpClientError {
    BuildError(reqwest::Error),
    ReqwestError(reqwest::Error),
    HttpError(String),
    MaxRetriesExceeded(String),
}

impl std::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpClientError::BuildError(e) => write!(f, "Failed to build HTTP client: {}", e),
            HttpClientError::ReqwestError(e) => write!(f, "HTTP request error: {}", e),
            HttpClientError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            HttpClientError::MaxRetriesExceeded(url) => write!(f, "Max retries exceeded for URL: {}", url),
        }
    }
}

impl std::error::Error for HttpClientError {}

