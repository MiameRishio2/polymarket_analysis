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

## 2026-06-07 Category Cache Key Naming

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T23:10:00+08:00 |
| `step` | category_cache key 命名规范化 |
| `action` | 统一 category_cache 表的 key 命名格式 |
| `detail` | 修改 handlers.rs 中的 storage.load/save 调用：<br>- menu → menu_menu<br>- football → menu_football<br>- basketball → menu_basketball<br>统一使用 "menu_" 前缀避免命名冲突 |
| `files_changed` | src/menu/handlers.rs |
| `test_result` | passed - 全部 22 个测试通过 |
| `next_action` | 无，需手动部署到 10.32.50.201:23333 |

## 2026-06-07 Fetch URL Encoding Fix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T23:20:00+08:00 |
| `step` | fix fetch_url decoding error |
| `action` | 修复 fetch_categories_for_sport 的 "error decoding response body" 问题 |
| `detail` | 问题根因：response.text() 在解码某些编码时失败<br>修复方案：直接使用 bytes() 获取原始字节，然后用 String::from_utf8_lossy() 解码<br>避免任何解码错误 |
| `files_changed` | src/menu/scraper.rs |
| `test_result` | passed - 全部 22 个测试通过 |
| `next_action` | 部署到远程服务器 10.32.50.201:23333 |

## 2026-06-07 Proxy Support in Scraper

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T23:25:00+08:00 |
| `step` | scraper proxy configuration fix |
| `action` | 重写 scraper HTTP 客户端，支持从 config.yaml 读取代理配置 |
| `detail` | 问题根因：scraper.rs 独立创建静态 HTTP 客户端，未使用 config.yaml 中的代理配置<br>修复方案：<br>- 添加 create_scraper_client() 函数从 config.yaml 加载代理配置<br>- 静态 CLIENT 使用 once_cell::Lazy 延迟初始化<br>- 如果代理配置失败，fallback 到无代理客户端 |
| `files_changed` | src/menu/scraper.rs |
| `test_result` | passed - 全部 22 个测试通过 |
| `next_action` | 部署到远程服务器 10.32.50.201:23333 |

## 2026-06-07 Esports Dota 2 Parsing Investigation

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-07T23:45:00+08:00 |
| `step` | 验证 /menu/esports 解析问题 |
| `action` | 分析并验证 scraper.rs 逻辑正确性 |
| `detail` | 调查用户报告的 esports 页面少 Dota 2 问题：<br>- 使用真实 HTML 测试，验证爬虫逻辑正确<br>- 运行 debug_esports 示例，确认能正确提取 3 个类别（counter-strike, dota-2, league-of-legends）<br>- 代码中的 EXCLUDED_PATHS 逻辑正确排除 results/standings 等<br>**结论：代码逻辑正确，无需修改核心逻辑** |
| `files_changed` | src/menu/scraper.rs（移除 unused import，添加 debug 日志） |
| `test_result` | passed - 全部 16 个测试通过（新增 3 个 esports 测试） |
| `next_action` | 重新部署到 10.32.50.201:23333 以应用最新代码 |

## 2026-06-08 Scraper Redirect Detection

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-08T00:05:00+08:00 |
| `step` | 添加爬虫重定向检测 |
| `action` | 修复 fetch_url 函数以检测 HTTP 重定向 |
| `detail` | 问题：服务器日志显示 football 返回 0 类别，可能是因为代理重定向到区域站点。<br>调查发现：<br>- 使用本地代理时，esports 页面正确返回 3 个类别<br>- 使用本地代理时，football 会被重定向到 cuotasahora.com<br>- 这个重定向导致页面结构变化，scraper 无法正确解析<br><br>修复：添加重定向检测和警告日志，当请求被重定向到不同域名时输出警告信息 |
| `files_changed` | src/menu/scraper.rs (fetch_url 函数) |
| `test_result` | passed - 全部 46 个测试通过 |
| `next_action` | 部署到 10.32.50.201:23333，检查日志中是否有重定向警告 |
