# Design: 足球爬取细化进度显示

## 概述

在足球数据爬取过程中，实时报告每个分类的爬取进度，让用户清楚知道：
- 正在处理哪个分类
- 已完成多少
- 剩余多少

## 架构设计

### 当前状态

```rust
// progress.rs - 当前进度状态
pub struct RefreshProgress {
    pub current_operation: String,  // 当前操作描述
    pub percent: u32,               // 进度百分比
    pub total_items: u32,           // 总数
    pub processed_items: u32,       // 已处理数
}
```

### 方案

#### 1. 增强爬取进度报告

在 `scrape_football` 函数中，按以下阶段报告进度：

```
开始爬取
  ↓
发现 N 个分类
  ↓
[循环] 正在获取: {分类名} ({已处理}/{总数})
  ↓
解析数据
  ↓
完成
```

#### 2. 新增进度报告 API

```rust
// 新增函数
pub async fn report_progress(category: &str, processed: u32, total: u32);
```

#### 3. 前端显示增强

```html
<!-- 显示格式 -->
<div class="progress-detail">
  <span>正在获取: Algeria (3/15)</span>
  <span class="remaining">剩余: 12 个分类</span>
</div>
```

## 文件变更

### 1. `src/menu/football/scraper.rs`

在 `scrape_football` 函数中添加进度报告：

```rust
pub async fn scrape_football(client: &HttpClient) -> Result<FootballData, ScrapeError> {
    // 开始
    progress::start_refresh("正在获取足球分类列表...").await;
    
    // 发现分类后
    progress::set_total_items(categories.len() as u32).await;
    
    // 循环爬取每个分类
    for (i, category) in categories.iter().enumerate() {
        progress::update_progress(
            "fetching",
            &format!("正在获取: {} ({}/{})", category.name, i+1, total),
            ((i+1) * 100) / total
        ).await;
        
        // 执行爬取...
        
        progress::item_processed().await;
    }
    
    progress::complete_refresh().await;
}
```

### 2. `src/menu/football/handlers.rs`

更新 handler 以使用新的进度报告：

```rust
// 无需大改，主要依赖 scraper 内部的进度报告
```

### 3. `public/football.html`

增强进度显示：

```javascript
function updateProgressUI(progress) {
    // 显示当前操作
    document.getElementById('current-operation').textContent = progress.current_operation;
    
    // 显示详细进度 (已处理/总数)
    if (progress.total_items > 0) {
        const detail = `${progress.processed_items}/${progress.total_items}`;
        document.getElementById('progress-detail').textContent = detail;
    }
    
    // 显示剩余
    const remaining = progress.total_items - progress.processed_items;
    document.getElementById('remaining').textContent = `剩余: ${remaining} 个分类`;
}
```

## 测试计划

1. **单元测试** - 验证进度更新逻辑
2. **手动测试** - 观察进度显示是否实时更新
3. **日志验证** - 检查日志中的进度信息

## 风险评估

- **低风险**：纯增强功能，不破坏现有逻辑
- **向后兼容**：API 响应结构不变
