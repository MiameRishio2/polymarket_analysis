
---

## 页面显示规则

### 列表页面规范

所有列表页面（菜单页、分类页、详情页等）必须遵循以下规范：

| 规则 | 说明 |
|------|------|
| **分页显示** | 每页显示 10 个条目 |
| **列表形式** | 使用表格列表展示，禁止使用网格卡片/按钮形态 |
| **分页控制** | 包含上一页/下一页按钮和页码导航 |

### 列表表格结构

```html
<!-- 表头 -->
<div class="list-header">
  <span>类型</span>
  <span>名称</span>
  <span>链接</span>
</div>

<!-- 数据行 -->
<a href="..." class="list-row">
  <span class="type">country</span>
  <span class="name">Argentina</span>
  <span class="url">/football/argentina/</span>
</a>
```

### 必要组件

| 组件 | 说明 |
|------|------|
| `stats-bar` | 显示统计信息（总数、各类型数量） |
| `pagination` | 分页导航（上一页、页码、下一页） |
| `list-table` | 列表表格容器 |

### 分页逻辑

```javascript
const ITEMS_PER_PAGE = 10;

function renderPage(page) {
  const totalPages = Math.ceil(list.length / ITEMS_PER_PAGE) || 1;
  const start = (page - 1) * ITEMS_PER_PAGE;
  const end = start + ITEMS_PER_PAGE;
  const pageItems = list.slice(start, end);
  // 渲染当前页数据
  // 渲染分页控件
}
```

### CSS 样式要求

- 列表行使用 `grid` 布局
- 鼠标悬停时高亮当前行 (`background: #f8fafc`)
- 奇偶行可使用不同背景色区分
- 分页按钮悬停时变蓝色
- 当前页码使用蓝色背景突出显示

### 适用页面

| 页面 | 路径 |
|------|------|
| 体育菜单 | `/menu` |
| 足球分类 | `/menu/football` |
| 其他体育分类 | `/menu/{sport}` |
| 未来所有列表页面 | - |

---

*页面显示规则添加时间：2026-06-07*
