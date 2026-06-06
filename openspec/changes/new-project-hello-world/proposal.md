# Proposal: 新项目 — 前后端分离 Hello World

## 摘要

基于 `src_old` 中 Rust/Axum 后端参考实现，创建一个最小化的 Hello World 项目。架构严格分离前端（静态页面）与后端（HTTP API），为后续功能开发奠定清晰的目录结构基础。

## 背景

当前 `src/` 目录为空，`src_old/` 中包含了完整的 Rust/Axum 项目结构（web、storage、scheduler 等模块）。新项目需要：
1. **保留技术栈**：继续使用 Rust + Axum 作为后端
2. **简化范围**：仅实现 hello world，无业务逻辑
3. **清晰架构**：前端与后端目录严格分离

## 目标

- 建立可运行的最小化 Web 项目
- 后端通过 Axum 提供 API 并托管静态前端文件
- 目录结构清晰，便于后续扩展

## 范围

### 包含
- `Cargo.toml` 依赖配置
- `src/main.rs` / `src/lib.rs` 最小化入口
- `src/handlers.rs` API handler（hello world）
- `public/index.html` 前端页面
- `public/` 目录作为静态文件根

### 不包含
- 数据库、存储层
- 业务数据采集逻辑
- 多页面路由
- 前端打包构建工具（Vite/Webpack 等）

## 风险与假设

- **假设**：Rust 工具链（cargo、rustc）已安装
- **风险**：无 — 项目结构简单，无技术难点
