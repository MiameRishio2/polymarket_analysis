const assert = require('assert');
const fs = require('fs');
const { TextDecoder } = require('util');
const vm = require('vm');

function loadMenuScript(pathname, fetchImpl) {
  const html = fs.readFileSync('public/menu.html', 'utf8');
  const script = html.match(/<script>([\s\S]*)<\/script>/)[1].replace(/\n\s*init\(\);\s*$/, '');
  const elements = {};
  const logs = [];
  const context = {
    window: { location: { pathname } },
    document: {
      title: '',
      getElementById: (id) => {
        if (!elements[id]) elements[id] = { innerHTML: '', textContent: '' };
        return elements[id];
      }
    },
    localStorage: { setItem: () => {} },
    fetch: fetchImpl || (async () => ({ json: async () => ({ ok: true, data: { categories: [] } }) })),
    setTimeout: () => {},
    TextDecoder,
    console: {
      log: (...args) => logs.push({ level: 'log', args }),
      warn: (...args) => logs.push({ level: 'warn', args }),
      error: (...args) => logs.push({ level: 'error', args }),
    },
  };

  vm.createContext(context);
  vm.runInContext(script, context);
  context.__elements = elements;
  context.__logs = logs;
  return context;
}

function streamResponse(lines) {
  const encoded = Buffer.from(lines.join('\n') + '\n', 'utf8');
  let consumed = false;

  return {
    body: {
      getReader: () => ({
        read: async () => {
          if (consumed) return { done: true };
          consumed = true;
          return {
            done: false,
            value: encoded
          };
        }
      })
    },
    json: async () => {
      throw new Error('stream response should not use json()');
    }
  };
}

function testThirdLevelConfig() {
  const context = loadMenuScript('/menu/football/argentina');
  const config = context.getPageConfig();

  assert.strictEqual(config.type, 'third-level');
  assert.strictEqual(config.title, 'Argentina 分类');
  assert.strictEqual(config.apiUrl, '/api/menu/football/argentina');
  assert.strictEqual(config.refreshUrl, '/api/menu/football/argentina/refresh');
  assert.strictEqual(config.localKey, 'categories_football_argentina');
}

function testSecondLevelRowLinksToLocalThirdLevelPage() {
  const context = loadMenuScript('/menu/football');
  const config = context.getPageConfig();
  const href = context.getCategoryRowHref(
    { url: '/football/argentina/', slug: 'argentina' },
    config
  );

  assert.strictEqual(href, '/menu/football/argentina/');
}

function testFourthLevelConfig() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  const config = context.getPageConfig();

  assert.strictEqual(config.type, 'fourth-level');
  assert.strictEqual(config.title, 'World Championship 2026 分类');
  assert.strictEqual(config.apiUrl, '/api/menu/football/world/world-championship-2026');
  assert.strictEqual(config.refreshUrl, '/api/menu/football/world/world-championship-2026/refresh');
  assert.strictEqual(config.localKey, 'categories_football_world_world-championship-2026');
}

function testThirdLevelRowLinksToLocalFourthLevelPage() {
  const context = loadMenuScript('/menu/football/world');
  const config = context.getPageConfig();
  const href = context.getCategoryRowHref(
    { url: '/football/world/world-championship-2026/', slug: 'world-championship-2026' },
    config
  );

  assert.strictEqual(href, '/menu/football/world/world-championship-2026/');
}


function testFourthLevelEventConfig() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  const config = context.getPageConfig();

  assert.strictEqual(config.eventApiUrl, '/api/events/football/world/world-championship-2026');
  assert.strictEqual(config.eventRefreshUrl, '/api/events/football/world/world-championship-2026/refresh');
  assert.strictEqual(config.eventLocalKey, 'events_football_world_world-championship-2026');
}

function testRenderEventRows() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '18 Jun 2026, 03:00',
      url: '/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /比赛/);
  assert.match(context.__elements.content.innerHTML, /开始时间/);
  assert.match(context.__elements.content.innerHTML, /Mexico VS South Africa/);
  assert.match(context.__elements.content.innerHTML, /18 Jun 2026, 03:00/);
  assert.match(context.__elements.content.innerHTML, /football\/h2h\/mexico-O6iHcNkd\/south-africa-W2ijYvlr/);
}

function testRenderEventStatusBadges() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      ended: true,
      url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
      polymarket_url: 'https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11'
    }, {
      matchup: 'Qatar VS Switzerland',
      start_time: '13 Jun 2026, 21:00',
      ended: false,
      url: 'https://www.oddsportal.com/football/h2h/qatar/switzerland/'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /状态/);
  assert.match(context.__elements.content.innerHTML, /已结束/);
  assert.match(context.__elements.content.innerHTML, /未结束/);
  assert.match(context.__elements.content.innerHTML, /OddsPortal/);
  assert.match(context.__elements.content.innerHTML, /Polymarket/);
  assert.match(context.__elements.content.innerHTML, /加入监控/);
}

function testRenderEventLinkButtons() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      url: 'https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2',
      polymarket_url: 'https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /OddsPortal/);
  assert.match(context.__elements.content.innerHTML, /Polymarket/);
  assert.match(
    context.__elements.content.innerHTML,
    /href="https:\/\/polymarket\.com\/sports\/world-cup\/fifwc-mex-rsa-2026-06-11"/
  );
}

function testRenderDisabledPolymarketButton() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      url: 'https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /link-btn disabled/);
  assert.match(context.__elements.content.innerHTML, /Polymarket/);
}

function testRenderEventSchedulerButton() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
      polymarket_url: 'https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /加入监控/);
  assert.match(context.__elements.content.innerHTML, /scheduler-btn/);
}

function testRenderRootSchedulerSection() {
  const context = loadMenuScript('/menu');
  vm.runInContext(`
    schedulerItems = [{
      id: 's1',
      matchup: 'Mexico VS South Africa',
      start_time: '2026-06-18T03:00:00Z',
      oddsportal_url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
      polymarket_url: null,
      monitoring_started: false
    }];
    data = [{ slug: 'football', name: 'Football', url: '/football/' }];
    dataMode = 'categories';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /scheduler-section/);
  assert.match(context.__elements.content.innerHTML, /Mexico VS South Africa/);
  assert.match(context.__elements.content.innerHTML, /开始监控/);
}

function testRenderSubpageSchedulerSection() {
  const context = loadMenuScript('/menu/football/world');
  vm.runInContext(`
    schedulerItems = [{
      id: 's1',
      matchup: 'Mexico VS South Africa',
      start_time: '2026-06-18T03:00:00Z',
      oddsportal_url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
      polymarket_url: null,
      monitoring_started: false
    }];
    data = [{ slug: 'world-championship-2026', name: 'World Championship 2026', url: '/football/world/world-championship-2026/' }];
    dataMode = 'categories';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /scheduler-section/);
  assert.match(context.__elements.content.innerHTML, /Mexico VS South Africa/);
  assert.match(context.__elements.content.innerHTML, /World Championship 2026/);
}

function testRenderEmptySubpageSchedulerSection() {
  const context = loadMenuScript('/menu/football/world');
  vm.runInContext(`
    schedulerItems = [{
      id: 's1',
      matchup: 'Mexico VS South Africa',
      start_time: '2026-06-18T03:00:00Z',
      oddsportal_url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
      polymarket_url: null,
      monitoring_started: false
    }];
    data = [];
    dataMode = 'categories';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /scheduler-section/);
  assert.match(context.__elements.content.innerHTML, /Mexico VS South Africa/);
  assert.match(context.__elements.content.innerHTML, /暂无数据/);
}

async function testScheduleEventPostsMetadata() {
  const requests = [];
  const context = loadMenuScript('/menu/football/world/world-championship-2026', async (url, options) => {
    requests.push({ url, options });
    return {
      json: async () => ({
        ok: true,
        data: {
          id: 's1',
          matchup: 'Mexico VS South Africa',
          monitoring_started: false
        }
      })
    };
  });

  await context.scheduleEvent({
    matchup: 'Mexico VS South Africa',
    home_team: 'Mexico',
    away_team: 'South Africa',
    start_time: '2026-06-18T03:00:00Z',
    url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
    polymarket_url: 'https://polymarket.com/event'
  });

  assert.strictEqual(requests[0].url, '/api/scheduler');
  assert.strictEqual(requests[0].options.method, 'POST');
  const body = JSON.parse(requests[0].options.body);
  assert.strictEqual(body.matchup, 'Mexico VS South Africa');
  assert.strictEqual(body.home_team, 'Mexico');
  assert.strictEqual(body.away_team, 'South Africa');
  assert.strictEqual(body.source_page, '/menu/football/world/world-championship-2026');
}

function testEventPaginationUsesTenRows() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = Array.from({ length: 11 }, (_, index) => ({
      matchup: 'Team ' + (index + 1) + ' VS Opponent ' + (index + 1),
      start_time: '18 Jun 2026, 03:00',
      url: '/event-' + (index + 1)
    }));
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /Team 10 VS Opponent 10/);
  assert.doesNotMatch(context.__elements.content.innerHTML, /Team 11 VS Opponent 11/);
  assert.match(context.__elements.pagination.innerHTML, /下一页/);
}

function testParentMenuHref() {
  assert.strictEqual(loadMenuScript('/menu').getParentMenuHref(), null);
  assert.strictEqual(loadMenuScript('/menu/football/').getParentMenuHref(), '/menu');
  assert.strictEqual(loadMenuScript('/menu/football/world/').getParentMenuHref(), '/menu/football');
  assert.strictEqual(
    loadMenuScript('/menu/football/world/world-championship-2026/').getParentMenuHref(),
    '/menu/football/world'
  );
}

function testRenderParentNavigation() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026/');
  context.renderParentNavigation();

  assert.match(context.__elements['top-nav'].innerHTML, /href="\/menu\/football\/world"/);
  assert.match(context.__elements['top-nav'].innerHTML, /返回上一级/);
}

function testRenderRootHasNoParentNavigation() {
  const context = loadMenuScript('/menu');
  context.renderParentNavigation();

  assert.strictEqual(context.__elements['top-nav'].innerHTML, '');
}

async function testFetchCategoryDataRendersRefreshTime() {
  const context = loadMenuScript('/menu/football', async () => ({
    json: async () => ({
      ok: true,
      data: {
        categories: [{
          slug: 'argentina',
          name: 'Argentina',
          url: '/football/argentina/',
          category_type: 'country'
        }],
        refreshed_at: '2026-06-12T08:00:00Z'
      }
    })
  }));

  await context.fetchData();

  assert.match(context.__elements['stats-bar'].innerHTML, /刷新时间: 2026-06-12T08:00:00Z/);
}

async function testFetchEventDataRendersEventRefreshTime() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026', async (url) => ({
    json: async () => {
      if (url.startsWith('/api/events/')) {
        return {
          ok: true,
          data: {
            events: [{
              matchup: 'Mexico VS South Africa',
              start_time: '18 Jun 2026, 03:00',
              url: '/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/'
            }],
            refreshed_at: '2026-06-12T09:00:00Z'
          }
        };
      }
      return { ok: true, data: { categories: [], refreshed_at: '1970-01-01T00:00:00Z' } };
    }
  }));

  await context.fetchData();

  assert.match(context.__elements['stats-bar'].innerHTML, /刷新时间: 2026-06-12T09:00:00Z/);
}

async function testRefreshEventDataLogsEventResponse() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026', async (url) => ({
    json: async () => {
      if (url.startsWith('/api/events/')) {
        return {
          ok: true,
          data: {
            events: [{
              matchup: 'Qatar VS Switzerland',
              start_time: '13 Jun 2026, 21:00',
              url: 'https://www.oddsportal.com/football/h2h/qatar/switzerland/'
            }],
            refreshed_at: '2026-06-13T00:00:00Z',
            source: 'scraped'
          }
        };
      }
      return { ok: true, data: { categories: [], refreshed_at: '1970-01-01T00:00:00Z' } };
    }
  }));

  await context.refreshData();

  assert(
    context.__logs.some(entry =>
      entry.level === 'log' &&
      String(entry.args[0]).includes('[menu] event refresh response') &&
      entry.args[1].eventCount === 1 &&
      entry.args[1].missingPolymarketCount === 1
    ),
    `expected event refresh response log, got ${JSON.stringify(context.__logs)}`
  );
}

async function testRefreshEventDataLogsEachStreamedEvent() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026', async (url) => {
    if (url.endsWith('/refresh-stream')) {
      return streamResponse([
        JSON.stringify({
          type: 'event',
          index: 1,
          total: 2,
          matchup: 'Qatar VS Switzerland',
          status: 'missing_polymarket',
          polymarket_url: null
        }),
        JSON.stringify({
          type: 'event',
          index: 2,
          total: 2,
          matchup: 'Haiti VS Scotland',
          status: 'matched',
          polymarket_url: 'https://polymarket.com/sports/world-cup/fifwc-hai-sco-2026-06-13'
        }),
        JSON.stringify({
          type: 'complete',
          event_count: 2,
          refreshed_at: '2026-06-13T00:00:00Z',
          events: []
        })
      ]);
    }
    return { json: async () => ({ ok: true, data: { categories: [], refreshed_at: '1970-01-01T00:00:00Z' } }) };
  });

  await context.refreshData();

  const eventLogs = context.__logs.filter(entry =>
    entry.level === 'log' && String(entry.args[0]).includes('[menu] event processed')
  );

  assert.strictEqual(eventLogs.length, 2);
  assert.strictEqual(eventLogs[0].args[1].matchup, 'Qatar VS Switzerland');
  assert.strictEqual(eventLogs[0].args[1].index, 1);
  assert.strictEqual(eventLogs[1].args[1].matchup, 'Haiti VS Scotland');
}

testThirdLevelConfig();
testSecondLevelRowLinksToLocalThirdLevelPage();
testFourthLevelConfig();
testThirdLevelRowLinksToLocalFourthLevelPage();
testFourthLevelEventConfig();
testRenderEventRows();
testRenderEventStatusBadges();
testRenderEventLinkButtons();
testRenderDisabledPolymarketButton();
testRenderEventSchedulerButton();
testRenderRootSchedulerSection();
testRenderSubpageSchedulerSection();
testRenderEmptySubpageSchedulerSection();
testEventPaginationUsesTenRows();
testParentMenuHref();
testRenderParentNavigation();
testRenderRootHasNoParentNavigation();

(async () => {
  await testFetchCategoryDataRendersRefreshTime();
  await testFetchEventDataRendersEventRefreshTime();
  await testRefreshEventDataLogsEventResponse();
  await testRefreshEventDataLogsEachStreamedEvent();
  await testScheduleEventPostsMetadata();
  console.log('menu_page_config_test passed');
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
