//! Menu data models

use serde::{Deserialize, Serialize};

/// 单个体育分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SportCategory {
    pub slug: String,
    pub name: String,
    pub url: String,
}

/// 菜单数据响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuData {
    pub sports: Vec<SportCategory>,
    pub last_updated: String,
    pub source: String,
}
