# Session 状态

> **核心用途**：任务中间状态记录，用于 agent 断点续接和状态恢复。
> 新 agent 启动时，通过本文件快速了解任务进度，无需从头开始。

---

## 任务状态

| 字段 | 值 | 说明 |
|------|-----|------|
| `task_name` | 待定 | 当前任务名称 |
| `task_status` | null | `null`=空闲 / `pending`=待开始 / `in_progress`=进行中 / `completed`=已完成 / `blocked`=阻塞 |
| `task_goal` | - | 任务目标（简洁描述） |
| `current_step` | - | 当前进行的步骤编号/名称 |
| `test_status` | - | `pending` / `passed` / `failed` |

---

## 进度跟踪

| 字段 | 值 |
|------|-----|
| `completed_steps` | [] |
| `pending_steps` | [] |
| `blocked_steps` | [] |

---

## 阻塞因素

> 如有阻塞，填写原因和解决方案

| 阻塞项 | 原因 | 解决方案 | 状态 |
|--------|------|----------|------|
| - | - | - | - |

---

## 操作日志

> **关键**：每次状态变更必须追加记录，用于恢复上下文

```yaml
log:
  - time: ""
    step: ""
    action: ""
    detail: ""
    files_changed: []
    test_result: ""
    next_action: ""
```

---

## 最近状态快照

| 字段 | 值 |
|------|-----|
| `last_action` | - |
| `last_action_time` | - |
| `last_checkpoint` | - |

---

*本文件由 agent 自动维护，每次状态变更后必须更新*
*更新时间：2026-06-06T21:20:00Z*
