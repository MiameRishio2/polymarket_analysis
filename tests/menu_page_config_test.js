const assert = require('assert');
const fs = require('fs');
const vm = require('vm');

function loadMenuScript(pathname, fetchImpl) {
  const html = fs.readFileSync('public/menu.html', 'utf8');
  const script = html.match(/<script>([\s\S]*)<\/script>/)[1].replace(/\n\s*init\(\);\s*$/, '');
  const elements = {};
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
    console,
  };

  vm.createContext(context);
  vm.runInContext(script, context);
  context.__elements = elements;
  return context;
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

testThirdLevelConfig();
testSecondLevelRowLinksToLocalThirdLevelPage();
testFourthLevelConfig();
testThirdLevelRowLinksToLocalFourthLevelPage();
testFourthLevelEventConfig();
testRenderEventRows();
testRenderEventLinkButtons();
testRenderDisabledPolymarketButton();
testEventPaginationUsesTenRows();
testParentMenuHref();
testRenderParentNavigation();

(async () => {
  await testFetchCategoryDataRendersRefreshTime();
  await testFetchEventDataRendersEventRefreshTime();
  console.log('menu_page_config_test passed');
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
