# Agent 工作规范

> 本文件是 agent 的唯一入口点。**每次启动任务前，agent 必须依次读取本文件与 session.md，再开始执行任何操作。**

---

## 必读文件（按顺序）

1. **`agent.md`**（本文件）— agent 的工作规范与行为约束
2. **`session.md`** — 当前任务状态、进度与上下文
3. **`architect.md`** — 项目架构与代码分层规范
4. **`test.md`** — 测试点清单与规范

读取顺序不可跳过。每次任务开始前，agent 必须先确认 session.md 中的当前状态。

---

## 📋 session.md 规范

> **核心用途**：任务中间状态记录，确保 agent 断点续接和状态恢复。**

### 设计原则

| 原则 | 说明 |
|------|------|
| **可中断** | 任何时候可停止，下次从断点继续 |
| **可恢复** | 新 agent 读取后能完全恢复上下文 |
| **可追溯** | log 记录所有操作历史 |
| **简洁明确** | 核心字段一目了然 |

### 文件结构

```yaml
# === 任务状态（核心）===
task_name: "任务名称"
task_status: "in_progress"      # null/pending/in_progress/completed/blocked
task_goal: "任务目标描述"
current_step: "当前步骤"
test_status: "passed"           # pending/passed/failed

# === 进度跟踪 ===
completed_steps:                # 已完成步骤列表
  - "步骤1: 编写测试"
  - "步骤2: 实现功能"
pending_steps:                  # 待完成步骤列表
  - "步骤3: 编写集成测试"
  - "步骤4: 更新文档"
blocked_steps: []               # 阻塞中的步骤

# === 阻塞因素 ===
blockers:                       # 如有阻塞填写
  - name: "阻塞项"
    reason: "原因"
    solution: "解决方案"

# === 操作日志（关键）===
log:                            # 每次状态变更追加
  - time: "2026-06-06T12:00:00Z"
    step: "步骤1"
    action: "完成：编写测试"
    detail: "test_config_load_success 已添加"
    files_changed: ["tests/config_test.rs"]
    test_result: "failed"       # 或 "passed"
    next_action: "继续实现 config.rs"

# === 快照（便于快速定位）===
last_action: "完成：编写测试"
last_action_time: "2026-06-06T12:00:00Z"
last_checkpoint: "步骤1完成，等待步骤2"
```

### 状态流转

```
┌─────────────────────────────────────────────────────────────┐
│                     任务状态流转                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   null ──→ pending ──→ in_progress ──→ completed           │
│    ↑           │              │                            │
│    │           │              ↓                            │
│    │           │         blocked ──→ 解决后继续 in_progress │
│    │           │                                             │
│    └───────────┴───── 新任务开始                             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 关键字段说明

| 字段 | 必填 | 说明 | 示例 |
|------|------|------|------|
| `task_name` | ✅ | 任务名称 | "增加测试点系统" |
| `task_status` | ✅ | 状态 | "in_progress" |
| `task_goal` | ✅ | 目标描述 | "实现 17 个测试点" |
| `current_step` | ✅ | 当前步骤 | "步骤3: 编写集成测试" |
| `test_status` | ✅ | 测试状态 | "passed" |
| `completed_steps` | ✅ | 已完成列表 | ["步骤1", "步骤2"] |
| `pending_steps` | ✅ | 待完成列表 | ["步骤3", "步骤4"] |
| `log` | ✅ | 操作历史 | 见上方示例 |
| `last_action` | ✅ | 最后操作 | "完成：编写测试" |
| `blockers` | ⚠️ | 阻塞因素 | 有阻塞时必填 |

### 状态恢复流程

当 agent 启动时读取 `session.md`：

```
1. 检查 task_status
   │
   ├── null ──→ 无任务，等待新任务
   │
   ├── pending ──→ 新任务，准备开始
   │   → 更新 task_status = "in_progress"
   │   → 从 pending_steps 第一项开始
   │
   ├── in_progress ──→ 从断点继续
   │   → 读取 completed_steps → 避免重复
   │   → 读取 current_step → 知道当前位置
   │   → 读取 last_checkpoint → 快速定位
   │   → 阅读 log 最后几条 → 恢复上下文
   │   → 进入 pending_steps 下一项
   │
   ├── completed ──→ 任务完成，等待新任务
   │
   └── blocked ──→ 有阻塞
       → 读取 blockers → 了解问题
       → 尝试解决或报告
```

### 日志记录规范

每次以下操作必须追加 log：

| 操作 | 记录内容 |
|------|----------|
| 开始任务 | time, action="任务开始", next_action |
| 完成步骤 | time, step, action, files_changed, next_action |
| 测试结果 | time, test_result, detail |
| 遇到阻塞 | time, action, blockers.reason, blockers.solution |
| 任务完成 | time, action, summary |

### 更新时机

| 时机 | 是否更新 | 更新内容 |
|------|----------|----------|
| 任务开始 | ✅ | task_status, task_goal, pending_steps |
| 完成一个步骤 | ✅ | completed_steps, current_step, log |
| 测试结果变化 | ✅ | test_status, log |
| 遇到阻塞 | ✅ | blocked_steps, blockers, log |
| 任务完成 | ✅ | task_status, log |
| 每次代码变更 | ⚠️ | files_changed in log（可选） |

---

## 🔴 强制流程：测试先行原则

> **严格遵守下述流程，否则禁止进行代码开发。**

### TDD 开发流程

```
┌─────────────────────────────────────────────────────────────┐
│                    开发循环（必须遵循）                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   1️⃣ 编写测试                                              │
│       ↓                                                     │
│   2️⃣ 运行测试 → 预期失败（测试未通过）                        │
│       ↓                                                     │
│   3️⃣ 编写代码（使测试通过）                                  │
│       ↓                                                     │
│   4️⃣ 运行测试 → 验证通过 ✅                                 │
│       ↓                                                     │
│   5️⃣ 同步文档 → 更新 test.md（如有新测试点）                  │
│       ↓                                                     │
│   6️⃣ 更新 session.md → 记录进度                             │
│       ↓                                                     │
│   7️⃣ 提交代码（或进入下一个功能）                            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 流程规则

| 规则 | 说明 | 违规处理 |
|------|------|----------|
| **R1: 先写测试** | 任何代码变更前，必须先编写对应的测试用例 | 禁止直接写业务代码 |
| **R2: 测试驱动** | 根据测试需求编写实现代码 | 禁止 TDD 倒置 |
| **R3: 全量测试** | `cargo test --all` 必须全部通过 | 禁止跳过测试 |
| **R4: 通过才能继续** | 测试通过前禁止开始下一个功能 | 阻塞后续开发 |
| **R5: 构建验证** | `cargo build` 编译成功 | 禁止提交无法编译的代码 |
| **R6: 文档同步** | 任务完成后同步更新 test.md | 禁止遗漏文档更新 |
| **R7: 状态同步** | 每次状态变更后更新 session.md | 禁止状态丢失 |

### 禁止行为清单

| ❌ 禁止 | 说明 |
|---------|------|
| 跳过测试直接写代码 | 必须 TDD |
| 部分测试通过就继续 | 全部通过才行 |
| 提交无法编译的代码 | build 必须成功 |
| 修改功能不写测试 | 测试覆盖率要求 |
| 忽略测试失败继续开发 | 失败必须修复 |
| 任务完成不更新 test.md | 新增测试点必须记录 |
| 状态变更后不更新 session.md | 会丢失进度 |

---

## 行为约束

### 开始任务前

```
1. 读取 agent.md（本文件）
2. 读取 session.md → 了解当前任务状态和上下文
3. 读取 architect.md → 理解架构规范
4. 读取 test.md → 了解测试点要求
5. 根据 task_status 判断：
   - null/pending → 新任务，开始
   - in_progress → 从断点继续
   - completed → 任务完成，等待新任务
   - blocked → 解决阻塞或报告
6. 更新 session.md（设置 task_status = "in_progress"）
```

### 任务状态变更后

每次对代码、配置或状态的修改完成后：
- **立即更新 session.md** 中的任务状态
- **追加 log 记录**
- 更新 `completed_steps` / `pending_steps`
- 更新 `current_step` 和 `last_action`

### 代码规范

- **严格遵守 architect.md 的分层规范**：网页端代码放 `public/`，数据端代码放 `src/`
- 禁止在 `public/` 目录下写 Rust 代码
- 禁止在 `src/` 目录下写前端 HTML/JS
- 所有跨层通信通过 HTTP API 进行
- 代码改动须与 architect.md 保持同步
- **新增功能必须先写测试**，测试通过后才能写实现

---

## 流程检查清单

每次完成代码开发后，必须逐项检查：

- [ ] **R1** 已编写测试（不能跳过）
- [ ] `cargo test --all` 全部通过
- [ ] `cargo build` 编译成功
- [ ] 无新增 warning
- [ ] session.md 已更新（task_status, completed_steps, pending_steps, log）
- [ ] architect.md 已同步（如有架构变更）
- [ ] test.md 已更新（如有新测试点）

**全部勾选后，才能继续下一个功能。**

---

## 快速参考

### 新 agent 启动流程

```
1. 读取 agent.md → 了解工作规范
2. 读取 session.md → 了解当前状态
   ├─ task_status = null → 空闲，等待任务
   ├─ task_status = pending → 新任务，准备开始
   ├─ task_status = in_progress → 从断点继续
   │   → completed_steps + pending_steps
   │   → last_action + last_checkpoint
   │   → log 最新几条
   ├─ task_status = completed → 任务完成，等待新任务
   └─ task_status = blocked → 查看 blockers
3. 读取 architect.md → 了解架构规范
4. 读取 test.md → 了解测试要求
5. 开始执行
```

### 状态恢复判断

| session.md 状态 | agent 行为 |
|-----------------|------------|
| `task_status: null` | 空闲状态，等待新任务 |
| `task_status: pending` | 新任务，准备开始 |
| `task_status: in_progress` | **从断点继续**，查看 completed_steps |
| `task_status: completed` | 任务完成，等待新任务 |
| `task_status: blocked` | 任务阻塞，查看 blockers |

---

## 文档同步规则

### test.md 同步时机

| 场景 | 是否需要更新 test.md |
|------|---------------------|
| 新增测试用例 | ✅ 必须更新 |
| 修改测试逻辑 | ✅ 必须更新 |
| 删除测试用例 | ✅ 必须更新 |
| 纯代码修改（无新增测试） | ❌ 可跳过 |
| 文档整理 | ✅ 建议更新 |

---

*本文件为机器可读规范，agent 必须严格遵守。*
*更新日期：2026-06-06*
