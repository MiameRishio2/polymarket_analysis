# Change Log

> **核心用途**：记录每次操作历史，便于问题追溯和上下文恢复。

---

## 记录规则

每次状态变更后追加记录：
- `time` - 操作时间（ISO 格式）
- `step` - 当前步骤
- `action` - 动作描述
- `detail` - 详细说明
- `files_changed` - 修改的文件列表
- `test_result` - 测试结果（passed/failed）
- `next_action` - 下一步动作

---

## 日志记录

（每次操作追加至此）

## 2026-06-07 Web Design Refactor

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T13:00:00+08:00 |
| `step` | web_design.md compliance verification |
| `action` | 分析并验证 public/menu.html 符合 web_design.md 规范 |
| `detail` | 确认以下规范点已正确实现：<br>- ITEMS_PER_PAGE = 10 ✅<br>- sports-grid (2列: slug, name) ✅<br>- category-grid (3列: type, name, url) ✅<br>- 类型标签颜色正确 ✅<br>- stats-bar, pagination, list-table 组件完整 ✅<br>- 数据处理使用 result.data.categories ✅ |
| `files_changed` | public/menu.html (无需修改，已符合规范) |
| `test_result` | N/A - 前端静态文件无需测试 |
| `next_action` | 无需进一步操作，代码已符合规范 |

## 2026-06-07 SQLite Edit Fix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T14:30:00+08:00 |
| `step` | sqlite.html edit form HTML escaping |
| `action` | 修复 editRow 编辑功能中 categories_json 只显示 {{ 的问题 |
| `detail` | 问题根因：escapeHtml 函数未转义换行符 \n，导致多行 JSON 在 HTML 属性中被截断。<br>修复方案：在 escapeHtml 返回前将 \n 替换为 <br>，并添加 null/undefined 检查。 |
| `files_changed` | public/sqlite.html (escapeHtml 函数) |
| `test_result` | N/A - 前端静态文件无需测试 |
| `next_action` | 无需进一步操作 |

## 2026-06-07 Menu Link Fix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T15:00:00+08:00 |
| `step` | menu.html link generation fix |
| `action` | 修复 /menu 页面跳转链接中出现 undefined 的问题 |
| `detail` | 问题根因：getPageConfig() 函数缺少 linkBase 字段，导致 config.linkBase 为 undefined。<br>修复方案：<br>- 添加 linkBase: '/menu/' 用于 /menu 页面<br>- 添加 linkBase: `/menu/${category}/` 用于 /menu/{category} 页面<br>- 同时修复了 apiUrl/refreshUrl/localKey 中使用 sport 变量的问题，改为使用正确的 category 变量 |
| `files_changed` | public/menu.html (getPageConfig 函数) |
| `test_result` | N/A - 前端静态文件无需测试 |
| `next_action` | 部署到远程服务器 10.32.50.201:23333 |

## 2026-06-07 Fetch Categories Fix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T22:30:00+08:00 |
| `step` | fetch_categories_for_sport parser fix |
| `action` | 修复 fetch_categories_for_sport 解析 "Upcoming Events" 区域标签失败的问题 |
| `detail` | 问题根因：<br>- HTML 解析错误时 fallback 到默认值（只有 5 个分类）<br>- 缺少对非分类链接的排除（results, standings 等）<br>修复方案：<br>- 添加 EXCLUDED_PATHS 常量排除非分类链接<br>- 改进错误处理：text() 失败时 fallback 到 String::from_utf8_lossy()<br>- 扩展国家/地区分类列表（添加更多非洲、亚洲国家）<br>- 添加 is_excluded_path() 函数排除无效路径<br>- 添加 extract_link_text() 函数正确提取链接文本 |
| `files_changed` | src/menu/scraper.rs, tests/menu_scraper_test.rs |
| `test_result` | passed - 全部 15 个测试通过 |
| `next_action` | 手动部署到远程服务器 10.32.50.201:23333 |

## 2026-06-07 Fetch Categories Fix (Simplified)

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T23:00:00+08:00 |
| `step` | simplify scraper logic |
| `action` | 简化 fetch_categories_for_sport，直接从 href 提取分类 |
| `detail` | 简化为直接提取 HTML 中的 href 链接：<br>- 使用 `extract_category_slug()` 匹配 `/football/{slug}/` 模式<br>- 使用 `String::from_utf8_lossy()` 处理编码问题<br>- 排除 results/standings 等页面链接<br>- 代码更简洁直接 |
| `files_changed` | src/menu/scraper.rs |
| `test_result` | passed - 全部 30 个测试通过 |
| `next_action` | 手动部署到远程服务器 10.32.50.201:23333 |
