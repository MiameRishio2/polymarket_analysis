//! 配置管理模块
//! 
//! 从 config.yaml 加载配置，提供应用所需的配置结构体

use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// 代理配置
#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub proxy_enabled: bool,
    pub proxy: String,
}

/// 爬取目标 URL 配置
#[derive(Debug, Clone, Deserialize)]
pub struct BaseUrls {
    pub oddsportal_url: String,
    pub polymarket_url: String,
}

/// 爬取体育配置
#[derive(Debug, Clone, Deserialize)]
pub struct ScrapeConfig {
    pub base_url: BaseUrls,
}

/// 应用配置（顶层聚合）
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub proxy_enabled: bool,
    pub proxy: String,
    #[serde(rename = "scrape_sports")]
    pub scrape: ScrapeConfig,
}

impl AppConfig {
    /// 获取代理 URL（如果启用）
    pub fn proxy_url(&self) -> Option<String> {
        if self.proxy_enabled {
            Some(self.proxy.clone())
        } else {
            None
        }
    }

    /// 获取 oddsportal 基础 URL
    pub fn oddsportal_url(&self) -> &str {
        &self.scrape.base_url.oddsportal_url
    }

    /// 获取 polymarket 基础 URL
    pub fn polymarket_url(&self) -> &str {
        &self.scrape.base_url.polymarket_url
    }
}

/// 从文件加载配置
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<AppConfig, ConfigError> {
    let content = fs::read_to_string(path).map_err(ConfigError::IoError)?;
    let config: AppConfig = serde_yaml::from_str(&content).map_err(ConfigError::ParseError)?;
    Ok(config)
}

/// 初始化全局配置
pub fn init_config<P: AsRef<Path>>(path: P) -> Result<(), ConfigError> {
    let config = load_config(path)?;
    CONFIG.set(config).map_err(|_| ConfigError::AlreadyInitialized)?;
    Ok(())
}

/// 获取全局配置（需先调用 init_config）
pub fn get_config() -> &'static AppConfig {
    CONFIG.get().expect("config not initialized, call init_config first")
}

/// 配置错误类型
#[derive(Debug)]
pub enum ConfigError {
    IoError(std::io::Error),
    ParseError(serde_yaml::Error),
    AlreadyInitialized,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error reading config: {}", e),
            ConfigError::ParseError(e) => write!(f, "Parse error in config: {}", e),
            ConfigError::AlreadyInitialized => write!(f, "config already initialized"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let config = load_config("config.yaml").expect("failed to load config");
        
        // 验证 proxy 配置
        assert!(config.proxy_enabled, "proxy should be enabled");
        assert!(config.proxy.contains("7890"), "proxy should contain port 7890");
        
        // 验证 URL 配置
        assert!(
            config.oddsportal_url().contains("oddsportal"),
            "oddsportal_url should contain oddsportal"
        );
        assert!(
            config.polymarket_url().contains("polymarket"),
            "polymarket_url should contain polymarket"
        );
    }

    #[test]
    fn test_proxy_url_disabled() {
        let mut config = load_config("config.yaml").expect("failed to load config");
        config.proxy_enabled = false;
        
        assert!(config.proxy_url().is_none(), "proxy should be None when disabled");
    }

    #[test]
    fn test_proxy_url_enabled() {
        let config = load_config("config.yaml").expect("failed to load config");
        
        if config.proxy_enabled {
            let proxy = config.proxy_url();
            assert!(proxy.is_some(), "proxy should be Some when enabled");
            assert!(proxy.unwrap().starts_with("http"), "proxy should be http URL");
        }
    }
}
