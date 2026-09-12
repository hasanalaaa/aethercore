/**
 * Arabic verification, from the rendered DOM rather than from a screenshot.
 *
 * A previous session judged Arabic from an image and reported a cause the HTML
 * disproved. Every check here reads the live document or asks the rendering
 * engine directly, and prints the observed value beside what was expected.
 *
 * The font check is the one that matters most. `document.fonts.check()` only
 * says a face *could* be used; it says nothing about what actually drew the
 * pixels. `CSS.getPlatformFontsForNode` reports the font the compositor really
 * used, per node, with a glyph count — so a system fallback that happens to
 * shape correctly on this Mac, and would not on a bare Windows box, cannot pass.
 *
 *   node tools/verify-arabic.mjs
 *
 * Requires a running fixture server (npm run fixture) and Google Chrome.
 */
import { spawn } from 'node:child_process';
import { rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = process.env.CHROME_BIN ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const BASE = process.env.AETHERCORE_BASE ?? 'http://127.0.0.1:1420';
/**
 * Which screen to check. Default `overview`, so the gate's command and its
 * output are unchanged. §51.3 moved four sections off the Overview onto a new
 * Settings screen and three more onto Activity, and this gate reads one screen —
 * without this flag their Arabic stopped being checked by anything.
 */
const PAGE = (process.argv.indexOf('--page') >= 0 ? process.argv[process.argv.indexOf('--page') + 1] : 'overview');
/**
 * `--page assistant` is not a screen. It is P57's assistant drawer, which lives
 * OUTSIDE `main` — so every selector below would have walked straight past it,
 * and its Arabic would have been checked by nothing. Same blindness as
 * `layout-sweep`'s `pages: ['overview']` default, caught before it shipped
 * rather than after.
 */
const DRAWER = PAGE === 'assistant';
const REGION = DRAWER ? '.assistant-drawer' : 'main';
const EMBEDDED_FACE = 'IBM Plex Sans Arabic';

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
    if (f.error) entry.fail(new Error(`${f.error.message} (${entry.method})`));
    else entry.ok(f.result);
  };
  const send = (method, params = {}) => new Promise((ok, fail) => {
    const id = (nextId += 1);
    pending.set(id, { ok, fail, method });
    socket.send(JSON.stringify({ id, method, params }));
  });
  return { send, close: () => socket.close() };
}

const results = [];
function check(name, expected, observed, pass) {
  results.push({ name, expected, observed, pass });
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}`);
  console.log(`        EXPECTED  ${expected}`);
  console.log(`        OBSERVED  ${observed}`);
}

async function main() {
  const profile = join(tmpdir(), `aethercore-arabic-${process.pid}`);
  const port = 9600 + (process.pid % 300);
  const chrome = spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check', '--disable-gpu', '--hide-scrollbars', 'about:blank',
  ], { stdio: 'ignore' });

  let cdp;
  try {
    cdp = await connect(port);
    await cdp.send('Page.enable');
    await cdp.send('Runtime.enable');
    await cdp.send('DOM.enable');
    await cdp.send('CSS.enable');

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
    await cdp.send('Page.navigate', { url: `${BASE}/layout-fixture.html` });
    await settle();
    await evaluate(`localStorage.setItem('aethercore.locale', 'ar')`);
    await cdp.send('Page.navigate', { url: `${BASE}/layout-fixture.html` });
    await settle();
    if (DRAWER) {
      // Opened by its own shortcut, so the gate exercises the path the user has.
      const opened = await evaluate(`(async () => {
        document.body.dispatchEvent(new KeyboardEvent('keydown', { key: '/', ctrlKey: true, bubbles: true }));
        const deadline = Date.now() + 5000;
        while (!document.querySelector('.assistant-drawer') && Date.now() < deadline) {
          await new Promise((r) => setTimeout(r, 25));
        }
        const input = document.querySelector('#assistant-input');
        if (!input) return false;
        // One answered turn, so the checks see a transcript rather than only
        // the empty state: the answer, its markers and its evidence chips are
        // where Arabic meets mono and LTR identifiers.
        const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set;
        setter.call(input, 'ماذا حدث على هذا الجهاز');
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
        await new Promise((r) => setTimeout(r, 2200));
        return Boolean(document.querySelector('.assistant-drawer'));
      })()`);
      if (!opened) throw new Error('could not open the assistant drawer');
      await settle();
    } else if (PAGE !== 'overview') {
      const reached = await evaluate(`(async () => {
        const button = document.querySelector('button[data-nav-item][data-page=' + JSON.stringify(${JSON.stringify(PAGE)}) + ']');
        if (!button) return false;
        button.click();
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        return document.querySelector('.app-shell')?.dataset.page === ${JSON.stringify(PAGE)};
      })()`);
      if (!reached) throw new Error(`could not route to page "${PAGE}"`);
      await settle();
    }

    // 1 — the root is RTL, and says so in every place that matters.
    const root = await evaluate(`({
      dir: document.documentElement.getAttribute('dir'),
      lang: document.documentElement.getAttribute('lang'),
      locale: document.documentElement.dataset.locale,
      computedDirection: getComputedStyle(document.documentElement).direction,
    })`);
    check('root is RTL', 'dir=rtl lang=ar direction=rtl',
      `dir=${root.dir} lang=${root.lang} locale=${root.locale} direction=${root.computedDirection}`,
      root.dir === 'rtl' && root.lang === 'ar' && root.computedDirection === 'rtl');

    // 2 — the layout is genuinely RTL, not a mirrored LTR one. Measured from
    // rendered boxes: the rail must sit on the RIGHT of the viewport, the main
    // region to its left, and prose must resolve to right alignment.
    const layout = await evaluate(`(() => {
      const rail = document.querySelector('.app-sidebar').getBoundingClientRect();
      const main = document.querySelector('main').getBoundingClientRect();
      const prose = document.querySelector(${JSON.stringify(REGION)} + ' p, ' + ${JSON.stringify(REGION)} + ' h1, ' + ${JSON.stringify(REGION)} + ' h2');
      const vw = document.documentElement.clientWidth;
      return {
        railLeft: Math.round(rail.left), railRight: Math.round(rail.right),
        mainLeft: Math.round(main.left), viewport: vw,
        railIsOnRight: rail.left > vw / 2,
        mainStartsAtLeftEdge: main.left < 8,
        proseTextAlign: getComputedStyle(prose).textAlign,
        proseDirection: getComputedStyle(prose).direction,
      };
    })()`);
    check('layout is genuinely RTL, not mirrored LTR',
      'rail on the right half, main flush to the left edge, prose direction rtl',
      `rail ${layout.railLeft}-${layout.railRight} of ${layout.viewport}, main.left=${layout.mainLeft}, `
      + `railIsOnRight=${layout.railIsOnRight} mainStartsAtLeftEdge=${layout.mainStartsAtLeftEdge} `
      + `prose direction=${layout.proseDirection} text-align=${layout.proseTextAlign}`,
      layout.railIsOnRight && layout.mainStartsAtLeftEdge && layout.proseDirection === 'rtl');

    // 3 — technical tokens stay LTR inside RTL prose, so a rule id or a digest
    // keeps its byte order.
    const isolate = await evaluate(`(() => {
      const nodes = [...document.querySelectorAll('.technical-isolate')];
      const rtlParented = nodes.filter((n) => getComputedStyle(n.parentElement).direction === 'rtl');
      return {
        total: nodes.length,
        allLtr: nodes.every((n) => getComputedStyle(n).direction === 'ltr'),
        insideRtl: rtlParented.length,
        sample: nodes.slice(0, 3).map((n) => n.textContent.trim().slice(0, 28)),
      };
    })()`);
    check('technical tokens stay LTR inside RTL prose',
      'every .technical-isolate resolves direction:ltr',
      `${isolate.total} nodes, ${isolate.insideRtl} inside an RTL parent, allLtr=${isolate.allLtr}, e.g. ${JSON.stringify(isolate.sample)}`,
      isolate.total > 0 && isolate.allLtr);

    // 4 — the embedded face is declared and the faces the page needs are loaded.
    // Not all four: a weight no element asks for stays `unloaded` by design, and
    // demanding otherwise would be testing lazy loading rather than the font.
    const faces = await evaluate(`(() => {
      const loaded = [...document.fonts].filter((f) => f.family === ${JSON.stringify(EMBEDDED_FACE)});
      const prose = [...document.querySelectorAll(${JSON.stringify(REGION)} + ' h1, ' + ${JSON.stringify(REGION)} + ' h2, ' + ${JSON.stringify(REGION)} + ' h3, ' + ${JSON.stringify(REGION)} + ' p')]
        .find((el) => /[\u0600-\u06FF]/.test(el.textContent));
      return {
        declared: loaded.map((f) => f.weight + ':' + f.status),
        anyLoaded: loaded.some((f) => f.status === 'loaded'),
        anyError: loaded.some((f) => f.status === 'error'),
        proseStackHead: prose ? getComputedStyle(prose).fontFamily.split(',')[0].replace(/["']/g, '') : null,
      };
    })()`);
    check('embedded face is declared, loaded and first in the Arabic prose stack',
      `4 faces declared, at least one loaded, none in error, Arabic prose asks for "${EMBEDDED_FACE}" first`,
      `declared=${JSON.stringify(faces.declared)} anyLoaded=${faces.anyLoaded} anyError=${faces.anyError} proseStackHead=${JSON.stringify(faces.proseStackHead)}`,
      faces.declared.length === 4 && faces.anyLoaded && !faces.anyError && faces.proseStackHead === EMBEDDED_FACE);

    // 5 — THE ONE THAT MATTERS, across EVERY node on the page that carries
    // Arabic. A system fallback shaping correctly on this Mac would pass every
    // check above and fail on a bare Windows box, so this asks the rendering
    // engine what it actually drew with.
    //
    // The criterion is "no glyph drawn by a non-bundled font", not "every glyph
    // drawn by IBM Plex". In a monospace element the Latin characters correctly
    // draw from JetBrains Mono while the Arabic draws from IBM Plex — that is
    // per-character fallback working, and a stricter rule would fail it wrongly.
    // What must never appear is `isCustomFont: false`.
    const { root: domRoot } = await cdp.send('DOM.getDocument', { depth: -1, pierce: true });
    const { nodeIds } = await cdp.send('DOM.querySelectorAll', {
      nodeId: domRoot.nodeId,
      selector: ['h1', 'h2', 'h3', 'h4', 'p', 'strong', 'span', 'small', 'button']
        .map((tag) => `${REGION} ${tag}`)
        .join(', '),
    });

    const systemDrawn = [];
    const embedded = new Map();
    let arabicNodes = 0;
    let totalGlyphs = 0;
    for (const nodeId of nodeIds) {
      const { outerHTML } = await cdp.send('DOM.getOuterHTML', { nodeId }).catch(() => ({ outerHTML: '' }));
      const inner = outerHTML.replace(/^<[^>]*>/, '').replace(/<\/[^>]*>$/, '');
      if (!/[\u0600-\u06FF]/.test(inner) || /</.test(inner)) continue;  // leaf nodes bearing Arabic only
      arabicNodes += 1;
      const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId }).catch(() => ({ fonts: [] }));
      for (const font of fonts ?? []) {
        totalGlyphs += font.glyphCount;
        if (font.isCustomFont) embedded.set(font.familyName, (embedded.get(font.familyName) ?? 0) + font.glyphCount);
        else systemDrawn.push({ family: font.familyName, glyphs: font.glyphCount });
      }
    }
    const systemGlyphs = systemDrawn.reduce((sum, f) => sum + f.glyphs, 0);
    check('no Arabic glyph is drawn by a system fallback font',
      `0 glyphs from a non-bundled font across every Arabic-bearing node`,
      `${arabicNodes} Arabic nodes, ${totalGlyphs} glyphs; bundled = ${JSON.stringify([...embedded])}; `
      + `system fallback = ${systemGlyphs} glyph(s) ${JSON.stringify(systemDrawn)}`,
      arabicNodes > 0 && totalGlyphs > 0 && systemGlyphs === 0);

    // 6 — the face is shaping, not merely displaying. An Arabic word rendered
    // with contextual joining is materially narrower than the same letters
    // forced into isolated forms; if GSUB were stripped by subsetting, these two
    // would measure the same.
    const shaping = await evaluate(`(() => {
      const make = (text, features) => {
        const el = document.createElement('span');
        el.textContent = text;
        el.style.cssText = 'position:absolute;visibility:hidden;white-space:nowrap;font:400 40px "${EMBEDDED_FACE}";' + features;
        document.body.appendChild(el);
        const w = el.getBoundingClientRect().width;
        el.remove();
        return w;
      };
      const word = 'التشخيص';
      const joined = make(word, '');
      const isolated = make([...word].join('\\u200C'), '');  // ZWNJ blocks joining
      return { word, joined: Math.round(joined), isolated: Math.round(isolated) };
    })()`);
    check('the subset face still shapes Arabic',
      'a joined word renders narrower than the same letters with joining blocked',
      `"${shaping.word}" joined=${shaping.joined}px vs joining-blocked=${shaping.isolated}px`,
      shaping.joined > 0 && shaping.joined < shaping.isolated);

    // 7 — every Arabic string is authored, not machine-substituted: the catalogs
    // are the only source, and a missing key is a compile error, so what is left
    // to check is that no visible Latin leaked into Arabic prose headings.
    const authored = await evaluate(`(() => {
      const heads = [...document.querySelectorAll(${JSON.stringify(REGION)} + ' h1, ' + ${JSON.stringify(REGION)} + ' h2, ' + ${JSON.stringify(REGION)} + ' h3, ' + ${JSON.stringify(REGION)} + ' .eyebrow')]
        .map((el) => el.textContent.trim()).filter(Boolean);
      const untranslated = heads.filter((s) => /^[\\x00-\\x7F]+$/.test(s) && /[a-zA-Z]{4,}/.test(s));
      return { total: heads.length, untranslated: untranslated.slice(0, 8), count: untranslated.length };
    })()`);
    check('headings are authored in Arabic',
      'no all-Latin heading left untranslated (product names excepted)',
      `${authored.total} headings, ${authored.count} all-Latin: ${JSON.stringify(authored.untranslated)}`,
      authored.count === 0);
  } finally {
    cdp?.close();
    chrome.kill();
    await rm(profile, { recursive: true, force: true }).catch(() => {});
  }

  const failed = results.filter((r) => !r.pass);
  console.log(`\n${results.length - failed.length}/${results.length} checks pass`);
  process.exitCode = failed.length ? 1 : 0;
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
