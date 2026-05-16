package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// BookmakerOdds 表示单个博彩公司的赔率
type BookmakerOdds struct {
	Bookmaker string  `json:"bookmaker"`
	Home      float64 `json:"home"`
	Draw      float64 `json:"draw"`
	Away      float64 `json:"away"`
}

// MatchData 表示一场比赛的完整数据
type MatchData struct {
	MatchID    string         `json:"match_id"`
	HomeTeam   string         `json:"home_team"`
	AwayTeam   string         `json:"away_team"`
	MatchTime  time.Time      `json:"match_time"`
	Polymarket PolymarketData `json:"polymarket"`
	OddsPortal OddsPortalData `json:"oddsportal"`
	DataPoints []DataPoint    `json:"data_points"`
}

// PolymarketData 表示Polymarket的市场数据
type PolymarketData struct {
	MarketID    string    `json:"market_id"`
	Title       string    `json:"title"`
	Volume      float64   `json:"volume"`
	Active      bool      `json:"active"`
	YesPrice    float64   `json:"yes_price"`
	NoPrice     float64   `json:"no_price"`
	LastUpdated time.Time `json:"last_updated"`
}

// OddsPortalData 表示OddsPortal的赔率数据
type OddsPortalData struct {
	MatchURL    string          `json:"match_url"`
	Bookmakers  []BookmakerOdds `json:"bookmakers"`
	AverageHome float64         `json:"average_home"`
	AverageDraw float64         `json:"average_draw"`
	AverageAway float64         `json:"average_away"`
	LastUpdated time.Time       `json:"last_updated"`
}

// DataPoint 表示一个时间点的数据
type DataPoint struct {
	Timestamp     time.Time `json:"timestamp"`
	PolymarketYes float64   `json:"polymarket_yes"`
	PolymarketNo  float64   `json:"polymarket_no"`
	OddsHome      float64   `json:"odds_home"`
	OddsDraw      float64   `json:"odds_draw"`
	OddsAway      float64   `json:"odds_away"`
}

// DataCache 用于缓存比赛数据
type DataCache struct {
	matches map[string]*MatchData
}

// NewDataCache 创建一个新的数据缓存
func NewDataCache() *DataCache {
	return &DataCache{
		matches: make(map[string]*MatchData),
	}
}

// AddMatch 添加比赛数据到缓存
func (c *DataCache) AddMatch(match *MatchData) {
	if existingMatch, exists := c.matches[match.MatchID]; exists {
		// 更新现有比赛的数据
		existingMatch.Polymarket = match.Polymarket
		existingMatch.OddsPortal = match.OddsPortal
		existingMatch.DataPoints = append(existingMatch.DataPoints, match.DataPoints...)
	} else {
		// 添加新比赛
		c.matches[match.MatchID] = match
	}
}

// GetMatch 从缓存中获取比赛数据
func (c *DataCache) GetMatch(matchID string) (*MatchData, bool) {
	match, exists := c.matches[matchID]
	return match, exists
}

// PolymarketScraper 用于从Polymarket获取数据
type PolymarketScraper struct {
	proxy string
}

// NewPolymarketScraper 创建一个新的PolymarketScraper
func NewPolymarketScraper(proxy string) *PolymarketScraper {
	return &PolymarketScraper{
		proxy: proxy,
	}
}

// GetMarketData 获取Polymarket市场数据
func (s *PolymarketScraper) GetMarketData(marketID string) (*PolymarketData, error) {
	// 这里模拟Polymarket数据，实际项目中应该从API获取
	// 模拟数据生成
	baseYesPrice := 0.65
	baseNoPrice := 0.35

	// 添加一些随机波动
	fluctuation := (float64(time.Now().UnixNano()%10) - 5) / 100.0
	yesPrice := baseYesPrice + fluctuation
	noPrice := baseNoPrice - fluctuation

	// 确保价格在合理范围内
	if yesPrice < 0.01 {
		yesPrice = 0.01
	}
	if noPrice < 0.01 {
		noPrice = 0.01
	}
	if yesPrice > 0.99 {
		yesPrice = 0.99
	}
	if noPrice > 0.99 {
		noPrice = 0.99
	}

	return &PolymarketData{
		MarketID:    marketID,
		Title:       "Example Market",
		Volume:      12500.75,
		Active:      true,
		YesPrice:    yesPrice,
		NoPrice:     noPrice,
		LastUpdated: time.Now(),
	}, nil
}

// OddsPortalScraper 用于从OddsPortal获取数据
type OddsPortalScraper struct {
	proxy string
}

// NewOddsPortalScraper 创建一个新的OddsPortalScraper
func NewOddsPortalScraper(proxy string) *OddsPortalScraper {
	return &OddsPortalScraper{
		proxy: proxy,
	}
}

// GetMatchOdds 获取比赛赔率数据
func (s *OddsPortalScraper) GetMatchOdds(url string) ([]BookmakerOdds, error) {
	// 创建HTTP客户端
	client := &http.Client{
		Timeout: 30 * time.Second,
	}

	// 创建请求
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("创建请求失败: %v", err)
	}

	// 添加请求头
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
	req.Header.Set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
	req.Header.Set("Accept-Language", "zh-CN,zh;q=0.8,en-US;q=0.5,en;q=0.3")
	req.Header.Set("Connection", "keep-alive")
	req.Header.Set("Upgrade-Insecure-Requests", "1")

	// 发送请求
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("发送请求失败: %v", err)
	}
	defer resp.Body.Close()

	// 读取响应
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("读取响应失败: %v", err)
	}

	// 解析HTML获取赔率数据
	return s.parseOddsHTML(string(body))
}

// parseOddsHTML 解析HTML获取赔率数据
func (s *OddsPortalScraper) parseOddsHTML(html string) ([]BookmakerOdds, error) {
	// 提取博彩公司和赔率的正则表达式
	bookmakerRegex := regexp.MustCompile(`class="bookmaker-name".*?>(.*?)</a>`)
	oddsRegex := regexp.MustCompile(`data-odd="([\d.]+)"`)

	// 提取博彩公司
	bookmakerMatches := bookmakerRegex.FindAllStringSubmatch(html, -1)
	if len(bookmakerMatches) == 0 {
		return nil, fmt.Errorf("未找到博彩公司数据")
	}

	// 提取赔率
	oddsMatches := oddsRegex.FindAllStringSubmatch(html, -1)
	if len(oddsMatches) == 0 {
		return nil, fmt.Errorf("未找到赔率数据")
	}

	// 确保赔率数据足够
	if len(oddsMatches) < len(bookmakerMatches)*3 {
		return nil, fmt.Errorf("赔率数据不完整")
	}

	// 构建赔率数据
	var bookmakers []BookmakerOdds
	for i, bookmakerMatch := range bookmakerMatches {
		if i*3+2 >= len(oddsMatches) {
			break
		}

		home, err := strconv.ParseFloat(oddsMatches[i*3][1], 64)
		if err != nil {
			continue
		}

		draw, err := strconv.ParseFloat(oddsMatches[i*3+1][1], 64)
		if err != nil {
			continue
		}

		away, err := strconv.ParseFloat(oddsMatches[i*3+2][1], 64)
		if err != nil {
			continue
		}

		bookmakers = append(bookmakers, BookmakerOdds{
			Bookmaker: bookmakerMatch[1],
			Home:      home,
			Draw:      draw,
			Away:      away,
		})
	}

	if len(bookmakers) == 0 {
		return nil, fmt.Errorf("解析赔率数据失败")
	}

	return bookmakers, nil
}

// FetchMatchData 获取一场比赛的数据
func FetchMatchData(polymarketScraper *PolymarketScraper, oddsScraper *OddsPortalScraper, matchID, marketID, matchURL, homeTeam, awayTeam string) (*MatchData, error) {
	// 获取Polymarket数据
	polymarketData, err := polymarketScraper.GetMarketData(marketID)
	if err != nil {
		return nil, fmt.Errorf("获取Polymarket数据失败: %v", err)
	}

	// 获取OddsPortal数据
	bookmakerOdds, err := oddsScraper.GetMatchOdds(matchURL)
	if err != nil {
		return nil, fmt.Errorf("获取OddsPortal数据失败: %v", err)
	}

	// 计算平均赔率
	var totalHome, totalDraw, totalAway float64
	for _, odds := range bookmakerOdds {
		totalHome += odds.Home
		totalDraw += odds.Draw
		totalAway += odds.Away
	}

	averageHome := totalHome / float64(len(bookmakerOdds))
	averageDraw := totalDraw / float64(len(bookmakerOdds))
	averageAway := totalAway / float64(len(bookmakerOdds))

	// 创建数据点
	dataPoint := DataPoint{
		Timestamp:     time.Now(),
		PolymarketYes: polymarketData.YesPrice,
		PolymarketNo:  polymarketData.NoPrice,
		OddsHome:      averageHome,
		OddsDraw:      averageDraw,
		OddsAway:      averageAway,
	}

	// 创建比赛数据
	matchData := &MatchData{
		MatchID:    matchID,
		HomeTeam:   homeTeam,
		AwayTeam:   awayTeam,
		MatchTime:  time.Now(),
		Polymarket: *polymarketData,
		OddsPortal: OddsPortalData{
			MatchURL:    matchURL,
			Bookmakers:  bookmakerOdds,
			AverageHome: averageHome,
			AverageDraw: averageDraw,
			AverageAway: averageAway,
			LastUpdated: time.Now(),
		},
		DataPoints: []DataPoint{dataPoint},
	}

	return matchData, nil
}

// CompareData 比较Polymarket和OddsPortal的数据
func CompareData(match *MatchData) {
	fmt.Println("\n" + "=" + repeatString("=", 80) + "=")
	fmt.Println("数据比较分析")
	fmt.Println("=" + repeatString("=", 80) + "=")
	fmt.Printf("比赛: %s vs %s\n", match.HomeTeam, match.AwayTeam)
	fmt.Printf("Polymarket 主胜概率: %.2f%%\n", match.Polymarket.YesPrice*100)
	fmt.Printf("Polymarket 客胜概率: %.2f%%\n", match.Polymarket.NoPrice*100)
	fmt.Printf("OddsPortal 主胜概率: %.2f%%\n", (1.0/match.OddsPortal.AverageHome)*100)
	fmt.Printf("OddsPortal 平局概率: %.2f%%\n", (1.0/match.OddsPortal.AverageDraw)*100)
	fmt.Printf("OddsPortal 客胜概率: %.2f%%\n", (1.0/match.OddsPortal.AverageAway)*100)

	// 计算差异
	polymarketHomeProb := match.Polymarket.YesPrice * 100
	oddsportalHomeProb := (1.0 / match.OddsPortal.AverageHome) * 100
	homeDiff := polymarketHomeProb - oddsportalHomeProb

	polymarketAwayProb := match.Polymarket.NoPrice * 100
	oddsportalAwayProb := (1.0 / match.OddsPortal.AverageAway) * 100
	awayDiff := polymarketAwayProb - oddsportalAwayProb

	fmt.Printf("\n差异分析:\n")
	fmt.Printf("主胜概率差异: %.2f%%\n", homeDiff)
	fmt.Printf("客胜概率差异: %.2f%%\n", awayDiff)

	// 分析结果
	if homeDiff > 5 {
		fmt.Println("Polymarket 对主胜的信心明显高于 OddsPortal")
	} else if homeDiff < -5 {
		fmt.Println("OddsPortal 对主胜的信心明显高于 Polymarket")
	}

	if awayDiff > 5 {
		fmt.Println("Polymarket 对客胜的信心明显高于 OddsPortal")
	} else if awayDiff < -5 {
		fmt.Println("OddsPortal 对客胜的信心明显高于 Polymarket")
	}
}

// GenerateVisualization 生成可视化HTML
func GenerateVisualization(match *MatchData, filename string) error {
	// 生成HTML内容
	htmlContent := generateVisualizationHTML(match)

	// 写入文件
	return os.WriteFile(filename, []byte(htmlContent), 0644)
}

// generateVisualizationHTML 生成可视化HTML
func generateVisualizationHTML(match *MatchData) string {
	var htmlContent strings.Builder

	htmlContent.WriteString(`
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>赔率对比分析 - `)
	htmlContent.WriteString(match.HomeTeam)
	htmlContent.WriteString(" vs ")
	htmlContent.WriteString(match.AwayTeam)
	htmlContent.WriteString(`</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body {
            font-family: Arial, sans-serif;
            margin: 20px;
            background-color: #f5f5f5;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background-color: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1 {
            text-align: center;
            color: #333;
        }
        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin: 30px 0;
        }
        .stat-card {
            background-color: #f9f9f9;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1);
        }
        .stat-card h2 {
            margin-top: 0;
            color: #555;
            font-size: 18px;
        }
        .stat-item {
            display: flex;
            justify-content: space-between;
            margin: 10px 0;
            padding: 8px 0;
            border-bottom: 1px solid #eee;
        }
        .stat-item:last-child {
            border-bottom: none;
        }
        .stat-label {
            font-weight: bold;
            color: #666;
        }
        .stat-value {
            color: #333;
        }
        .chart-container {
            margin: 30px 0;
            height: 400px;
        }
        .footer {
            text-align: center;
            margin-top: 30px;
            padding-top: 20px;
            border-top: 1px solid #eee;
            color: #999;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>赔率对比分析 - `)
	htmlContent.WriteString(match.HomeTeam)
	htmlContent.WriteString(" vs ")
	htmlContent.WriteString(match.AwayTeam)
	htmlContent.WriteString(`</h1>
        
        <div class="stats">
            <div class="stat-card">
                <h2>Polymarket 数据</h2>
                <div class="stat-item">
                    <span class="stat-label">市场名称:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(match.Polymarket.Title)
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">交易量:</span>
                    <span class="stat-value">$`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.Polymarket.Volume))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">主胜概率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f%%", match.Polymarket.YesPrice*100))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">客胜概率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f%%", match.Polymarket.NoPrice*100))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">最后更新:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(match.Polymarket.LastUpdated.Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`</span>
                </div>
            </div>
            
            <div class="stat-card">
                <h2>OddsPortal 数据</h2>
                <div class="stat-item">
                    <span class="stat-label">比赛URL:</span>
                    <span class="stat-value"><a href="`)
	htmlContent.WriteString(match.OddsPortal.MatchURL)
	htmlContent.WriteString(`" target="_blank">查看</a></span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">平均主胜赔率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageHome))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">平均平局赔率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageDraw))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">平均客胜赔率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageAway))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">最后更新:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(match.OddsPortal.LastUpdated.Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`</span>
                </div>
            </div>
        </div>
        
        <div class="chart-container">
            <canvas id="probabilityChart"></canvas>
        </div>
        
        <div class="chart-container">
            <canvas id="oddsChart"></canvas>
        </div>
        
        <div class="footer">
            <p>分析生成时间: `)
	htmlContent.WriteString(time.Now().Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`</p>
        </div>
    </div>
    
    <script>
        // 创建概率对比图表
        const probCtx = document.getElementById('probabilityChart').getContext('2d');
        const probChart = new Chart(probCtx, {
            type: 'bar',
            data: {
                labels: ['主胜', '平局', '客胜'],
                datasets: [
                    {
                        label: 'Polymarket',
                        data: [`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.Polymarket.YesPrice*100))
	htmlContent.WriteString(`, 0, `)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.Polymarket.NoPrice*100))
	htmlContent.WriteString(`],
                        backgroundColor: 'rgba(75, 192, 192, 0.6)',
                        borderColor: 'rgba(75, 192, 192, 1)',
                        borderWidth: 1
                    },
                    {
                        label: 'OddsPortal',
                        data: [`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", (1.0/match.OddsPortal.AverageHome)*100))
	htmlContent.WriteString(`, `)
	htmlContent.WriteString(fmt.Sprintf("%.2f", (1.0/match.OddsPortal.AverageDraw)*100))
	htmlContent.WriteString(`, `)
	htmlContent.WriteString(fmt.Sprintf("%.2f", (1.0/match.OddsPortal.AverageAway)*100))
	htmlContent.WriteString(`],
                        backgroundColor: 'rgba(54, 162, 235, 0.6)',
                        borderColor: 'rgba(54, 162, 235, 1)',
                        borderWidth: 1
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        max: 100,
                        title: {
                            display: true,
                            text: '概率 (%)'
                        }
                    }
                },
                plugins: {
                    legend: {
                        position: 'top',
                    },
                    title: {
                        display: true,
                        text: '概率对比'
                    }
                }
            }
        });
        
        // 创建赔率对比图表
        const oddsCtx = document.getElementById('oddsChart').getContext('2d');
        const oddsChart = new Chart(oddsCtx, {
            type: 'bar',
            data: {
                labels: ['主胜', '平局', '客胜'],
                datasets: [
                    {
                        label: 'OddsPortal 赔率',
                        data: [`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageHome))
	htmlContent.WriteString(`, `)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageDraw))
	htmlContent.WriteString(`, `)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageAway))
	htmlContent.WriteString(`],
                        backgroundColor: 'rgba(255, 99, 132, 0.6)',
                        borderColor: 'rgba(255, 99, 132, 1)',
                        borderWidth: 1
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: false,
                        title: {
                            display: true,
                            text: '赔率'
                        }
                    }
                },
                plugins: {
                    legend: {
                        position: 'top',
                    },
                    title: {
                        display: true,
                        text: 'OddsPortal 赔率'
                    }
                }
            }
        });
    </script>
</body>
</html>
`)

	return htmlContent.String()
}

// SaveMatchData 保存比赛数据到文件
func SaveMatchData(match *MatchData, filename string) error {
	// 将数据转换为JSON
	data, err := json.MarshalIndent(match, "", "  ")
	if err != nil {
		return fmt.Errorf("序列化数据失败: %v", err)
	}

	// 写入文件
	return os.WriteFile(filename, data, 0644)
}

// repeatString 重复字符串
func repeatString(s string, n int) string {
	var result strings.Builder
	for i := 0; i < n; i++ {
		result.WriteString(s)
	}
	return result.String()
}

// GetRealTimeOdds 获取实时胜率
func GetRealTimeOdds(homeTeam, awayTeam, proxy string) (*MatchData, error) {
	// 创建爬虫实例
	polymarketScraper := NewPolymarketScraper(proxy)
	oddsScraper := NewOddsPortalScraper(proxy)

	// 生成匹配的比赛ID和市场ID
	matchID := strings.ToLower(strings.ReplaceAll(homeTeam, " ", "_")) + "_vs_" + strings.ToLower(strings.ReplaceAll(awayTeam, " ", "_"))
	marketID := "market_" + matchID

	// 生成模拟的比赛URL（实际项目中应该根据队伍名称搜索）
	matchURL := "https://www.oddsportal.com/football/h2h/" + strings.ToLower(strings.ReplaceAll(homeTeam, " ", "-")) + "/" + strings.ToLower(strings.ReplaceAll(awayTeam, " ", "-")) + "/"

	// 获取比赛数据
	matchData, err := FetchMatchData(polymarketScraper, oddsScraper, matchID, marketID, matchURL, homeTeam, awayTeam)
	if err != nil {
		return nil, err
	}

	return matchData, nil
}

// StartContinuousDataCollection 连续获取数据并实时显示
func StartContinuousDataCollection(homeTeam, awayTeam, proxy string, interval int) error {
	// 创建爬虫实例
	polymarketScraper := NewPolymarketScraper(proxy)
	oddsScraper := NewOddsPortalScraper(proxy)

	// 生成匹配的比赛ID和市场ID
	matchID := strings.ToLower(strings.ReplaceAll(homeTeam, " ", "_")) + "_vs_" + strings.ToLower(strings.ReplaceAll(awayTeam, " ", "_"))
	marketID := "market_" + matchID

	// 生成模拟的比赛URL（实际项目中应该根据队伍名称搜索）
	matchURL := "https://www.oddsportal.com/football/h2h/" + strings.ToLower(strings.ReplaceAll(homeTeam, " ", "-")) + "/" + strings.ToLower(strings.ReplaceAll(awayTeam, " ", "-")) + "/"

	// 创建数据缓存
	cache := NewDataCache()

	// 初始化比赛数据
	matchData, err := FetchMatchData(polymarketScraper, oddsScraper, matchID, marketID, matchURL, homeTeam, awayTeam)
	if err != nil {
		fmt.Printf("获取真实数据失败，使用模拟数据: %v\n", err)
		// 使用模拟数据
		matchData = generateMockMatchData(homeTeam, awayTeam, matchID, marketID, matchURL)
	}
	cache.AddMatch(matchData)

	// 启动HTTP服务器
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		// 获取最新的比赛数据
		match, exists := cache.GetMatch(matchID)
		if !exists {
			http.Error(w, "未找到比赛数据", http.StatusInternalServerError)
			return
		}

		// 生成实时HTML
		htmlContent := generateRealTimeHTML(match)

		// 发送响应
		w.Header().Set("Content-Type", "text/html")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(htmlContent))
	})

	// 启动定时任务，定期获取数据
	ticker := time.NewTicker(time.Duration(interval) * time.Second)
	go func() {
		for range ticker.C {
			// 获取最新数据
			newMatchData, err := FetchMatchData(polymarketScraper, oddsScraper, matchID, marketID, matchURL, homeTeam, awayTeam)
			if err != nil {
				fmt.Printf("获取数据失败: %v\n", err)
				continue
			}

			// 更新缓存
			cache.AddMatch(newMatchData)
			fmt.Printf("数据已更新: %s\n", time.Now().Format("2006-01-02 15:04:05"))
		}
	}()

	// 启动服务器
	fmt.Printf("服务器已启动，访问 http://localhost:8081 查看实时赔率\n")
	return http.ListenAndServe(":8081", nil)
}

// generateRealTimeHTML 生成实时显示的HTML
func generateRealTimeHTML(match *MatchData) string {
	var htmlContent strings.Builder

	// 构建数据数组
	var timestamps []string
	var polymarketHome []float64
	var polymarketAway []float64
	var oddsHome []float64
	var oddsDraw []float64
	var oddsAway []float64

	for _, dataPoint := range match.DataPoints {
		timestamps = append(timestamps, dataPoint.Timestamp.Format("15:04:05"))
		polymarketHome = append(polymarketHome, dataPoint.PolymarketYes*100)
		polymarketAway = append(polymarketAway, dataPoint.PolymarketNo*100)
		oddsHome = append(oddsHome, (1.0/dataPoint.OddsHome)*100)
		oddsDraw = append(oddsDraw, (1.0/dataPoint.OddsDraw)*100)
		oddsAway = append(oddsAway, (1.0/dataPoint.OddsAway)*100)
	}

	// 生成JavaScript数组字符串
	timestampsStr := "['" + strings.Join(timestamps, "', '") + "']"
	polymarketHomeStr := "[" + formatFloatSlice(polymarketHome) + "]"
	polymarketAwayStr := "[" + formatFloatSlice(polymarketAway) + "]"
	tsHomeStr := "[" + formatFloatSlice(oddsHome) + "]"
	tsDrawStr := "[" + formatFloatSlice(oddsDraw) + "]"
	tsAwayStr := "[" + formatFloatSlice(oddsAway) + "]"

	// 生成数据点JSON
	var dataPointsJSON []string
	for _, dataPoint := range match.DataPoints {
		dataPointJSON := fmt.Sprintf(`{
                        timestamp: "%s",
                        polymarketHome: %.2f,
                        polymarketAway: %.2f,
                        oddsHome: %.2f,
                        oddsDraw: %.2f,
                        oddsAway: %.2f
                    }`,
			dataPoint.Timestamp.Format("2006-01-02 15:04:05"),
			dataPoint.PolymarketYes*100,
			dataPoint.PolymarketNo*100,
			(1.0/dataPoint.OddsHome)*100,
			(1.0/dataPoint.OddsDraw)*100,
			(1.0/dataPoint.OddsAway)*100)
		dataPointsJSON = append(dataPointsJSON, dataPointJSON)
	}
	dataPointsJSONStr := "[" + strings.Join(dataPointsJSON, ",") + "]"

	htmlContent.WriteString(`
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>实时赔率监控 - `)
	htmlContent.WriteString(match.HomeTeam)
	htmlContent.WriteString(" vs ")
	htmlContent.WriteString(match.AwayTeam)
	htmlContent.WriteString(`</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body {
            font-family: Arial, sans-serif;
            margin: 20px;
            background-color: #f5f5f5;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background-color: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1 {
            text-align: center;
            color: #333;
        }
        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin: 30px 0;
        }
        .stat-card {
            background-color: #f9f9f9;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1);
        }
        .stat-card h2 {
            margin-top: 0;
            color: #555;
            font-size: 18px;
        }
        .stat-item {
            display: flex;
            justify-content: space-between;
            margin: 10px 0;
            padding: 8px 0;
            border-bottom: 1px solid #eee;
        }
        .stat-item:last-child {
            border-bottom: none;
        }
        .stat-label {
            font-weight: bold;
            color: #666;
        }
        .stat-value {
            color: #333;
        }
        .chart-container {
            margin: 30px 0;
            height: 400px;
        }
        .footer {
            text-align: center;
            margin-top: 30px;
            padding-top: 20px;
            border-top: 1px solid #eee;
            color: #999;
        }
        .refresh-btn {
            display: block;
            margin: 20px auto;
            padding: 10px 20px;
            background-color: #4CAF50;
            color: white;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-size: 16px;
        }
        .refresh-btn:hover {
            background-color: #45a049;
        }
        .save-btn {
            display: block;
            margin: 20px auto;
            padding: 10px 20px;
            background-color: #2196F3;
            color: white;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-size: 16px;
        }
        .save-btn:hover {
            background-color: #0b7dda;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>实时赔率监控 - `)
	htmlContent.WriteString(match.HomeTeam)
	htmlContent.WriteString(" vs ")
	htmlContent.WriteString(match.AwayTeam)
	htmlContent.WriteString(`</h1>
        
        <button class="refresh-btn" onclick="location.reload()">刷新数据</button>
        <button class="save-btn" onclick="saveData()">保存数据</button>
        
        <div class="stats">
            <div class="stat-card">
                <h2>Polymarket 数据</h2>
                <div class="stat-item">
                    <span class="stat-label">市场名称:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(match.Polymarket.Title)
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">交易量:</span>
                    <span class="stat-value">$`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.Polymarket.Volume))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">主胜概率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f%%", match.Polymarket.YesPrice*100))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">客胜概率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f%%", match.Polymarket.NoPrice*100))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">最后更新:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(match.Polymarket.LastUpdated.Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`</span>
                </div>
            </div>
            
            <div class="stat-card">
                <h2>OddsPortal 数据</h2>
                <div class="stat-item">
                    <span class="stat-label">比赛URL:</span>
                    <span class="stat-value"><a href="`)
	htmlContent.WriteString(match.OddsPortal.MatchURL)
	htmlContent.WriteString(`" target="_blank">查看</a></span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">平均主胜赔率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageHome))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">平均平局赔率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageDraw))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">平均客胜赔率:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(fmt.Sprintf("%.2f", match.OddsPortal.AverageAway))
	htmlContent.WriteString(`</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">最后更新:</span>
                    <span class="stat-value">`)
	htmlContent.WriteString(match.OddsPortal.LastUpdated.Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`</span>
                </div>
            </div>
        </div>
        
        <div class="chart-container">
            <canvas id="oddsChart"></canvas>
        </div>
        
        <div class="footer">
            <p>数据更新时间: `)
	htmlContent.WriteString(time.Now().Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`</p>
        </div>
    </div>
    
    <script>
        // 准备数据
        const timestamps = `)
	htmlContent.WriteString(timestampsStr)
	htmlContent.WriteString(`;
        const polymarketHome = `)
	htmlContent.WriteString(polymarketHomeStr)
	htmlContent.WriteString(`;
        const polymarketAway = `)
	htmlContent.WriteString(polymarketAwayStr)
	htmlContent.WriteString(`;
        const oddsHome = `)
	htmlContent.WriteString(tsHomeStr)
	htmlContent.WriteString(`;
        const oddsDraw = `)
	htmlContent.WriteString(tsDrawStr)
	htmlContent.WriteString(`;
        const oddsAway = `)
	htmlContent.WriteString(tsAwayStr)
	htmlContent.WriteString(`;
        
        // 创建图表
        const ctx = document.getElementById('oddsChart').getContext('2d');
        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: timestamps,
                datasets: [
                    {
                        label: 'Polymarket 主胜',
                        data: polymarketHome,
                        borderColor: 'rgba(75, 192, 192, 1)',
                        backgroundColor: 'rgba(75, 192, 192, 0.2)',
                        tension: 0.1
                    },
                    {
                        label: 'Polymarket 客胜',
                        data: polymarketAway,
                        borderColor: 'rgba(153, 102, 255, 1)',
                        backgroundColor: 'rgba(153, 102, 255, 0.2)',
                        tension: 0.1
                    },
                    {
                        label: 'OddsPortal 主胜',
                        data: oddsHome,
                        borderColor: 'rgba(255, 99, 132, 1)',
                        backgroundColor: 'rgba(255, 99, 132, 0.2)',
                        tension: 0.1
                    },
                    {
                        label: 'OddsPortal 平局',
                        data: oddsDraw,
                        borderColor: 'rgba(255, 206, 86, 1)',
                        backgroundColor: 'rgba(255, 206, 86, 0.2)',
                        tension: 0.1
                    },
                    {
                        label: 'OddsPortal 客胜',
                        data: oddsAway,
                        borderColor: 'rgba(54, 162, 235, 1)',
                        backgroundColor: 'rgba(54, 162, 235, 0.2)',
                        tension: 0.1
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: false,
                        title: {
                            display: true,
                            text: '胜率 (%)'
                        }
                    },
                    x: {
                        title: {
                            display: true,
                            text: '时间'
                        }
                    }
                },
                plugins: {
                    legend: {
                        position: 'top',
                    },
                    title: {
                        display: true,
                        text: '赔率变化趋势'
                    }
                }
            }
        });
        
        // 保存数据函数
        function saveData() {
            const data = {
                match: {
                    homeTeam: "`)
	htmlContent.WriteString(match.HomeTeam)
	htmlContent.WriteString(`",
                    awayTeam: "`)
	htmlContent.WriteString(match.AwayTeam)
	htmlContent.WriteString(`",
                    timestamp: "`)
	htmlContent.WriteString(time.Now().Format("2006-01-02 15:04:05"))
	htmlContent.WriteString(`"
                },
                dataPoints: `)
	htmlContent.WriteString(dataPointsJSONStr)
	htmlContent.WriteString(`
            };
            
            // 创建下载链接
            const dataStr = JSON.stringify(data, null, 2);
            const dataBlob = new Blob([dataStr], {type: 'application/json'});
            const url = URL.createObjectURL(dataBlob);
            const link = document.createElement('a');
            link.href = url;
            link.download = 'odds_data_`)
	htmlContent.WriteString(strings.ToLower(strings.ReplaceAll(match.HomeTeam, " ", "_")))
	htmlContent.WriteString("_")
	htmlContent.WriteString(strings.ToLower(strings.ReplaceAll(match.AwayTeam, " ", "_")))
	htmlContent.WriteString("_")
	htmlContent.WriteString(time.Now().Format("20060102_150405"))
	htmlContent.WriteString(`.json');
            link.click();
            URL.revokeObjectURL(url);
        }
    </script>
</body>
</html>
`)

	return htmlContent.String()
}

// formatFloatSlice 将float64切片格式化为逗号分隔的字符串
func formatFloatSlice(slice []float64) string {
	var parts []string
	for _, val := range slice {
		parts = append(parts, fmt.Sprintf("%.2f", val))
	}
	return strings.Join(parts, ",")
}

// generateMockMatchData 生成模拟比赛数据
func generateMockMatchData(homeTeam, awayTeam, matchID, marketID, matchURL string) *MatchData {
	// 生成模拟的Polymarket数据
	polymarketData := &PolymarketData{
		MarketID:    marketID,
		Title:       homeTeam + " vs " + awayTeam,
		Volume:      12500.75,
		Active:      true,
		YesPrice:    0.65,
		NoPrice:     0.35,
		LastUpdated: time.Now(),
	}

	// 生成模拟的OddsPortal数据
	bookmakers := []BookmakerOdds{
		{Bookmaker: "1xBet", Home: 2.24, Draw: 3.24, Away: 3.21},
		{Bookmaker: "22Bet", Home: 2.24, Draw: 3.24, Away: 3.21},
		{Bookmaker: "888sport", Home: 2.20, Draw: 3.10, Away: 3.10},
		{Bookmaker: "bet365", Home: 2.20, Draw: 3.25, Away: 3.25},
		{Bookmaker: "Betfury", Home: 2.20, Draw: 3.20, Away: 3.20},
	}

	// 计算平均赔率
	var totalHome, totalDraw, totalAway float64
	for _, odds := range bookmakers {
		totalHome += odds.Home
		totalDraw += odds.Draw
		totalAway += odds.Away
	}

	averageHome := totalHome / float64(len(bookmakers))
	averageDraw := totalDraw / float64(len(bookmakers))
	averageAway := totalAway / float64(len(bookmakers))

	// 创建数据点
	dataPoint := DataPoint{
		Timestamp:     time.Now(),
		PolymarketYes: polymarketData.YesPrice,
		PolymarketNo:  polymarketData.NoPrice,
		OddsHome:      averageHome,
		OddsDraw:      averageDraw,
		OddsAway:      averageAway,
	}

	// 创建比赛数据
	matchData := &MatchData{
		MatchID:    matchID,
		HomeTeam:   homeTeam,
		AwayTeam:   awayTeam,
		MatchTime:  time.Now(),
		Polymarket: *polymarketData,
		OddsPortal: OddsPortalData{
			MatchURL:    matchURL,
			Bookmakers:  bookmakers,
			AverageHome: averageHome,
			AverageDraw: averageDraw,
			AverageAway: averageAway,
			LastUpdated: time.Now(),
		},
		DataPoints: []DataPoint{dataPoint},
	}

	return matchData
}

func main() {
	// 定义命令行参数
	proxy := flag.String("proxy", "10.32.110.233:7890", "代理服务器地址")
	homeTeam := flag.String("home-team", "", "主队名称")
	awayTeam := flag.String("away-team", "", "客队名称")
	realTimeFlag := flag.Bool("realtime", false, "获取实时胜率")
	continuousFlag := flag.Bool("continuous", false, "连续获取数据并实时显示")
	interval := flag.Int("interval", 30, "获取数据的时间间隔（秒）")

	// 解析命令行参数
	flag.Parse()

	// 检查必要的参数
	if *realTimeFlag || *continuousFlag {
		if *homeTeam == "" || *awayTeam == "" {
			fmt.Println("错误: 必须指定主队和客队名称")
			flag.Usage()
			os.Exit(1)
		}

		if *continuousFlag {
			// 连续获取数据并实时显示
			err := StartContinuousDataCollection(*homeTeam, *awayTeam, *proxy, *interval)
			if err != nil {
				fmt.Printf("启动连续数据采集失败: %v\n", err)
				os.Exit(1)
			}
		} else {
			// 获取实时胜率
			matchData, err := GetRealTimeOdds(*homeTeam, *awayTeam, *proxy)
			if err != nil {
				fmt.Printf("获取实时胜率失败: %v\n", err)
				os.Exit(1)
			}

			// 比较数据
			CompareData(matchData)

			// 生成可视化
			filename := "odds_visualization.html"
			err = GenerateVisualization(matchData, filename)
			if err != nil {
				fmt.Printf("生成可视化失败: %v\n", err)
				os.Exit(1)
			}
			fmt.Printf("可视化已生成: %s\n", filename)
		}
	} else {
		// 显示用法
		flag.Usage()
		os.Exit(1)
	}
}
