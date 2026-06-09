const assert = require('assert');
const fs = require('fs');
const vm = require('vm');

function loadMenuScript(pathname) {
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
    fetch: async () => ({ json: async () => ({ ok: true, data: { categories: [] } }) }),
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

testThirdLevelConfig();
testSecondLevelRowLinksToLocalThirdLevelPage();
testFourthLevelConfig();
testThirdLevelRowLinksToLocalFourthLevelPage();
testParentMenuHref();
testRenderParentNavigation();
console.log('menu_page_config_test passed');
