# 测试点清单

> 本文档记录项目中所有测试点。**所有代码变更必须通过对应测试点才能合并。**

---

## 测试运行

```bash
# 运行所有测试
cargo test --all

# 验证构建
cargo build
```

---

## 测试清单

### 1. 配置模块 (`tests/config_test.rs`)

| 测试名称 | 描述 |
|----------|------|
| `test_config_load_success` | config.yaml 正确加载 |
| `test_config_oddsportal_url` | oddsportal URL 正确解析 |
| `test_config_polymarket_url` | polymarket URL 正确解析 |
| `test_config_proxy_format` | 代理配置格式正确 |
| `test_config_proxy_url_method` | proxy_url() 方法正确 |

### 2. HTTP 客户端 (`tests/http_client_test.rs`)

| 测试名称 | 描述 |
|----------|------|
| `test_http_client_new` | HTTP 客户端创建成功 |
| `test_http_client_clone` | 客户端可克隆 |
| `test_http_client_creation_with_proxy` | 代理模式创建成功 |

### 3. 菜单爬取 (`tests/menu_scraper_test.rs`)

| 测试名称 | 描述 |
|----------|------|
| `test_menu_data_structure` | 菜单数据结构正确 |
| `test_menu_data_serialization` | 菜单数据序列化 |
| `test_get_menu_or_default_returns_data` | 默认菜单返回数据 |

### 4. SQLite 存储 (`tests/storage_test.rs`)

| 测试名称 | 描述 |
|----------|------|
| `test_storage_new` | 存储实例创建 |
| `test_save_and_load_menu` | 保存和加载菜单 |
| `test_load_empty_storage` | 加载空存储 |
| `test_overwrite_menu` | 覆盖菜单数据 |
| `test_has_cache` | 检查缓存状态 |
| `test_clear_cache` | 清除缓存 |

---

## 测试原则

| 原则 | 说明 |
|------|------|
| 先写测试 | 任何代码变更前，必须先编写测试用例 |
| 全部通过 | `cargo test --all` 必须全部通过 |
| 同步更新 | 任务完成需同步更新本文档 |

---

## 新增测试

1. 在对应测试文件添加 `#[test]` 或 `#[tokio::test]`
2. 编写测试逻辑和断言
3. 运行 `cargo test` 验证
4. 在本文档添加测试记录
5. 提交代码

---

*本文档由 agent 维护，每次测试变更后必须同步更新。*
*更新日期：2026-06-07*
