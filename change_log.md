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

## 2026-06-08 Third-Level Football Category Page

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-08T22:30:00+08:00 |
| `step` | third-level football category page |
| `action` | 新增 `/menu/{sport}/{category}` 三级菜单页面和 `/api/menu/:sport/:category` API |
| `detail` | 按 Comet 新建 `third-level-football-category-page` change：<br>- 更新 `web_design.md`，新增三级分类页面、API、抓取规则<br>- 新增 path-aware scraper，提取 `/football/argentina/primera-nacional/` 直接子路径<br>- 新增三级 API 与 refresh API，缓存 key 使用 `menu_{sport}_{category}`<br>- 更新 `public/menu.html`，二级分类行跳转到本地三级页面，三级行保留 OddsPortal 原始路径<br>- 新增 Rust 和 Node 测试覆盖 scraper、API cache key、前端路由 |
| `files_changed` | Cargo.toml, Cargo.lock, public/menu.html, src/menu/scraper.rs, src/menu/handlers.rs, tests/menu_third_level_test.rs, tests/menu_page_config_test.js, web_design.md, architect.md, test.md, session.md, change_log.md, openspec/changes/third-level-football-category-page/*, docs/superpowers/* |
| `test_result` | passed - `cargo test --all` 通过（提权后运行，沙箱环境无法绑定 wiremock 端口）；`cargo build` 通过；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 按需部署到远程服务器并刷新 `/menu/football/argentina` 数据 |

## 2026-06-08 Second-Level Empty Cache Hotfix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-08T22:45:00+08:00 |
| `step` | second-level empty cache refresh fix |
| `action` | 修复 `/api/menu/football` 返回空缓存导致 `/menu/football` 无数据的问题 |
| `detail` | 调查发现：<br>- `curl http://10.32.50.201:23333/menu/football` 返回 Connection refused，远程端口未监听<br>- 本地 `/menu/football` 页面路由返回 200，页面路由本身正常<br>- 本地 `/api/menu/football` 返回旧空缓存，且 `data.sport` 为内部 key `menu_football`<br><br>修复：<br>- 二级分类缓存只有在 `categories` 非空时才作为可用缓存返回<br>- 命中非空缓存时，将 `data.sport` 归一为公开 slug（如 `football`）<br>- 空缓存会触发重新抓取并覆盖缓存 |
| `files_changed` | src/menu/handlers.rs, tests/menu_third_level_test.rs, test.md, session.md, change_log.md, openspec/changes/second-level-empty-cache-refresh-fix/* |
| `test_result` | passed - `cargo test --all` 通过；`cargo build` 通过；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 远程服务器启动/重启服务后，刷新 `/api/menu/football/refresh` 或清理 `menu_football` 空缓存 |

## 2026-06-08 Scraper Response Decoding Hotfix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-08T22:55:00+08:00 |
| `step` | scraper response body decoding |
| `action` | 修复 OddsPortal football 抓取时 `error decoding response body` 问题 |
| `detail` | 远程日志显示 `response.bytes()` 阶段仍报 `error decoding response body`。根因是 reqwest 会根据响应 `Content-Encoding` 自动解压，代理或站点返回损坏/不匹配 gzip 时，即使使用 bytes 也会先解压失败。修复：<br>- scraper client builder 禁用 `gzip`、`brotli`、`deflate`、`zstd` 自动解压<br>- 每个 scraper 请求显式设置 `Accept-Encoding: identity`<br>- 新增 wiremock regression，模拟 `Content-Encoding: gzip` 但 body 非 gzip 的响应，确保读取 body 不再失败 |
| `files_changed` | src/menu/scraper.rs, test.md, session.md, change_log.md |
| `test_result` | passed - `fetch_url_with_client_does_not_decode_bad_gzip_body` 通过；`cargo test --all` 通过；`cargo build` 通过；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 部署新二进制到远程服务器，重启后刷新 `/api/menu/football/refresh` 或清理 `menu_football` 空缓存 |

## 2026-06-08 Prevent Empty Error Cache Persist

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-08T23:05:41+08:00 |
| `step` | prevent empty error cache persist |
| `action` | 防止 OddsPortal 分类抓取失败时保存 0 categories 到缓存 |
| `detail` | 用户日志显示 `Failed to refresh stale football cache` 后立刻 `Saved 0 categories for 'menu_football'`。根因是 handler 对抓取失败构造的 `source=error` 空结果仍执行 storage.save。修复：新增 `should_persist_category_data`，二级/三级分类普通加载和 refresh handler 仅在结果非空且不是 error 时写入缓存；同时将二进制入口改为复用 library crate，清理重复私有模块编译产生的 warnings。说明：当前 scraper 已经是先下载完整 HTML 再解析，提速重点是避免失败结果污染缓存并减少无效后续空缓存命中。 |
| `files_changed` | src/menu/handlers.rs, src/main.rs, tests/test_scraper.rs, test.md, session.md, change_log.md, openspec/changes/prevent-empty-error-cache-persist/* |
| `test_result` | passed - `cargo test empty_error_category_data_is_not_persistable` 通过；`cargo test --all` 通过（提权运行 wiremock 端口绑定测试）；`cargo build` 通过且无 warning；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 部署新二进制到远程服务器，重启服务后清理旧 `menu_football` 空缓存或调用 `/api/menu/football/refresh` |

## 2026-06-09 Scraper HTTPS Proxy Fix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-09T09:39:33+08:00 |
| `step` | scraper HTTPS proxy routing |
| `action` | 修复 OddsPortal scraper 日志显示代理但 HTTPS 请求可能未走代理的问题 |
| `detail` | 用户反馈浏览器网页可访问但 scraper 仍失败。排查发现 `create_scraper_client` 优先使用 `Proxy::http(proxy_url)`，该调用成功只能说明代理配置可构造，不能证明 `https://www.oddsportal.com/football/` 走代理。修复为 `Proxy::all(proxy_url)`，日志改为 `Scraper using all-scheme proxy`；新增本地 fake proxy regression，验证 HTTPS OddsPortal 请求会向代理发送 `CONNECT www.oddsportal.com:443`。 |
| `files_changed` | src/menu/scraper.rs, architect.md, test.md, session.md, change_log.md, openspec/changes/scraper-https-proxy-fix/* |
| `test_result` | passed - `cargo test scraper_proxy_is_used_for_https_requests` 通过；`cargo test --all` 通过（提权运行本地端口测试）；`cargo build` 通过；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 部署新二进制并重启，刷新 `/api/menu/football/refresh`，确认日志出现 `Scraper using all-scheme proxy` |

## 2026-06-09 Fourth-Level Menu Pages

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-09T22:48:57+08:00 |
| `step` | fourth-level menu page implementation |
| `action` | 新增 `/menu/{sport}/{category}/{league}` 四级页面和 `/api/menu/:sport/:category/:league` API |
| `detail` | 用户要求 football 三级页面从 `https://www.oddsportal.com/football/world/` 获取 `href="/football/world/"` 下级，并将 `href="/football/world/world-championship-2026/"` 映射到本地四级页面。实现内容：三级分类行改为跳转本地四级页面；新增四级页面配置、API、refresh API、缓存 key `menu_{sport}_{category}_{league}`；scraper 复用嵌套路径提取逻辑抓取 `https://www.oddsportal.com/{sport}/{category}/{league}/` 的直接子路径。 |
| `files_changed` | public/menu.html, src/menu/handlers.rs, src/menu/scraper.rs, tests/menu_page_config_test.js, tests/menu_third_level_test.rs, web_design.md, architect.md, test.md, session.md, change_log.md |
| `test_result` | passed - `cargo test --all` 通过（提权运行本地端口测试）；`cargo build` 通过；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 部署新二进制并访问 `/menu/football/world`、`/menu/football/world/world-championship-2026` 验证远程页面 |

## 2026-06-09 Menu Trailing Slash Route Hotfix

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-09T23:05:00+08:00 |
| `step` | menu trailing slash route hotfix |
| `action` | 修复 `/menu/football/world/` 带尾部斜杠页面 404 |
| `detail` | 远程验证显示 `/menu/football/world/` 返回 404，而 `/menu/football/world` 返回 200。根因是 Axum 页面路由只注册无尾部斜杠版本，但前端三级行链接会生成带尾部斜杠 URL。修复为二级、三级、四级页面同时注册尾部斜杠路由，API 路径保持不变。 |
| `files_changed` | src/menu/handlers.rs, tests/menu_third_level_test.rs, web_design.md, architect.md, test.md, session.md, change_log.md |
| `test_result` | passed - `cargo test --all` 通过（提权运行本地端口测试）；`cargo build` 通过；`node tests/menu_page_config_test.js` 通过 |
| `next_action` | 部署新二进制并重启服务后，验证 `/menu/football/world/` 返回 200 |

## 2026-06-09 Menu Parent Back Button

| 字段 | 值 |
|------|-----|
| `time` | 2026-06-09T23:18:00+08:00 |
| `step` | menu parent back button |
| `action` | 在菜单子页面顶部增加“返回上一级”按钮 |
| `detail` | 按用户要求，每个 `/menu` 子页面顶部显示父级导航。返回目标基于当前本地路径段回退一级：`/menu/football/world/world-championship-2026/` → `/menu/football/world`，`/menu/football/world/` → `/menu/football`，`/menu/football/` → `/menu`；根页面 `/menu` 不显示按钮。 |
| `files_changed` | public/menu.html, tests/menu_page_config_test.js, web_design.md, test.md, session.md, change_log.md |
| `test_result` | passed - `node tests/menu_page_config_test.js` 通过；`cargo build` 通过；`cargo test --all` 通过（提权运行本地端口测试） |
| `next_action` | 部署新静态页面/二进制并访问四级页面，确认顶部返回按钮指向 `/menu/football/world` |

