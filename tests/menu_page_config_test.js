const assert = require('assert');
const fs = require('fs');
const vm = require('vm');

function loadMenuScript(pathname) {
  const html = fs.readFileSync('public/menu.html', 'utf8');
  const script = html.match(/<script>([\s\S]*)<\/script>/)[1].replace(/\n\s*init\(\);\s*$/, '');
  const context = {
    window: { location: { pathname } },
    document: { title: '', getElementById: () => ({ innerHTML: '', textContent: '' }) },
    localStorage: { setItem: () => {} },
    fetch: async () => ({ json: async () => ({ ok: true, data: { categories: [] } }) }),
    setTimeout: () => {},
    console,
  };

  vm.createContext(context);
  vm.runInContext(script, context);
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

testThirdLevelConfig();
testSecondLevelRowLinksToLocalThirdLevelPage();
console.log('menu_page_config_test passed');
