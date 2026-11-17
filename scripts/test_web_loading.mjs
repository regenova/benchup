// Run with node --test scripts/test_web_loading.mjs; no app build required.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import initializer from '../web/initializer.mjs';

const catalog = JSON.parse(readFileSync(new URL('../assets/i18n/en_US.json', import.meta.url)));
function setup(messages = Promise.resolve(catalog)) {
    const attributes = new Map();
    const properties = new Map();
    const bar = {
        setAttribute: (key, value) => attributes.set(key, value),
        removeAttribute: key => attributes.delete(key),
        style: {setProperty: (key, value) => properties.set(key, value)},
    };
    const label = {};
    const elements = new Map([['loading-progress', bar], ['loading-download', label]]);
    const document = {documentElement: {lang: 'en-US'}, getElementById: id => elements.get(id)};
    return {events: initializer({document, messages}), bar, label, elements, attributes, properties};
}

test('Trunk is configured to use the measured download initializer', () => {
    const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
    assert.match(html, /<link[^>]+rel="rust"[^>]+data-initializer="web\/initializer.mjs"/);
    assert.match(html, /\.loading-progress\[data-determinate\]::after\s*\{[^}]*animation: none/);
});

test('the bar follows actual byte counts, then waits for the renderer at 100%', async () => {
    const view = setup();
    await Promise.resolve();
    view.events.onStart();
    assert.equal(view.attributes.has('aria-valuenow'), false);
    assert.equal(view.label.textContent, catalog['web.downloading']);
    view.events.onProgress({current: 1_000_000, total: 4_000_000});
    assert.equal(view.attributes.get('aria-valuenow'), '25');
    assert.equal(view.properties.get('--progress'), '25%');
    assert.equal(view.label.textContent, 'Downloading BenchUp… 25% (1.0 / 4.0 MB)');
    view.events.onProgress({current: 3_999_999, total: 4_000_000});
    assert.equal(view.attributes.get('aria-valuenow'), '99');
    view.events.onProgress({current: 4_000_000, total: 4_000_000});
    assert.equal(view.attributes.get('aria-valuenow'), '100');
    assert.equal(view.label.textContent, catalog['web.starting']);
    view.events.onSuccess();
    assert.equal(view.elements.size, 2, 'Rust owns removal after GPU startup');
});

test('cached downloads can complete in a single callback', async () => {
    const view = setup();
    await Promise.resolve();
    view.events.onProgress({current: 100, total: 100});
    assert.equal(view.attributes.get('aria-valuenow'), '100');
    assert.equal(view.label.textContent, catalog['web.starting']);
});

test('unknown sizes stay indeterminate and invalid values never become percentages', async () => {
    const view = setup();
    await Promise.resolve();
    for (const total of [0, undefined, NaN, Infinity, -1]) {
        view.events.onProgress({current: 50, total});
        assert.equal(view.attributes.has('aria-valuenow'), false);
        assert.equal(view.label.textContent, catalog['web.downloading']);
    }
    for (const current of [-1, NaN, Infinity]) {
        view.events.onProgress({current, total: 100});
        assert.equal(view.attributes.get('aria-valuenow'), '0');
    }
});

test('late translations retain the latest download or failure state', async () => {
    let resolve;
    const view = setup(new Promise(done => { resolve = done; }));
    view.events.onProgress({current: 50, total: 100});
    assert.equal(view.label.textContent, '50%');
    resolve(catalog);
    await Promise.resolve();
    assert.match(view.label.textContent, /50%/);
    view.events.onFailure(new Error('private diagnostic'));
    assert.equal(view.bar.hidden, true);
    assert.equal(view.attributes.has('aria-valuenow'), false);
    assert.equal(view.label.textContent, catalog['web.download_failed']);
});

test('a failure before translations arrive is still reported when they load', async () => {
    let resolve;
    const view = setup(new Promise(done => { resolve = done; }));
    view.events.onFailure();
    resolve(catalog);
    await Promise.resolve();
    assert.equal(view.bar.hidden, true);
    assert.equal(view.label.textContent, catalog['web.download_failed']);
});

test('late callbacks leave renderer errors and the running app untouched', async () => {
    const view = setup();
    view.elements.clear();
    view.events.onProgress({current: 100, total: 100});
    view.events.onFailure();
    await Promise.resolve();
    assert.equal(view.label.textContent, undefined);
});

test('all supported catalogs translate startup progress and preserve placeholders', () => {
    const keys = ['web.downloading', 'web.download_progress', 'web.starting', 'web.download_failed'];
    for (const language of ['es_419', 'fr_FR', 'it_IT', 'pt_BR', 'vi_VN', 'zh_CN']) {
        const translated = JSON.parse(readFileSync(new URL(`../assets/i18n/${language}.json`, import.meta.url)));
        for (const key of keys) {
            assert.ok(translated[key], `${language}: ${key}`);
            assert.notEqual(translated[key], catalog[key]);
            const placeholders = text => [...text.matchAll(/\{\w+\}/g)].map(match => match[0]).sort();
            assert.deepEqual(placeholders(translated[key]), placeholders(catalog[key]));
        }
    }
});
