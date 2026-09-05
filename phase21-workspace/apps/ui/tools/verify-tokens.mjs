/**
 * Every `var(--x)` in the product must resolve on the elements that use it.
 *
 * An unresolved custom property is silent: the declaration becomes invalid at
 * computed-value time and the property falls back to its inherited or initial
 * value, so a padding disappears or a colour becomes `currentColor` and nothing
 * warns. The layout sweep passes anyway — spacing that collapses to 0 does not
 * overflow. Only reading the computed value on the live element finds it.
 *
 * The check walks the real CSSOM, so it sees Svelte's scoped rules and the
 * cascade exactly as the browser assembled them, then asks
 * `getComputedStyle(el).getPropertyValue(name)` for every referenced property on
 * every element the rule actually matches.
 *
 *   node tools/verify-tokens.mjs
 *
 * Requires a running fixture server (npm run fixture) and Google Chrome.
 *
 * The count of declarations examined is printed and asserted. This check's own
 * first run reported a clean zero because CSS nesting makes `cssRules` present
 * on every CSSStyleRule, so recursing into it skipped every rule — a measuring
 * instrument that could only ever pass. A zero here has to be a zero out of a
 * known non-empty search.
 */
import { spawn } from 'node:child_process';
import { rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = process.env.CHROME_BIN ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const BASE = process.env.AETHERCORE_BASE ?? 'http://127.0.0.1:1420';
const PAGES = ['overview', 'deepScan', 'drivers', 'repair', 'cleanup', 'startup', 'performance', 'hardware', 'crash', 'activity', 'fleet'];
const LOCALES = ['en', 'ar'];
/** Below this, the walk found nothing to check and the run proves nothing. */
const MIN_DECLARATIONS = 100;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect(port) {
  let info;
  for (let i = 0; i < 200 && !info; i += 1) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      info = targets.find((t) => t.type === 'page');
    } catch { /* not up yet */ }
    if (!info) await sleep(100);
  }
  if (!info) throw new Error('chrome devtools endpoint never came up');
  const socket = new WebSocket(info.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { socket.onopen = ok; socket.onerror = fail; });
  socket.onerror = () => {};
  let nextId = 0;
  const pending = new Map();
  socket.onmessage = (m) => {
    const f = JSON.parse(m.data);
    if (f.id === undefined) return;
    const entry = pending.get(f.id);
    pending.delete(f.id);
    if (!entry) return;
    if (f.error) entry.fail(new Error(f.error.message)); else entry.ok(f.result);
  };
  return {
    send: (method, params = {}) => new Promise((ok, fail) => {
      const id = (nextId += 1);
      pending.set(id, { ok, fail });
      socket.send(JSON.stringify({ id, method, params }));
    }),
    close: () => socket.close(),
  };
}

const SCAN = `(() => {
  const dead = {};
  let declarations = 0, rules = 0;
  const walk = (list) => {
    for (const rule of list) {
      // Chrome supports CSS nesting, so CSSStyleRule.cssRules exists — usually
      // empty — on every style rule. Recursing must not skip the rule itself.
      if (rule.cssRules && rule.cssRules.length) walk(rule.cssRules);
      if (!rule.style || !rule.selectorText) continue;
      rules += 1;
      let matched = null;
      for (let i = 0; i < rule.style.length; i += 1) {
        const property = rule.style[i];
        const value = rule.style.getPropertyValue(property);
        if (!value.includes('var(')) continue;
        declarations += 1;
        for (const m of value.matchAll(/var\\(\\s*(--[A-Za-z0-9_-]+)\\s*([,)])/g)) {
          const name = m[1];
          if (matched === null) {
            try { matched = [...document.querySelectorAll(rule.selectorText)]; }
            catch { matched = []; }
          }
          for (const el of matched) {
            if (getComputedStyle(el).getPropertyValue(name).trim() !== '') continue;
            const id = name + '|' + property + '|' + rule.selectorText;
            dead[id] = dead[id] ?? { name, property, selector: rule.selectorText, fallback: m[2] === ',', elements: 0 };
            dead[id].elements += 1;
          }
        }
      }
    }
  };
  for (const sheet of document.styleSheets) { try { walk(sheet.cssRules); } catch { /* cross-origin */ } }
  return { dead: Object.values(dead), declarations, rules };
})()`;

async function main() {
  const profile = join(tmpdir(), `aethercore-tokens-${process.pid}`);
  const port = 9500 + (process.pid % 200);
  const chrome = spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--disable-gpu', '--hide-scrollbars', 'about:blank',
  ], { stdio: 'ignore' });

  const found = new Map();
  let declarations = 0;
  let rules = 0;
  let cdp;
  try {
    cdp = await connect(port);
    await cdp.send('Page.enable');
    await cdp.send('Runtime.enable');
    const evaluate = async (expression) => {
      const { result, exceptionDetails } = await cdp.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (exceptionDetails) throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text);
      return result.value;
    };
    const settle = () => evaluate(`(async () => {
      const deadline = Date.now() + 15000;
      while (!document.querySelector('.app-shell') && Date.now() < deadline) await new Promise((r) => setTimeout(r, 25));
      await document.fonts.ready;
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    })()`);

    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    for (const locale of LOCALES) {
      await cdp.send('Page.navigate', { url: `${BASE}/layout-fixture.html` });
      await settle();
      await evaluate(`localStorage.setItem('aethercore.locale', ${JSON.stringify(locale)})`);
      for (const page of PAGES) {
        await cdp.send('Page.navigate', { url: `${BASE}/layout-fixture.html` });
        await settle();
        await evaluate(`document.querySelector('button[data-nav-item][data-page=' + JSON.stringify(${JSON.stringify(page)}) + ']')?.click()`);
        await sleep(180);
        const scan = await evaluate(SCAN);
        declarations = Math.max(declarations, scan.declarations);
        rules = Math.max(rules, scan.rules);
        for (const entry of scan.dead) {
          const key = `${entry.name}|${entry.property}|${entry.selector}`;
          const previous = found.get(key);
          if (!previous || entry.elements > previous.elements) found.set(key, { ...entry, page, locale });
        }
      }
    }
  } finally {
    cdp?.close();
    chrome.kill();
    await rm(profile, { recursive: true, force: true }).catch(() => {});
  }

  const rows = [...found.values()].sort((a, b) => a.name.localeCompare(b.name));
  const names = new Set(rows.map((r) => r.name));
  console.log(`=== ${PAGES.length} pages x ${LOCALES.length} languages, read from the live CSSOM ===`);
  console.log(`  style rules walked                 ${rules}`);
  console.log(`  declarations referencing var()     ${declarations}`);
  console.log(`  unresolved on a matched element    ${rows.length} (property, selector) pair(s), ${names.size} distinct custom propert(ies)`);
  console.log('');
  for (const r of rows) {
    console.log(`  FAIL  ${r.name.padEnd(26)} ${r.property.padEnd(22)} fallback=${String(r.fallback).padEnd(5)} elements=${String(r.elements).padEnd(4)} ${r.selector.slice(0, 72)}`);
    console.log(`        first seen on ${r.locale} ${r.page}`);
  }

  if (declarations < MIN_DECLARATIONS) {
    console.log(`\nINCONCLUSIVE — only ${declarations} var() declarations were examined, below the ${MIN_DECLARATIONS} this product is known to carry.`);
    console.log('The walk found nothing to check, so a clean result would prove nothing.');
    process.exitCode = 1;
    return;
  }
  if (rows.length) {
    console.log(`\n${rows.length} unresolved var() reference(s). Each one silently drops its declaration.`);
    process.exitCode = 1;
    return;
  }
  console.log(`PASS — every var() reference in ${declarations} declarations resolves on every element the rule matches.`);
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
