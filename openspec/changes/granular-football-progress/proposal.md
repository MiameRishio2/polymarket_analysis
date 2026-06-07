# Proposal: 足球爬取细化进度显示

## 问题描述

当前足球数据刷新只显示简单的加载状态，用户无法得知：
1. 正在爬取的具体内容（哪个国家/联赛）
2. 已完成多少
3. 剩余多少

用户日志显示：
```
INFO polymarket_analysis::menu::football::scraper: Scraping football data from: https://www.oddsportal.com/football/
```
爬取过程较慢，但前端只显示通用"加载中"提示。

## 目标

在足球数据刷新过程中，实时显示：
- **当前操作**：正在爬取的具体分类名称（如"正在获取 Algeria 联赛"）
- **已处理数**：已完成爬取的项目数量
- **剩余数**：还需爬取的项目数量
- **进度百分比**：基于已处理/总数的百分比

## 影响范围

### 后端修改
- `src/menu/football/scraper.rs` - 爬取过程中报告进度
- `src/menu/football/handlers.rs` - 使用细化进度更新
- `src/menu/football/progress.rs` - 可能需要增强进度字段

### 前端修改
- `public/football.html` - 显示更详细的进度信息（已处理/剩余）

## 优先级

高 - 改善用户体验，减少"假死"感知
