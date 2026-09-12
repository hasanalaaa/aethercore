/**
 * Density instrument — turns "everything has far too much text" into numbers.
 *
 * The owner's complaint is about copy, and copy is the one thing this project
 * had no gate for: `layout-sweep` measures boxes, `verify-numbers` measures
 * traceability, `contrast-sweep` measures colour. Nothing counted words. So the
 * three previous design passes argued about taste with nothing to move.
 *
 * This reads the rendered DOM — not the source — because what the owner sees is
 * what the browser painted, and a `pk()` pair in a catalog is not evidence that
 * the string reached a screen.
 *
 * DEFINITIONS, so a later run means the same thing as this one:
 *
 *   run          a VISIBLE LEAF element inside the measured region carrying
 *                non-empty text. Leaves only: counting a container would count
 *                its children's words again.
 *
 *   technical    the run is inside `.technical-isolate`, `code` or `kbd`, or it
 *                is painted in the mono face. These are readings, never prose.
 *
 *   prose run    >= PROSE_MIN_WORDS words and not technical. Five words is the
 *                shortest thing in this product that is a sentence rather than
 *                a label: "Action items", "Core service log" and "CPU headroom"
 *                are all four words or fewer, and every subtitle and body
 *                paragraph is longer. The threshold is the whole measurement's
 *                one judgement call, and it is stated here so it can be argued
 *                with rather than hidden.
 *
 *   data reading a technical run, an em dash at rest, or a short run carrying a
 *                digit — the numbers and identifiers the instrument exists to
 *                show.
 *
 *   label        everything else: a short non-technical run that names a thing.
 *
 *   type size    distinct computed `font-size` across every visible run.
 *
 *   elevation    distinct (background-color, box-shadow) pair across visible
 *                elements that PAINT a background of their own — a surface. Two
 *                cards with the same fill and the same shadow are one
 *                elevation; a card, an inset well and a chip are three.
 *
 *   text-to-data prose words per data reading. The complaint expressed as one
 *                number: how many words of explanation the screen spends per
 *                measurement it shows.
 *
 * Usage:
 *   node tools/measure-density.mjs                       # populated, both languages
 *   node tools/measure-density.mjs --state empty
 *   node tools/measure-density.mjs --json out/before.json --label before
 *   node tools/measure-density.mjs --compare out/before.json
 *
 * Requires a running fixture server (pnpm fixture) and Google Chrome.
 */
import { spawn } from 'node:child_process';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = process.env.CHROME_BIN ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const BASE = process.env.AETHERCORE_BASE ?? 'http://127.0.0.1:1420';
const PAGES = ['overview', 'deepScan', 'drivers', 'repair', 'cleanup', 'startup', 'performance', 'hardware', 'crash', 'activity', 'fleet', 'settings'];
const PROSE_MIN_WORDS = 5;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function parseArgs(argv) {
  const args = { locales: ['en', 'ar'], state: 'populated', width: 1280, json: '', compare: '', label: '', pages: PAGES.join(',') };
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.replace(/^--/, '');
    const value = argv[i + 1];
    if (!key || value === undefined) continue;
    if (key === 'locales') args.locales = value.split(',');
    else if (key === 'width') args.width = Number(value);
    else if (key in args) args[key] = value;
  }
  args.pages = args.pages.split(',');
  return args;
}

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
  const send = (method, params = {}) => new Promise((ok, fail) => {
    const id = (nextId += 1);
    pending.set(id, { ok, fail });
    socket.send(JSON.stringify({ id, method, params }));
  });
  return { send, close: () => socket.close() };
}

/**
 * The count, run inside the page. Region is passed as a selector so the shell
 * (the sidebar, which carries a description under every entry) is measured with
 * the same rules as a screen rather than by eye.
 */
const COUNT = (selector, proseMin) => `(() => {
  const region = document.querySelector(${JSON.stringify(selector)});
  if (!region) return null;

  const visible = (el) => {
    const style = getComputedStyle(el);
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false;
    return el.getClientRects().length > 0;
  };

  const runs = [];
  const sizes = new Map();
  for (const el of region.querySelectorAll('*')) {
    if (el.children.length) continue;
    const text = (el.textContent ?? '').trim();
    if (!text) continue;
    if (!visible(el)) continue;
    const style = getComputedStyle(el);
    const family = style.fontFamily.toLowerCase();
    const mono = family.includes('jetbrains') || family.includes('mono');
    const technical = mono
      || el.closest('.technical-isolate, code, kbd') !== null
      || el.tagName === 'CODE' || el.tagName === 'KBD';
    const words = text.split(/\\s+/).filter(Boolean);
    const size = Math.round(parseFloat(style.fontSize) * 100) / 100;
    sizes.set(size, (sizes.get(size) ?? 0) + 1);
    runs.push({
      text: text.slice(0, 90),
      words: words.length,
      technical,
      digits: /[0-9\\u0660-\\u0669]/.test(text),
      dash: text === '\\u2014',
      size,
      tag: el.tagName.toLowerCase(),
      cls: String(el.className ?? '').slice(0, 48),
    });
  }

  // Surfaces: an element that paints a background of its own, or casts a shadow.
  const surfaces = new Map();
  for (const el of [region, ...region.querySelectorAll('*')]) {
    if (!visible(el)) continue;
    const style = getComputedStyle(el);
    const bg = style.backgroundColor;
    const image = style.backgroundImage;
    const shadow = style.boxShadow;
    const paints = (bg && bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent') || (image && image !== 'none');
    if (!paints && (!shadow || shadow === 'none')) continue;
    const key = image && image !== 'none' ? bg + ' | image | ' + shadow : bg + ' | ' + shadow;
    surfaces.set(key, (surfaces.get(key) ?? 0) + 1);
  }

  const prose = runs.filter((r) => !r.technical && r.words >= ${proseMin});
  const data = runs.filter((r) => r.technical || r.dash || (r.digits && r.words < ${proseMin}));
  const labels = runs.filter((r) => !prose.includes(r) && !data.includes(r));

  return {
    runs: runs.length,
    proseRuns: prose.length,
    proseWords: prose.reduce((total, r) => total + r.words, 0),
    dataReadings: data.length,
    labelRuns: labels.length,
    labelWords: labels.reduce((total, r) => total + r.words, 0),
    typeSizes: [...sizes.keys()].sort((a, b) => a - b),
    elevations: [...surfaces.keys()].sort(),
    longest: prose.sort((a, b) => b.words - a.words).slice(0, 6).map((r) => ({ words: r.words, tag: r.tag, cls: r.cls, text: r.text })),
  };
})()`;

function ratio(prose, data) {
  if (!data) return prose ? Infinity : 0;
  return Math.round((prose / data) * 100) / 100;
}

function fmtRatio(value) {
  if (value === Infinity || value === null) return '∞';
  return value.toFixed(2);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const entry = args.state === 'empty' ? 'index.html' : 'layout-fixture.html';
  const profile = join(tmpdir(), `aethercore-density-${process.pid}`);
  const port = 9700 + (process.pid % 200);
  const chrome = spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--disable-gpu', '--hide-scrollbars', 'about:blank',
  ], { stdio: 'ignore' });

  const rows = [];
  let shell = null;
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

    await cdp.send('Emulation.setDeviceMetricsOverride', { width: args.width, height: 900, deviceScaleFactor: 1, mobile: false });
    for (const locale of args.locales) {
      await cdp.send('Page.navigate', { url: `${BASE}/${entry}` });
      await settle();
      await evaluate(`localStorage.setItem('aethercore.locale', ${JSON.stringify(locale)})`);
      for (const page of args.pages) {
        await cdp.send('Page.navigate', { url: `${BASE}/${entry}` });
        await settle();
        const reached = await evaluate(`(() => {
          const button = document.querySelector('button[data-nav-item][data-page=' + JSON.stringify(${JSON.stringify(page)}) + ']');
          if (!button) return false;
          button.click();
          return true;
        })()`);
        if (!reached) throw new Error(`could not route to "${page}"`);
        await sleep(220);
        const measured = await evaluate(COUNT('main', PROSE_MIN_WORDS));
        if (!measured) throw new Error(`no <main> on "${page}"`);
        rows.push({ page, locale, state: args.state, ...measured, textToData: ratio(measured.proseWords, measured.dataReadings) });
        if (!shell && locale === 'en') {
          const sidebar = await evaluate(COUNT('.app-sidebar', PROSE_MIN_WORDS));
          if (sidebar) shell = { region: 'sidebar', locale, state: args.state, ...sidebar, textToData: ratio(sidebar.proseWords, sidebar.dataReadings) };
        }
      }
    }
  } finally {
    cdp?.close();
    chrome.kill();
    await rm(profile, { recursive: true, force: true }).catch(() => {});
  }

  const report = { label: args.label, state: args.state, width: args.width, proseMinWords: PROSE_MIN_WORDS, measuredAt: new Date().toISOString(), rows, shell };

  console.log(`=== density, ${args.state}, ${args.width}px, prose >= ${PROSE_MIN_WORDS} words ===\n`);
  for (const locale of args.locales) {
    console.log(`--- ${locale} ---`);
    console.log(`  ${'screen'.padEnd(12)} ${'prose'.padStart(6)} ${'runs'.padStart(5)} ${'data'.padStart(5)} ${'labels'.padStart(6)} ${'sizes'.padStart(5)} ${'elev'.padStart(4)} ${'w/reading'.padStart(9)}`);
    for (const row of rows.filter((r) => r.locale === locale)) {
      console.log(
        `  ${row.page.padEnd(12)} ${String(row.proseWords).padStart(6)} ${String(row.proseRuns).padStart(5)} `
        + `${String(row.dataReadings).padStart(5)} ${String(row.labelRuns).padStart(6)} ${String(row.typeSizes.length).padStart(5)} `
        + `${String(row.elevations.length).padStart(4)} ${fmtRatio(row.textToData).padStart(9)}`,
      );
    }
    const subset = rows.filter((r) => r.locale === locale);
    const words = subset.reduce((t, r) => t + r.proseWords, 0);
    const data = subset.reduce((t, r) => t + r.dataReadings, 0);
    console.log(`  ${'TOTAL'.padEnd(12)} ${String(words).padStart(6)} ${String(subset.reduce((t, r) => t + r.proseRuns, 0)).padStart(5)} ${String(data).padStart(5)} ${''.padStart(6)} ${''.padStart(5)} ${''.padStart(4)} ${fmtRatio(ratio(words, data)).padStart(9)}\n`);
  }
  if (shell) {
    console.log(`--- shell (sidebar, constant on every screen) ---`);
    console.log(`  prose words ${shell.proseWords}, prose runs ${shell.proseRuns}, labels ${shell.labelRuns}, type sizes ${shell.typeSizes.length}, elevations ${shell.elevations.length}\n`);
  }

  if (args.compare) {
    const before = JSON.parse(await readFile(resolve(args.compare), 'utf8'));
    console.log(`=== against ${args.compare} (${before.label || 'unlabelled'}) ===\n`);
    console.log(`  ${'screen'.padEnd(12)} ${'locale'.padEnd(6)} ${'prose before→after'.padEnd(22)} ${'sizes'.padEnd(12)} ${'elev'.padEnd(10)} ${'w/reading'}`);
    for (const row of rows) {
      const prior = before.rows.find((r) => r.page === row.page && r.locale === row.locale && r.state === row.state);
      if (!prior) continue;
      const delta = row.proseWords - prior.proseWords;
      const pct = prior.proseWords ? Math.round((-delta / prior.proseWords) * 100) : 0;
      console.log(
        `  ${row.page.padEnd(12)} ${row.locale.padEnd(6)} `
        + `${`${prior.proseWords} → ${row.proseWords} (${delta >= 0 ? '+' : ''}${delta}, ${pct >= 0 ? '-' : '+'}${Math.abs(pct)}%)`.padEnd(22)} `
        + `${`${prior.typeSizes.length} → ${row.typeSizes.length}`.padEnd(12)} `
        + `${`${prior.elevations.length} → ${row.elevations.length}`.padEnd(10)} `
        + `${fmtRatio(prior.textToData)} → ${fmtRatio(row.textToData)}`,
      );
    }
    console.log('');
  }

  if (args.json) {
    const path = resolve(args.json);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, JSON.stringify(report, null, 2));
    console.log(`written ${path}`);
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
