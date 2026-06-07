# Tasks: 足球爬取细化进度显示

## 任务清单

- [ ] 1. 分析当前 `scrape_football` 函数结构
- [ ] 2. 修改 `scraper.rs` 添加进度报告点
- [ ] 3. 更新 `handlers.rs` 确保使用细化进度
- [ ] 4. 增强前端 `football.html` 显示已处理/剩余数
- [ ] 5. 运行测试验证
- [ ] 6. 手动测试观察进度更新

## 任务详情

### Task 1: 分析当前 scrape_football 函数结构

**文件**: `src/menu/football/scraper.rs`

**目标**: 了解当前爬取流程，确定进度报告插入点

**步骤**:
1. 读取 `scrape_football` 函数
2. 识别爬取循环/异步操作的位置
3. 记录需要添加进度报告的代码位置

### Task 2: 修改 scraper.rs 添加进度报告点

**文件**: `src/menu/football/scraper.rs`

**目标**: 在爬取过程中实时报告进度

**步骤**:
1. 在函数开始时调用 `progress::start_refresh()`
2. 获取分类列表后调用 `progress::set_total_items()`
3. 在循环中调用 `progress::update_progress()` 报告当前分类
4. 每完成一个分类后调用 `progress::item_processed()`
5. 函数结束时调用 `progress::complete_refresh()`

### Task 3: 更新 handlers.rs 确保使用细化进度

**文件**: `src/menu/football/handlers.rs`

**目标**: 确认 handler 正确使用 scraper 的进度报告

**步骤**:
1. 检查 `football_refresh_handler` 是否调用了 `scrape_football`
2. 确认 handler 中的 progress 状态更新与 scraper 一致
3. 添加错误处理中的 `progress::fail_refresh()` 调用

### Task 4: 增强前端显示已处理/剩余数

**文件**: `public/football.html`

**目标**: 在进度条旁显示详细数字

**步骤**:
1. 找到当前进度显示位置
2. 添加 "已处理/总数" 格式显示
3. 添加 "剩余 X 个" 文字显示
4. 验证 UI 布局美观

### Task 5: 运行测试验证

**目标**: 确保修改未破坏现有功能

**步骤**:
```bash
cargo test --all
```

**验收标准**: 全部测试通过

### Task 6: 手动测试观察进度更新

**目标**: 验证进度实时显示效果

**步骤**:
1. 启动服务 `cargo run`
2. 打开 `public/football.html`
3. 点击刷新按钮
4. 观察进度显示是否实时更新
5. 记录日志中的进度信息
