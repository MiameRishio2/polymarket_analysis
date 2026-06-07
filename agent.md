# Agent 工作规范

> 本文件是 agent 的唯一入口点。每次启动任务前，agent 必须依次读取本文件与设计规范文档，再开始执行任何操作。

---

## 必读文件

| 文件 | 说明 | 读取时机 |
|------|------|----------|
| `agent.md` | 工作规范与行为约束 | 每次任务前 |
| `session.md` | 任务状态记录 | 每次任务前 |
| `change_log.md` | 操作日志记录 | 每次任务前 |
| `web_design.md` | 网页设计规范 | 执行网页任务前 |
| `architect.md` | 项目架构规范 | 每次任务前 |
| `test.md` | 测试点清单 | 编写测试前 |

---

## 设计规范索引

### 1. session.md - 任务状态记录

**核心用途**：记录任务中间状态，确保断点续接和状态恢复。

**关键规则**：
- 如果 `session.md` 为空或不包含任务状态，必须创建新的 session.md
- session.md 记录的是**实际状态**，而非模板

**关键字段**：
- `task_status` - 任务状态（null/pending/in_progress/completed/blocked）
- `completed_steps` - 已完成步骤
- `pending_steps` - 待完成步骤
- `last_action` - 最后操作
- `last_checkpoint` - 最后检查点

### 2. change_log.md - 操作日志

**核心用途**：记录每次操作历史，便于问题追溯和上下文恢复。

**关键字段**：
- `time` - 操作时间
- `step` - 当前步骤
- `action` - 动作描述
- `detail` - 详细说明
- `files_changed` - 修改文件
- `test_result` - 测试结果
- `next_action` - 下一步动作

### 3. web_design.md - 网页设计规范

**核心用途**：定义列表页面结构和交互规范。

**关键规范**：
- 分页显示：每页 10 条
- 列表形式：表格展示，禁止卡片/按钮
- 必要组件：`stats-bar`、`pagination`、`list-table`
- 适用页面：`/menu`、`/menu/football`、`/menu/{sport}`

### 4. architect.md - 项目架构规范

**核心用途**：定义代码分层和目录结构。

**关键规范**：
- 网页端代码放 `public/`
- 数据端代码放 `src/`
- 禁止跨层混写
- 跨层通信通过 HTTP API

### 5. test.md - 测试点清单

**核心用途**：记录测试用例和覆盖率要求。

**关键规范**：
- 新增功能必须先写测试
- 测试通过后才能写实现
- 全部测试通过才能继续
- 任务完成需同步更新

---

## 流程规范

执行流程遵循 comet skill 规范。（详见后续文档）

---

## 禁止行为

| 禁止 | 说明 |
|------|------|
| 跳过测试直接写代码 | 必须 TDD |
| 部分测试通过就继续 | 全部通过才行 |
| 提交无法编译的代码 | build 必须成功 |
| 修改功能不写测试 | 测试覆盖率要求 |
| 忽略测试失败继续开发 | 失败必须修复 |
| 状态变更后不更新 session.md | 会丢失进度 |
| 使用 `#[allow(dead_code)]` 规避 warning | 必须修复 warning |

---

## 禁止工具

| 工具 | 禁止原因 |
|------|----------|
| `apply_patch` | 存在 bug，禁止使用 |

---

## 代码规范

- 单个 `.rs` 文件不超过 500 行
- 严格遵守 architect.md 的分层规范
- 新增功能必须先写测试，测试通过后才能写实现

---

## 任务结束后

任务完成后必须更新以下文件：

| 文件 | 必填 | 说明 |
|------|------|------|
| `session.md` | ✅ | 更新 task_status=completed、current_step、last_action |
| `change_log.md` | ✅ | 追加最终操作记录 |
| `test.md` | ⚠️ | 如有新测试点则更新 |
| `architect.md` | ⚠️ | 如有架构变更则更新 |

---

*本文件为机器可读规范，agent 必须严格遵守。*
*更新日期：2026-06-07*
