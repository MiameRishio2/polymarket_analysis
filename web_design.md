# 网页设计规范

> 在执行任何网页相关任务前，必须先阅读本文件。

---

## 页面显示规则

### 列表页面规范

| 规则 | 说明 |
|------|------|
| 分页显示 | 每页 10 个条目 |
| 列表形式 | 表格展示，禁止卡片/按钮 |
| 分页控制 | 上一页/下一页 + 页码导航 |

### 必要组件

| 组件 | 说明 |
|------|------|
| `stats-bar` | 统计信息（总数、各类型数量） |
| `pagination` | 分页导航 |
| `list-table` | 列表表格容器 |

### 列表布局

**体育列表（2 列）**：
```html
<div class="list-header sports-grid">
  <span>标识符</span>
  <span>名称</span>
</div>
<a href="..." class="list-row sports-grid">
  <span class="slug">football</span>
  <span class="name">足球</span>
</a>
```

**分类列表（3 列）**：
```html
<div class="list-header category-grid">
  <span>类型</span>
  <span>名称</span>
  <span>链接</span>
</div>
<a href="..." class="list-row category-grid">
  <span class="type country">country</span>
  <span class="name">Argentina</span>
  <span class="url">/football/argentina/</span>
</a>
```

### 类型标签样式

| 类型 | 样式类 | 颜色 |
|------|--------|------|
| country | `.type.country` | 蓝色 (#dbeafe) |
| global | `.type.global` | 黄色 (#fef3c7) |
| tournament | `.type.tournament` | 绿色 (#dcfce7) |
| league | `.type.league` | 紫色 (#f3e8ff) |
| other | `.type.other` | 灰色 (#f1f5f9) |

### 分页逻辑

```javascript
const ITEMS_PER_PAGE = 10;

function renderPage(page) {
  const totalPages = Math.ceil(data.length / ITEMS_PER_PAGE) || 1;
  const start = (page - 1) * ITEMS_PER_PAGE;
  const end = start + ITEMS_PER_PAGE;
  const pageItems = data.slice(start, end);
  // 渲染当前页数据
  // 渲染分页控件
}
```

### 数据加载机制

**加载顺序**：
1. 默认从 SQLite 读取缓存数据
2. 如果 SQLite 无数据，调用 API 获取数据并立即保存到 SQLite

**API 接口**：
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/menu` | 获取菜单数据 |
| POST | `/api/menu/refresh` | 刷新并保存数据 |

**加载流程**：
```javascript
async function fetchData() {
  // 1. 尝试从 API 获取数据（API 优先从 SQLite 返回缓存）
  const response = await fetch('/api/menu');
  const result = await response.json();
  
  if (result.ok && result.data) {
    data = result.data;
    renderPage(1);
  }
  // 如果无数据，API 会自动从网页获取并保存到 SQLite
}
```

### CSS 样式

- 列表行使用 `grid` 布局
- 悬停高亮：`background: #f8fafc`
- 分页按钮悬停：`background: #3b82f6; color: #fff`
- 当前页码：`background: #3b82f6; color: #fff`

### 适用页面

| 页面 | 布局 | 说明 |
|------|------|------|
| `/menu` | sports-grid | 体育菜单 |
| `/menu/{sport}` | category-grid | 体育分类（最多4层） |

---

*更新日期：2026-06-07*
