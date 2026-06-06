# Proposal: 增加测试点系统

## Why（为什么需要这个变更）

当前项目在快速开发阶段，需要建立 **代码质量保障机制**，确保核心功能模块在开发过程中始终保持可用状态。

### 当前问题
1. **缺乏自动化测试验证** - 没有测试点机制，无法确保代码变更不破坏现有功能
2. **配置分散** - 缺少统一的配置管理规范
3. **网站访问未测试** - oddsportal 和 polymarket 的访问逻辑没有测试保障

### 业务价值
- 通过测试点强制要求，提高代码质量
- 配置集中管理，提高可维护性
- 为后续功能扩展打下坚实基础

---

## What（具体做什么）

### 必须实现
1. **测试点机制**
   - 在 `src/` 目录下建立测试点
   - 使用 Rust 单元测试 + 集成测试
   - CI/CD 前置条件：所有测试必须通过才能合并代码

2. **配置管理**
   - 使用 `config.yaml` 作为唯一配置源
   - 配置结构清晰，包含 proxy 和目标 URL
   - 代码读取配置而非硬编码

3. **网站访问**
   - 封装 HTTP 客户端 (`src/http.rs`)
   - 支持 oddsportal.com 访问
   - 支持 polymarket.com 访问
   - 包含错误处理和重试逻辑

### 测试点清单（必须通过）
- [ ] `config.yaml` 配置加载测试
- [ ] HTTP 客户端初始化测试
- [ ] oddsportal URL 可达性测试（mock）
- [ ] polymarket URL 可达性测试（mock）
- [ ] 代理配置读取测试

---

## Success Criteria（成功标准）

1. **所有测试点通过** - `cargo test` 全部绿标
2. **配置零硬编码** - 所有配置从 `config.yaml` 读取
3. **代码可运行** - `cargo run` 能正常启动服务
4. **文档完整** - 更新 `architect.md` 反映新架构

---

## Constraints（约束条件）

- 必须使用 `config.yaml` 进行配置管理
- 访问 oddsportal 和 polymarket 两个网站
- 测试点必须实现才能进行代码开发
- 严格遵守 `architect.md` 的前后端分离规范

---

Change: add-testing-checkpoints
Created: 2026-06-06
