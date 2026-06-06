# 测试文档

> 本文档记录项目中所有测试点。**所有代码变更必须通过对应测试点才能合并。**

---

## 测试运行

```bash
# 运行所有测试
cargo test --all

# 运行指定测试文件
cargo test --test config_test
cargo test --test http_client_test
cargo test --test menu_scraper_test
cargo test --test storage_test

# 运行指定测试
cargo test test_config_load_success

# 带详细输出
cargo test --all -- --nocapture
```

---

## 测试清单

### 1. 配置模块测试 (`tests/config_test.rs`)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 1.1 | `test_config_load_success` | 验证 `config.yaml` 能正确加载并解析 | ✅ |
| 1.2 | `test_config_oddsportal_url` | 验证 oddsportal URL 正确解析 | ✅ |
| 1.3 | `test_config_polymarket_url` | 验证 polymarket URL 正确解析 | ✅ |
| 1.4 | `test_config_proxy_format` | 验证代理配置格式正确（http:// + 端口） | ✅ |
| 1.5 | `test_config_proxy_url_method` | 验证 `proxy_url()` 方法返回正确值 | ✅ |
| 1.6 | `test_config_web_host` | 验证 Web 服务主机配置 | ✅ |
| 1.7 | `test_config_web_port` | 验证 Web 服务端口配置 | ✅ |
| 1.8 | `test_config_remote_access_enabled` | 验证远程访问配置 | ✅ |
| 1.9 | `test_config_proxy_toggle_scenario` | 验证代理开关场景 | ✅ |
| 1.10 | `test_load_config_detailed` | 验证详细配置加载 | ✅ |
| 1.11 | `test_proxy_url_disabled` | 验证代理禁用时返回 None | ✅ |
| 1.12 | `test_proxy_url_enabled` | 验证代理启用时返回 Some | ✅ |

---

### 2. HTTP 客户端测试 (`tests/http_client_test.rs`)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 2.1 | `test_http_client_new` | 验证 HTTP 客户端创建成功 | ✅ |
| 2.2 | `test_http_client_clone` | 验证客户端可克隆且数据一致 | ✅ |
| 2.3 | `test_http_client_creation_with_proxy` | 验证带代理创建客户端 | ✅ |
| 2.4 | `test_http_client_oddsportal_url_format` | 验证 oddsportal URL 格式 | ✅ |
| 2.5 | `test_http_client_polymarket_url_format` | 验证 polymarket URL 格式 | ✅ |
| 2.6 | `test_http_client_init` | 验证 HTTP 客户端初始化 | ✅ |
| 2.7 | `test_check_url_localhost` | 验证 URL 检查方法 | ✅ |
| 2.8 | `test_wiremock_server_starts` | 验证 mock 服务器可启动 | ✅ |
| 2.9 | `test_mock_server_responds_to_get` | 验证 mock 响应注册能力 | ✅ |

---

### 3. 菜单爬取模块测试 (`tests/menu_scraper_test.rs`)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 3.1 | `test_menu_data_structure` | 验证菜单数据结构正确 | ✅ |
| 3.2 | `test_menu_data_serialization` | 验证菜单数据序列化/反序列化 | ✅ |
| 3.3 | `test_get_menu_or_default_returns_data` | 验证默认菜单返回数据 | ✅ |
| 3.4 | `test_get_cached_menu_returns_none_initially` | 验证缓存初始状态 | ✅ |
| 3.5 | `test_default_sports_contain_football` | 验证默认体育包含足球 | ✅ |
| 3.6 | `test_default_sports_count` | 验证默认体育分类数量 | ✅ |
| 3.7 | `test_sport_categories_are_unique` | 验证体育分类唯一性 | ✅ |

---

### 4. SQLite 存储测试 (`tests/storage_test.rs`)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 4.1 | `test_storage_new` | 验证存储实例创建 | ✅ |
| 4.2 | `test_save_and_load_menu` | 验证保存和加载菜单数据 | ✅ |
| 4.3 | `test_load_empty_storage` | 验证加载空存储返回 None | ✅ |
| 4.4 | `test_overwrite_menu` | 验证覆盖菜单数据 | ✅ |
| 4.5 | `test_has_cache` | 验证检查缓存状态 | ✅ |
| 4.6 | `test_clear_cache` | 验证清除缓存 | ✅ |

---

### 5. 单元测试 (`src/config.rs` 内联测试)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 5.1 | `test_load_config` | 验证配置加载功能 | ✅ |
| 5.2 | `test_proxy_url_disabled` | 验证代理禁用时返回 None | ✅ |
| 5.3 | `test_proxy_url_enabled` | 验证代理启用时返回 Some | ✅ |

---

### 6. HTTP 模块单元测试 (`src/http.rs` 内联测试)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 6.1 | `test_http_client_init` | 验证 HTTP 客户端初始化 | ✅ |
| 6.2 | `test_check_url_localhost` | 验证 URL 检查方法 | ✅ |

---

### 7. 菜单爬取模块单元测试 (`src/menu_scraper.rs` 内联测试)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 7.1 | `test_is_valid_sport_path` | 验证体育路径验证逻辑 | ✅ |
| 7.2 | `test_default_sports` | 验证默认体育列表 | ✅ |
| 7.3 | `test_get_menu_or_default` | 验证默认菜单获取 | ✅ |

---

### 8. 存储模块单元测试 (`src/storage.rs` 内联测试)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 8.1 | `test_storage_new_in_memory` | 验证内存存储创建 | ✅ |
| 8.2 | `test_save_and_load_menu` | 验证保存和加载菜单 | ✅ |
| 8.3 | `test_load_empty_storage` | 验证加载空存储 | ✅ |
| 8.4 | `test_has_cache` | 验证检查缓存 | ✅ |
| 8.5 | `test_clear_cache` | 验证清除缓存 | ✅ |
| 8.6 | `test_overwrite_menu` | 验证覆盖菜单 | ✅ |

---

## 测试覆盖矩阵

| 功能 | 单元测试 | 集成测试 | 状态 |
|------|----------|----------|------|
| config.yaml 加载 | ✅ | ✅ | ✅ |
| 代理配置解析 | ✅ | ✅ | ✅ |
| oddsportal URL | ✅ | ✅ | ✅ |
| polymarket URL | ✅ | ✅ | ✅ |
| HTTP 客户端创建 | ✅ | ✅ | ✅ |
| 客户端克隆 | - | ✅ | ✅ |
| Mock 服务器 | - | ✅ | ✅ |
| URL 格式验证 | - | ✅ | ✅ |
| 菜单数据爬取 | ✅ | ✅ | ✅ |
| 菜单数据序列化 | ✅ | ✅ | ✅ |
| 默认体育分类 | ✅ | ✅ | ✅ |
| SQLite 存储 | ✅ | ✅ | ✅ |
| 菜单数据持久化 | ✅ | ✅ | ✅ |

---

## 配置文件依赖

测试依赖 `config.yaml`：

```yaml
proxy_enabled: true
proxy: "http://10.32.110.233:7890"

scrape_sports:
  base_url:
    oddsportal_url: "https://www.oddsportal.com/"
    polymarket_url: "https://polymarket.com/"

web:
  host: "127.0.0.1"
  port: 8080
```

---

## 测试质量标准

| 标准 | 要求 | 当前状态 |
|------|------|----------|
| 全部通过 | `cargo test --all` 无失败 | ✅ 38/38 通过 |
| 无警告 | 编译无 warning | ✅ |
| 文档化 | 所有测试有文档 | ✅ |
| 覆盖配置 | config.yaml 全覆盖 | ✅ |
| 覆盖 HTTP | 客户端功能覆盖 | ✅ |
| 覆盖菜单爬取 | 菜单模块功能覆盖 | ✅ |
| 覆盖存储 | SQLite 存储功能覆盖 | ✅ |

---

## 维护指南

### 新增测试

1. 在对应测试文件添加 `#[test]` 或 `#[tokio::test]`
2. 编写测试逻辑和断言
3. 运行 `cargo test` 验证
4. 在本文档添加测试记录
5. 提交代码

### 修改后测试

1. 修改代码
2. 运行 `cargo test --all` 验证
3. 如有新增功能，添加新测试
4. 更新本文档

### 测试失败处理

1. 查看失败测试的断言信息
2. 定位问题代码
3. 修复后重新运行测试
4. 确认全部通过后提交

---

*本文档由 agent 维护，每次测试变更后必须同步更新。*
*更新时间：2026-06-06*
