# Design: 新项目架构设计

## 1. 技术栈

| 层 | 技术选型 | 说明 |
|---|---|---|
| 后端运行时 | Rust (stable) | 参考 `src_old` 技术栈 |
| HTTP 框架 | Axum 0.7 | `src_old/web.rs` 核心依赖 |
| 序列化 | serde + serde_json | API 响应序列化 |
| 静态文件服务 | Axum static_files 路由 | 托管 `public/` 目录 |
| 前端 | 纯 HTML/CSS/JS | 无框架，最小化依赖 |

## 2. 项目目录结构

```
polymarket_analysis/
├── Cargo.toml              # Rust 依赖定义
├── src/
│   ├── lib.rs              # 库入口，模块导出
│   ├── main.rs             # 可执行入口
│   └── handlers.rs         # API Handler（当前仅 hello）
├── public/                 # 前端静态文件（网页端）
│   └── index.html          # Hello World 页面
└── openspec/               # OpenSpec 变更管理（变更范围外）
```

### 分离原则

- **`src/`** — 数据端：Rust 后端，处理 HTTP 请求、业务逻辑、数据结构
- **`public/`** — 网页端：纯静态文件，浏览器直接访问，无构建步骤
- 后端通过 Axum 路由 `/` 托管 `public/` 目录，实现前后端同端口部署

## 3. API 设计

### GET /api/hello

返回简单的 JSON 问候响应。

**Request**: 无需参数

**Response 200**:
```json
{
  "message": "Hello, World!",
  "timestamp": "2026-06-06T19:00:00Z"
}
```

## 4. 前端页面

`public/index.html` — 单文件，包含：
- 标题 "Hello World"
- 展示 API 返回数据的区域
- fetch 调用 `/api/hello` 并渲染

## 5. 启动方式

```bash
cargo run
# 服务启动于 http://127.0.0.1:8080
# 前端: http://127.0.0.1:8080/
# API:  http://127.0.0.1:8080/api/hello
```

## 6. 关键设计决策

1. **同端口部署**：后端和前端共享同一 Axum 实例，前者通过路由区分
2. **无前端构建工具**：直接用原生 HTML/CSS/JS，避免工程复杂度
3. **基于 Axum 0.7**：与 `src_old` 保持主版本一致
4. **minimal lib.rs**：仅导出 `handlers` 模块，便于后续扩展
