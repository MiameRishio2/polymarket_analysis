# Agent 工作规范

> 本文件是 agent 的唯一入口点。**每次启动任务前，agent 必须依次读取本文件与 session.md，再开始执行任何操作。**

---

## 必读文件（按顺序）

1. **`agent.md`**（本文件）— agent 的工作规范与行为约束
2. **`session.md`** — 当前任务状态、进度与上下文
3. **`architect.md`** — 项目架构与代码分层规范

读取顺序不可跳过。每次任务开始前，agent 必须先确认 session.md 中的当前状态。

---

## 行为约束

### 开始任务前

```
1. 读取 agent.md
2. 读取 session.md → 了解当前任务状态与上下文
3. 读取 architect.md → 理解架构规范
4. 如果 session.md 中 task_status 为 "pending"：
   → 理解目标后，先更新 session.md（设置 task_status = "in_progress"）
   → 禁止在状态更新前写代码
5. 如果 task_status 为 "in_progress"：
   → 从上次中断处继续，不重复已完成的工作
```

### 任务状态变更后

每次对代码、配置或状态的修改完成后：
- 立即更新 `session.md` 中的 `task_status` 与进度字段
- 记录 `last_action`（刚刚完成的操作）
- 更新 `updated_at`

### 代码规范

- **严格遵守 architect.md 的分层规范**：网页端代码放 `public/`，数据端代码放 `src/`
- 禁止在 `public/` 目录下写 Rust 代码
- 禁止在 `src/` 目录下写前端 HTML/JS
- 所有跨层通信通过 HTTP API 进行
- 代码改动须与 architect.md 保持同步

### 架构变更

如果需要修改架构（如新增目录、新增 API、新增模块）：
1. 先更新 `architect.md`
2. 再更新 `session.md`
3. 最后再写代码

---

## session.md 字段说明

| 字段 | 说明 |
|------|------|
| `task_name` | 当前任务名称 |
| `task_status` | `pending` / `in_progress` / `completed` / `blocked` |
| `task_goal` | 任务目标描述 |
| `current_step` | 当前正在进行的步骤 |
| `completed_steps` | 已完成步骤列表 |
| `pending_steps` | 待完成步骤列表 |
| `last_action` | 最近一次操作描述 |
| `blockers` | 阻塞因素（如有） |
| `updated_at` | 最后更新时间 |

---

## 变更日志规范

每次修改 session.md 时，在 `log` 字段追加一条记录：

```yaml
log:
  - time: "2026-06-06T12:00:00Z"
    action: "任务开始：创建 hello world 项目"
    from: null
    to: "in_progress"
  - time: "2026-06-06T12:05:00Z"
    action: "完成：创建 Cargo.toml + handlers.rs"
    from: "in_progress"
    to: "in_progress"
```

---

*本文件为机器可读规范，agent 必须严格遵守。*
