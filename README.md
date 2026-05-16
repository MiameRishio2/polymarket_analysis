# Polymarket 交易记录分析工具 (Go 版本)

这是一个用 Go 语言编写的工具，用于爬取和分析 Polymarket 用户的交易记录。

## 功能特性

1. **数据爬取**
   - 获取用户个人资料
   - 获取用户持仓信息
   - 获取市场列表
   - 数据自动保存为 CSV 文件

2. **数据分析**
   - 基本统计信息
   - 交易量排名
   - 盈亏分析
   - 交易策略分析建议

## 编译和安装

```bash
cd polymarket_analysis
go build -o polymarket-analysis main.go
```

## 使用方法

### 1. 获取用户数据

```bash
./polymarket-analysis --username bonereaper
```

### 2. 获取市场列表

```bash
./polymarket-analysis --markets
```

## 项目结构

```
polymarket_analysis/
├── main.go              # 主程序
├── go.mod               # Go 模块文件
└── README.md           # 说明文档
```

## 核心结构体

### UserProfile
用户个人资料结构
- `User.ID`: 用户 ID
- `User.Name`: 用户名

### Market
市场信息结构
- `ID`: 市场 ID
- `Title`: 市场标题
- `Description`: 市场描述
- `Volume`: 交易量
- `Active`: 是否活跃

### Position
持仓信息结构
- `ID`: 持仓 ID
- `UserID`: 用户 ID
- `MarketID`: 市场 ID
- `TokenID`: 代币 ID
- `Size`: 持仓数量
- `Value`: 持仓价值
- `CostBasis`: 成本基础
- `RealizedPnL`: 已实现盈亏
- `UnrealizedPnL`: 未实现盈亏

## 核心函数

### NewPolymarketScraper()
创建 Polymarket 爬虫实例

### GetUserProfile(username)
获取用户个人资料

### GetMarkets(limit)
获取市场列表

### GetPositions(userID)
获取用户持仓

### SaveToCSV(filename, data)
保存数据到 CSV 文件

### AnalyzeMarkets(markets)
分析市场数据

### AnalyzePositions(positions)
分析持仓数据

## 交易策略分析方向

获取足够数据后，可以从以下维度分析交易策略：

1. **交易频率分析**
   - 每日/每周交易次数统计
   - 交易活跃度变化趋势

2. **持仓时间分析**
   - 平均持仓时长
   - 持仓时间分布

3. **市场偏好分析**
   - 最常交易的市场类型
   - 偏好的政治/体育/其他类别

4. **盈亏表现分析**
   - 盈利/亏损交易比例
   - 平均盈亏金额

5. **风险偏好分析**
   - 单笔交易金额分布
   - 风险集中度

6. **时机选择分析**
   - 交易时间分布（一天中的时段）
   - 事件前后的交易行为

## 注意事项

- 需要网络连接访问 Polymarket API
- 请遵守 Polymarket 的使用条款
- 建议合理设置请求间隔，避免 API 限制
