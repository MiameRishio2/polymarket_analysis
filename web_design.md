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
| `top-nav` | 页面顶部导航，非根级 `/menu` 页面显示“返回上一级”按钮 |
| `stats-bar` | 统计信息（总数、各类型数量） |
| `pagination` | 分页导航 |
| `list-table` | 列表表格容器 |

### 顶部返回按钮

除 `/menu` 根页面外，每个菜单页面顶部必须显示“返回上一级”按钮。返回目标按本地 `/menu` 路径段回退一级，并去掉尾部斜杠：

| 当前页面 | 返回目标 |
|----------|----------|
| `/menu/football/` | `/menu` |
| `/menu/football/world/` | `/menu/football` |
| `/menu/football/world/world-championship-2026/` | `/menu/football/world` |

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

**三级分类列表（3 列）**：
```html
<div class="list-header category-grid">
  <span>类型</span>
  <span>名称</span>
  <span>链接</span>
</div>
<a href="/menu/football/world/world-championship-2026/" class="list-row category-grid">
  <span class="type league">league</span>
  <span class="name">World Championship 2026</span>
  <span class="url">/football/world/world-championship-2026/</span>
</a>
```

**四级分类列表（3 列）**：
```html
<div class="list-header category-grid">
  <span>类型</span>
  <span>名称</span>
  <span>链接</span>
</div>
<a href="/football/world/world-championship-2026/winner/" target="_blank" class="list-row category-grid">
  <span class="type league">league</span>
  <span class="name">Winner</span>
  <span class="url">/football/world/world-championship-2026/winner/</span>
</a>
```

**四级赛事列表（3 列）**：当 `/api/events/{sport}/{category}/{league}` 返回赛事数据时，四级页面优先显示赛事表格；无赛事数据时回退四级分类列表。
```html
<div class="list-header event-grid">
  <span>比赛</span>
  <span>开始时间</span>
  <span>链接</span>
</div>
<a href="/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2" target="_blank" class="list-row event-grid">
  <span class="name">Mexico VS South Africa</span>
  <span class="time">18 Jun 2026, 03:00</span>
  <span class="url">/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2</span>
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

#### GET /api/menu/:sport/:category

获取指定体育分类下的三级子分类列表（从 `/{sport}/{category}/` 页面提取）。

**请求**：
```
GET /api/menu/football/argentina
```

**响应**：
```json
{
  "ok": true,
  "data": {
    "sport": "football/argentina",
    "categories": [
      {
        "slug": "primera-nacional",
        "name": "Primera Nacional",
        "url": "/football/argentina/primera-nacional/",
        "category_type": "league"
      }
    ],
    "last_updated": "2026-06-08T00:00:00Z",
    "source": "scraped"
  },
  "error": null
}
```

**字段说明**：
| 字段 | 类型 | 说明 |
|------|------|------|
| `sport` | string | 三级路径标识，如 `"football/argentina"` |
| `categories` | array | 子分类数组 |
| `categories[].slug` | string | 子分类标识符，如 `primera-nacional` |
| `categories[].name` | string | 子分类名称（首字母大写，- 替换为空格） |
| `categories[].url` | string | OddsPortal 路径，如 `/football/argentina/primera-nacional/` |
| `categories[].category_type` | string | 三级页面默认使用 `league` |

**提取规则**：
- 抓取 URL：`https://www.oddsportal.com/{sport}/{category}/`
- 仅保留直接子路径：`/{sport}/{category}/{child}/`
- 排除更深路径：`/{sport}/{category}/{child}/results/`
- 排除无关路径：其他 sport/category 下的链接

---

#### POST /api/menu/:sport/:category/refresh

强制刷新指定三级分类列表。

**请求**：
```
POST /api/menu/football/argentina/refresh
```

**响应**：同 GET /api/menu/:sport/:category

---

#### GET /api/menu/:sport/:category/:league

获取指定三级分类下的四级子分类列表（从 `/{sport}/{category}/{league}/` 页面提取）。

**请求**：
```
GET /api/menu/football/world/world-championship-2026
```

**响应**：
```json
{
  "ok": true,
  "data": {
    "sport": "football/world/world-championship-2026",
    "categories": [
      {
        "slug": "winner",
        "name": "Winner",
        "url": "/football/world/world-championship-2026/winner/",
        "category_type": "league"
      }
    ],
    "last_updated": "2026-06-09T00:00:00Z",
    "source": "scraped"
  },
  "error": null
}
```

**提取规则**：
- 抓取 URL：`https://www.oddsportal.com/{sport}/{category}/{league}/`
- 仅保留直接子路径：`/{sport}/{category}/{league}/{child}/`
- 排除更深路径和 results/standings/live/archive 等非分类页面

#### POST /api/menu/:sport/:category/:league/refresh

强制刷新指定四级分类列表。

**请求**：
```
POST /api/menu/football/world/world-championship-2026/refresh
```

**响应**：同 GET /api/menu/:sport/:category/:league

---

#### GET /api/events/:sport/:category/:league

获取指定四级赛事页的比赛列表（从 `/{sport}/{category}/{league}/` 页面提取 H2H/赛事链接）。

**请求**：
```
GET /api/events/football/world/world-championship-2026
```

**响应**：
```json
{
  "ok": true,
  "data": {
    "sport": "football/world/world-championship-2026",
    "events": [
      {
        "slug": "mexico-vs-south-africa",
        "home_team": "Mexico",
        "away_team": "South Africa",
        "matchup": "Mexico VS South Africa",
        "start_time": "18 Jun 2026, 03:00",
        "url": "/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2"
      }
    ],
    "last_updated": "2026-06-10T00:00:00Z",
    "source": "scraped"
  },
  "error": null
}
```

#### POST /api/events/:sport/:category/:league/refresh

强制刷新指定四级赛事列表。

**请求**：
```
POST /api/events/football/world/world-championship-2026/refresh
```

**响应**：同 GET /api/events/:sport/:category/:league

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
| `/menu/{sport}/{category}` | category-grid | 三级分类（3列：type, name, url） |
| `/menu/{sport}/{category}/{league}` | event-grid/category-grid | 四级赛事（3列：matchup, start_time, url）优先；无赛事时回退四级分类 |

页面路由必须兼容尾部斜杠，例如 `/menu/football/world/` 与 `/menu/football/world` 均应渲染同一三级页面。

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

**三级分类列表**（/api/menu/:sport/:category）：
- 抓取 URL：`https://www.oddsportal.com/{sport}/{category}/`
- 解析方式：提取所有 `href="/{sport}/{category}/{child}/"` 的链接
- 示例：`href="/football/world/world-championship-2026/"`
- 过滤条件：仅保留 category 下的一层直接子路径

**四级分类列表**（/api/menu/:sport/:category/:league）：
- 抓取 URL：`https://www.oddsportal.com/{sport}/{category}/{league}/`
- 解析方式：提取所有 `href="/{sport}/{category}/{league}/{child}/"` 的链接
- 过滤条件：仅保留 league 下的一层直接子路径

---

*更新日期：2026-06-10*
