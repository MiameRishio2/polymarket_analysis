//! HTTP 客户端模块
//!
//! 此模块提供 HTTP 客户端的构建功能，用于发起网络请求。
//! 客户端支持可选的代理配置。

use anyhow::Result;
use reqwest::{Client, Proxy};

/// 构建并返回一个 HTTP 客户端
///
/// 使用 [`reqwest`] 库创建客户端实例。
/// 当 `proxy_enabled` 为 `true` 时，所有请求将通过 `proxy_url` 指定的代理服务器转发。
/// 当 `proxy_enabled` 为 `false` 时，客户端直接发起请求，不使用代理。
///
/// # 参数
///
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL（仅在 `proxy_enabled` 为 `true` 时生效）
///
/// # 返回
///
/// 返回一个配置完成的 `reqwest::Client` 实例，如果代理配置或客户端构建失败则返回错误。
pub fn build_http_client(proxy_enabled: bool, proxy_url: &str) -> Result<Client> {
    let mut builder = Client::builder();
    if proxy_enabled {
        builder = builder.proxy(Proxy::all(proxy_url)?);
    }
    Ok(builder.build()?)
}
