//! 配置管理模块
//! 
//! 从 config.yaml 加载配置，提供应用所需的配置结构体

use serde::Deserialize;
use std::fs;
use std::net::IpAddr;
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

/// Web 服务配置
#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    pub host: String,
    pub port: u16,
}

/// 应用配置（顶层聚合）
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub proxy_enabled: bool,
    pub proxy: String,
    #[serde(rename = "scrape_sports")]
    pub scrape: ScrapeConfig,
    pub web: WebConfig,
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

    /// 获取 Web 服务端口
    pub fn web_port(&self) -> u16 {
        self.web.port
    }

    /// 获取 Web 服务绑定地址
    pub fn web_host(&self) -> &str {
        &self.web.host
    }

    /// 检查是否允许远程访问（绑定到非本地地址）
    pub fn is_remote_access_enabled(&self) -> bool {
        match self.web.host.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => !ip.is_loopback(),
            Ok(IpAddr::V6(_)) => true,
            Err(_) => self.web.host == "0.0.0.0" || self.web.host == "::",
        }
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

