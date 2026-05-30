//! Polymarket 分析工具的根库模块
//!
//! 本模块是整个项目的核心库入口，导出了所有功能子模块，包括：
//!
//! - [`cli`] - 命令行接口模块，负责解析用户输入并提供交互式操作界面
//! - [`collector`] - 数据采集模块，负责从 Polymarket 等平台抓取市场数据
//! - [`config`] - 配置加载模块，负责从 YAML 文件加载应用配置
//! - [`http`] - HTTP 客户端模块，封装网络请求逻辑
//! - [`match_resolver`] - 匹配解析模块，用于解析和匹配市场预测结果
//! - [`model`] - 数据模型模块，定义项目中使用的核心数据结构
//! - [`providers`] - 数据提供者模块，对接不同的数据源接口
//! - [`storage`] - 存储模块，负责数据的持久化与缓存

/// 命令行接口模块
pub mod cli;
/// 数据采集模块
pub mod collector;
/// 配置加载模块
pub mod config;
/// HTTP 客户端模块
pub mod http;
/// 匹配解析模块
pub mod match_resolver;
/// 数据模型模块
pub mod model;
/// 数据提供者模块
pub mod providers;
/// 存储模块
pub mod storage;
