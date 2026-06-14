const assert = require('assert');
const fs = require('fs');
const vm = require('vm');

function loadAnalysisScript() {
  const html = fs.readFileSync('public/analysis.html', 'utf8');
  const script = html.match(/<script>([\s\S]*)<\/script>/)[1].replace(/\n\s*init\(\);\s*$/, '');
  const context = {
    document: {
      getElementById: () => ({ innerHTML: '', textContent: '', disabled: false })
    },
    fetch: async () => ({ json: async () => ({ ok: true, data: [] }) }),
    setInterval: () => {},
    console
  };
  vm.createContext(context);
  vm.runInContext(script, context);
  return context;
}

function testOneXTwoHistoryDisplaysTeamProbabilitiesAndBookmakerName() {
  const context = loadAnalysisScript();
  const history = [{
    captured_at: '2026-06-14T08:39:39Z',
    rows: [{
      market_key: 'd/oddsdata/back/E-1-2-0-0-0',
      market_label: '1X2 · Full Time',
      bookmaker_id: '1015',
      bookmaker_name: '1xBet',
      outcomes: [
        { index: 0, label: '1', odds: 5.6 },
        { index: 1, label: 'X', odds: 4.3 },
        { index: 2, label: '2', odds: 1.55 }
      ]
    }]
  }];

  const result = context.buildSeries(history);

  assert.strictEqual(result.series.length, 2);
  assert.strictEqual(result.series[0].outcomeLabel, '主胜率');
  assert.strictEqual(result.series[1].outcomeLabel, '客胜率');
  assert.ok(result.series.every(item => item.bookmakerId === '1xBet'));
  assert.ok(result.series.every(item => item.marketLabel === '主/客胜率 · Full Time'));
  assert.ok(Math.abs(result.series[0].points[0].odds - 0.2167) < 0.001);
  assert.ok(Math.abs(result.series[1].points[0].odds - 0.7833) < 0.001);
}

function testHistoryOmitsPlaceholderBookmakerRows() {
  const context = loadAnalysisScript();
  const result = context.buildSeries([{
    captured_at: '2026-06-14T08:39:39Z',
    rows: [{
      market_key: 'd/oddsdata/back/E-3-2-0-0-0',
      market_label: 'Home/Away · Full Time',
      bookmaker_id: '99999',
      bookmaker_name: 'Bookmaker 99999',
      outcomes: [
        { index: 0, label: '1', odds: 1.5 },
        { index: 1, label: '2', odds: 2.7 }
      ]
    }]
  }]);

  assert.strictEqual(result.series.length, 0);
}

function testHistoryKeepsKnownBookmakerIdFallback() {
  const context = loadAnalysisScript();
  const result = context.buildSeries([{
    captured_at: '2026-06-14T08:39:39Z',
    rows: [{
      market_key: 'd/oddsdata/back/E-3-2-0-0-0',
      market_label: 'Home/Away · Full Time',
      bookmaker_id: '44',
      outcomes: [
        { index: 0, label: '1', odds: 1.5 },
        { index: 1, label: '2', odds: 2.7 }
      ]
    }]
  }]);

  assert.strictEqual(result.series[0].bookmakerId, 'bet365');
}

function testHistoryKeepsRequestedOddsPortalBookmakers() {
  const context = loadAnalysisScript();
  const rows = [
    ['500', '22bet'],
    ['911', 'BetFury'],
    ['575', 'BetInAsia'],
    ['909', 'Bets.io']
  ].map(([bookmakerId]) => ({
    market_key: 'd/oddsdata/back/E-3-2-0-0-0',
    market_label: 'Home/Away · Full Time',
    bookmaker_id: bookmakerId,
    bookmaker_name: `Bookmaker ${bookmakerId}`,
    outcomes: [
      { index: 0, label: '1', odds: 1.5 },
      { index: 1, label: '2', odds: 2.7 }
    ]
  }));

  const result = context.buildSeries([{ captured_at: '2026-06-14T08:39:39Z', rows }]);
  const names = [...new Set(result.series.map(item => item.bookmakerId))];

  assert.deepStrictEqual(names, ['22bet', 'BetFury', 'BetInAsia', 'Bets.io']);
}

function testVisibleAnalysisItemsKeepsOnlyFirstScheduleItem() {
  const context = loadAnalysisScript();
  const items = [
    { match_id: 'first', oddsportal: { status: 'ok' }, polymarket: { status: 'ok' } },
    { match_id: 'second', oddsportal: { status: 'ok' }, polymarket: { status: 'error' } }
  ];

  const visibleItems = context.visibleAnalysisItems(items);
  assert.strictEqual(visibleItems.length, 1);
  assert.strictEqual(visibleItems[0].match_id, 'first');
  assert.strictEqual(context.visibleAnalysisItems(null).length, 0);
}

function testVisibleAnalysisItemsUsesManualSelection() {
  const context = loadAnalysisScript();
  const items = [
    { match_id: 'first', matchup: 'First Match' },
    { match_id: 'second', matchup: 'Second Match' }
  ];

  const visibleItems = context.visibleAnalysisItems(items, 'second');

  assert.strictEqual(visibleItems.length, 1);
  assert.strictEqual(visibleItems[0].match_id, 'second');
}

function testRenderAnalysisSelectorAllowsManualScheduleChoice() {
  const context = loadAnalysisScript();
  const items = [
    { match_id: 'first', matchup: 'First Match', start_time: '2026-06-14 10:00' },
    { match_id: 'second', matchup: 'Second Match', start_time: '2026-06-14 12:00' }
  ];

  const html = context.renderAnalysisSelector(items, 'second');

  assert.ok(html.includes('<select'));
  assert.ok(html.includes('onchange="selectAnalysisItem(this.value)"'));
  assert.ok(html.includes('value="second" selected'));
  assert.ok(html.includes('Second Match'));
}

function testMarketHistoryRendersAllSeriesInOneCombinedChart() {
  const context = loadAnalysisScript();
  const history = [{
    captured_at: '2026-06-14T08:39:39Z',
    rows: [
      {
        market_key: 'd/oddsdata/back/E-3-2-0-0-0',
        market_label: 'Home/Away · Full Time',
        bookmaker_id: '44',
        bookmaker_name: 'bet365',
        outcomes: [
          { index: 0, label: '1', odds: 1.5 },
          { index: 1, label: '2', odds: 2.7 }
        ]
      },
      {
        market_key: 'd/oddsdata/back/E-3-2-0-0-0',
        market_label: 'Home/Away · Full Time',
        bookmaker_id: '500',
        bookmaker_name: '22bet',
        outcomes: [
          { index: 0, label: '1', odds: 1.6 },
          { index: 1, label: '2', odds: 2.5 }
        ]
      }
    ]
  }];

  const html = context.renderMarketHistory(history);

  assert.strictEqual((html.match(/class="combined-chart"/g) || []).length, 1);
  assert.strictEqual((html.match(/class="sparkline"/g) || []).length, 0);
  assert.ok(html.includes('Home/Away · Full Time · bet365 · 1'));
  assert.ok(html.includes('Home/Away · Full Time · 22bet · 2'));
}

testOneXTwoHistoryDisplaysTeamProbabilitiesAndBookmakerName();
testHistoryOmitsPlaceholderBookmakerRows();
testHistoryKeepsKnownBookmakerIdFallback();
testHistoryKeepsRequestedOddsPortalBookmakers();
testVisibleAnalysisItemsKeepsOnlyFirstScheduleItem();
testVisibleAnalysisItemsUsesManualSelection();
testRenderAnalysisSelectorAllowsManualScheduleChoice();
testMarketHistoryRendersAllSeriesInOneCombinedChart();
