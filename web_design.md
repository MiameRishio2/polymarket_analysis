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
| region | `.type.region` | 青色 (#cffafe) |
| popular | `.type.popular` | 橙色 (#fed7aa) |
| league | `.type.league` | 紫色 (#f3e8ff) |
| other | `.type.other` | 灰色 (#f1f5f9) |

---

## API 接口规范

### Menu API

#### GET /api/menu

获取体育项目列表（从 OddsPortal 首页 `sport-data` 提取）

**请求**：
```
GET /api/menu
```

**响应**：
```json
{
  "ok": true,
  "data": {
    "sport": "menu",
    "categories": [
      {
        "slug": "football",
        "name": "Football",
        "url": "/football/",
        "category_type": null
      },
      {
        "slug": "basketball",
        "name": "Basketball",
        "url": "/basketball/",
        "category_type": null
      }
      // ... 21 个体育项目
    ],
    "last_updated": "2026-06-07T14:50:27.417557+00:00",
    "source": "scraped"
  },
  "error": null
}
```

**字段说明**：
| 字段 | 类型 | 说明 |
|------|------|------|
| `sport` | string | 固定值 `"menu"` |
| `categories` | array | 体育项目数组 |
| `categories[].slug` | string | 体育标识符，如 `football`, `basketball` |
| `categories[].name` | string | 体育名称（首字母大写） |
| `categories[].url` | string | 路径，如 `/football/` |
| `categories[].category_type` | null | 固定 `null` |

---

#### GET /api/menu/:sport

获取指定体育的分类列表（从 `/football/` 页面提取）

**请求**：
```
GET /api/menu/football
GET /api/menu/basketball
GET /api/menu/tennis
```

**响应**：
```json
{
  "ok": true,
  "data": {
    "sport": "football",
    "categories": [
      {
        "slug": "argentina",
        "name": "Argentina",
        "url": "/football/argentina/",
        "category_type": "country"
      },
      {
        "slug": "asia",
        "name": "Asia",
        "url": "/football/asia/",
        "category_type": "region"
      },
      {
        "slug": "europe",
        "name": "Europe",
        "url": "/football/europe/",
        "category_type": "region"
      }
      // ... 50 个分类（排除 results, standings 等页面）
    ],
    "last_updated": "2026-06-07T14:27:28.335984+00:00",
    "source": "scraped"
  },
  "error": null
}
```

**字段说明**：
| 字段 | 类型 | 说明 |
|------|------|------|
| `sport` | string | 体育标识符，如 `"football"` |
| `categories` | array | 分类数组 |
| `categories[].slug` | string | 分类标识符，如 `england`, `asia`, `champions-league` |
| `categories[].name` | string | 分类名称（首字母大写，- 替换为空格） |
| `categories[].url` | string | 完整路径，如 `/football/england/` |
| `categories[].category_type` | string | 分类类型：`country`, `region`, `league`, `category` |

**分类类型规则**：
| 类型 | 条件 |
|------|------|
| `country` | 常见国家名（england, argentina, japan 等） |
| `region` | 大洲/地区（asia, europe, africa, world 等） |
| `league` | 联赛关键词（champions-league, premier-league, euro, world-cup 等） |
| `category` | 其他未匹配的分类 |

**排除规则**：
以下路径不返回：
- `results` - 结果页面
- `standings` - 排名页面
- `live` - 直播页面
- `archive` - 存档页面

---

#### POST /api/menu/refresh

强制刷新体育列表（从 OddsPortal 重新抓取）

**请求**：
```
POST /api/menu/refresh
```

**响应**：同 GET /api/menu

---

#### POST /api/menu/:sport/refresh

强制刷新指定体育的分类列表

**请求**：
```
POST /api/menu/football/refresh
```

**响应**：同 GET /api/menu/:sport

---

## 分页逻辑

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
| `/menu` | sports-grid | 体育菜单（2列：slug, name） |
| `/menu/{sport}` | category-grid | 体育分类（3列：type, name, url） |

---

## 数据来源

### OddsPortal 抓取规则

**体育列表**（/api/menu）：
- 抓取 URL：`https://www.oddsportal.com/`
- 解析位置：HTML 中的 `sport-data="{...}"`
- 数据格式：JSON 对象，键如 `S_1`, `S_2`...

**分类列表**（/api/menu/:sport）：
- 抓取 URL：`https://www.oddsportal.com/{sport}/`
- 解析方式：提取所有 `href="/{sport}/{slug}/"` 的链接
- 过滤条件：仅保留单层路径（不含第二级斜杠）

---

*更新日期：2026-06-07*
