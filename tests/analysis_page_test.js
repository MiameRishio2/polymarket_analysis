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

function testHistoryUsesReadableBookmakerFallbackForLegacyRows() {
  const context = loadAnalysisScript();
  const result = context.buildSeries([{
    captured_at: '2026-06-14T08:39:39Z',
    rows: [{
      market_key: 'd/oddsdata/back/E-3-2-0-0-0',
      market_label: 'Home/Away · Full Time',
      bookmaker_id: '1159',
      outcomes: [
        { index: 0, label: '1', odds: 1.5 },
        { index: 1, label: '2', odds: 2.7 }
      ]
    }]
  }]);

  assert.strictEqual(result.series[0].bookmakerId, 'Bookmaker 1159');
}

testOneXTwoHistoryDisplaysTeamProbabilitiesAndBookmakerName();
testHistoryUsesReadableBookmakerFallbackForLegacyRows();
