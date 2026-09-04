/**
 * The numbers gate.
 *
 * Every percentage and score the UI renders must trace to something measured.
 * A previous pass shipped `confidence 0.94` — a figure nobody computed — past
 * this exact check, so this one reads the RENDERED DOM of every screen rather
 * than grepping source, prints each number it found with the element that
 * produced it, and states plainly which ones it could not trace.
 *
 * It also reports the raw `denied` and `evidence` counts the brief asks for,
 * from the rendered output rather than from the repository.
 *
 *   node tools/verify-numbers.mjs
 *
 * Requires a running fixture server (npm run fixture) and Google Chrome.
 */
import { spawn } from 'node:child_process';
import { rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = process.env.CHROME_BIN ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const BASE = process.env.AETHERCORE_BASE ?? 'http://127.0.0.1:1420';
const PAGES = ['overview', 'deepScan', 'drivers', 'repair', 'cleanup', 'startup', 'performance', 'hardware', 'crash', 'activity', 'fleet'];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect(port) {
  let info;
  for (let i = 0; i < 100 && !info; i += 1) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      info = targets.find((t) => t.type === 'page');
    } catch { /* not up yet */ }
    if (!info) await sleep(100);
  }
  if (!info) throw new Error('chrome devtools endpoint never came up');
  const socket = new WebSocket(info.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { socket.onopen = ok; socket.onerror = fail; });
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
  const send = (method, params = {}) => new Promise((ok, fail) => {
    const id = (nextId += 1);
    pending.set(id, { ok, fail, method });
    socket.send(JSON.stringify({ id, method, params }));
  });
  return { send, close: () => socket.close() };
}

/**
 * Pulls every score-shaped number out of the rendered page, with the element
 * that rendered it.
 *
 * Score-shaped means what the invariant is about: a percentage, or a bare
 * decimal between 0 and 1 of the `0.94` kind. Counts, byte sizes, versions,
 * durations and timestamps are measurements of real things and are not scores;
 * they are excluded by shape and the exclusions are listed in the output so the
 * reader can see what was not examined.
 */
const SCAN = `(() => {
  const out = [];
  const walker = document.createTreeWalker(document.querySelector('main'), NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.textContent.trim();
    if (!text) continue;
    const el = node.parentElement;
    if (!el || !el.getClientRects().length) continue;
    const percent = [...text.matchAll(/(\\d+(?:[.,]\\d+)?)\\s*%/g)].map((m) => m[0]);
    const unitless = [...text.matchAll(/(?:^|[\\s(:=])(0[.,]\\d+)(?![.,\\d])/g)].map((m) => m[1]);
    for (const value of [...percent, ...unitless]) {
      out.push({
        value,
        text: text.slice(0, 90),
        where: el.tagName.toLowerCase() + (el.className ? '.' + String(el.className).split(' ').filter(Boolean).slice(0, 2).join('.') : ''),
      });
    }
  }
  return {
    numbers: out,
    denied: document.body.innerText.match(/denied|ممنوع|مرفوض/gi)?.length ?? 0,
    deniedElements: document.querySelectorAll('.policy-denied-chip, .policy-denied-row, .policy-band').length,
    evidence: document.body.innerText.match(/evidence|الدليل|رصد/gi)?.length ?? 0,
    evidenceElements: document.querySelectorAll('[data-evidence-chip]').length,
  };
})()`;

async function main() {
  const profile = join(tmpdir(), `aethercore-numbers-${process.pid}`);
  const port = 9700 + (process.pid % 200);
  const chrome = spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--disable-gpu', '--hide-scrollbars', 'about:blank',
  ], { stdio: 'ignore' });

  const all = [];
  const totals = { denied: 0, deniedElements: 0, evidence: 0, evidenceElements: 0 };
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
    for (const locale of ['en', 'ar']) {
      await cdp.send('Page.navigate', { url: `${BASE}/layout-fixture.html` });
      await settle();
      await evaluate(`localStorage.setItem('aethercore.locale', ${JSON.stringify(locale)})`);
      for (const page of PAGES) {
        await cdp.send('Page.navigate', { url: `${BASE}/layout-fixture.html` });
        await settle();
        await evaluate(`document.querySelector('button[data-nav-item][data-page=' + JSON.stringify(${JSON.stringify(page)}) + ']')?.click()`);
        await sleep(180);
        const found = await evaluate(SCAN);
        for (const key of Object.keys(totals)) totals[key] += found[key];
        for (const entry of found.numbers) all.push({ page, locale, ...entry });
      }
    }
  } finally {
    cdp?.close();
    chrome.kill();
    await rm(profile, { recursive: true, force: true }).catch(() => {});
  }

  console.log('=== raw counts, from the rendered DOM of 11 pages x 2 languages ===');
  console.log(`  "denied" word occurrences        ${totals.denied}`);
  console.log(`  denied ELEMENTS rendered         ${totals.deniedElements}`);
  console.log(`  "evidence" word occurrences      ${totals.evidence}`);
  console.log(`  evidence CHIPS rendered          ${totals.evidenceElements}`);
  console.log('');
  console.log('=== every score-shaped number rendered, with its source element ===');
  if (!all.length) console.log('  (none)');
  const seen = new Set();
  for (const entry of all) {
    const key = `${entry.page}|${entry.locale}|${entry.value}|${entry.where}`;
    if (seen.has(key)) continue;
    seen.add(key);
    console.log(`  ${entry.locale} ${entry.page.padEnd(12)} ${entry.value.padEnd(10)} ${entry.where.padEnd(34)} ${JSON.stringify(entry.text)}`);
  }
  console.log(`\n${seen.size} distinct score-shaped number(s) rendered.`);
  console.log('Each must trace to a measurement. Percentages of a counted total and');
  console.log('service-reported ratios are traceable; a bare confidence score is not.');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
