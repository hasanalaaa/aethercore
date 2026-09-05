/**
 * Contrast sweep — every rendered text node against the surface it is actually
 * painted on, in both themes.
 *
 * `layout-sweep.mjs` measures geometry: overflow, clipping, overlap. Light theme
 * passes all of it, which is exactly why DBT-P47-001 survived a full design port
 * and a screenshot review. `feature-layout.css` carried 440 literal colours and
 * zero `data-theme` rules, so under the light theme every surface it painted
 * stayed dark while the text turned dark: near-black on near-black, 1.02:1
 * measured, on eight of eleven screens.
 *
 * Geometry cannot see that. This can, and it is committed rather than run once,
 * because an uncommitted measurement is how this class of defect comes back.
 *
 *   node tools/contrast-sweep.mjs
 *   node tools/contrast-sweep.mjs --themes light --pages overview,drivers
 *   node tools/contrast-sweep.mjs --min 4.5 --json out.json
 *
 * Requires `npm run fixture` (the populated shell) and Google Chrome. Exits 1 if
 * any text/background pair falls below --min (default 4.5, WCAG AA for body text).
 *
 * WHAT IT MEASURES, and the two places a naive version gets it wrong:
 *
 *  1. The BACKDROP IS COMPOSITED, not read off the node. `background-color` on a
 *     text node is usually `rgba(0,0,0,0)`; the visible surface is whatever the
 *     nearest opaque ancestor paints, with every translucent layer between them
 *     composited over it in order. Reading the parent's declared colour reports a
 *     transparent black and invents failures that are not there.
 *  2. The TEXT COLOUR IS COMPOSITED TOO. This palette states text as
 *     `rgba(8,12,18,0.95)`, not a hex, so alpha has to be resolved against the
 *     same backdrop or every reading is wrong by the alpha.
 *
 * Only nodes that actually paint a glyph are measured: non-empty text, non-zero
 * box, visible, and not `visibility:hidden`/`opacity:0`.
 */
import { spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = process.env.CHROME_BIN
  ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

const ALL_PAGES = ['overview', 'deepScan', 'drivers', 'repair', 'cleanup', 'startup',
  'performance', 'hardware', 'crash', 'activity', 'fleet'];

function parseArgs(argv) {
  const args = {
    pages: ALL_PAGES, locales: ['en'], themes: ['dark', 'light'], width: 1280,
    min: 4.5, base: 'http://127.0.0.1:1420', entry: 'layout-fixture.html', json: '',
  };
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.replace(/^--/, '');
    const value = argv[i + 1];
    if (!key || value === undefined) continue;
    if (key === 'pages') args.pages = value.split(',');
    else if (key === 'locales') args.locales = value.split(',');
    else if (key === 'themes') args.themes = value.split(',');
    else if (key === 'width') args.width = Number(value);
    else if (key === 'min') args.min = Number(value);
    else if (key in args) args[key] = value;
  }
  return args;
}

async function connect(port) {
  let info;
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      info = (await response.json()).find((t) => t.type === 'page');
      if (info) break;
    } catch { /* chrome not up yet */ }
    await new Promise((r) => setTimeout(r, 100));
  }
  if (!info) throw new Error('chrome devtools endpoint never came up');
  const socket = new WebSocket(info.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { socket.onopen = ok; socket.onerror = fail; });
  let closed = null;
  socket.onerror = (e) => { closed = new Error(`devtools socket error: ${e?.message ?? 'unknown'}`); };
  socket.onclose = () => { closed ??= new Error('devtools socket closed'); };
  let nextId = 0;
  const pending = new Map();
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id === undefined) return;
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    if (message.error) entry.fail(new Error(message.error.message));
    else entry.ok(message.result);
  };
  return {
    send(method, params = {}) {
      if (closed) return Promise.reject(closed);
      const id = (nextId += 1);
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((ok, fail) => pending.set(id, { ok, fail }));
    },
    close() { socket.close(); },
  };
}

async function settle(cdp) {
  await cdp.send('Runtime.evaluate', {
    awaitPromise: true,
    expression: `(async () => {
      if (document.readyState !== 'complete') await new Promise((r) => addEventListener('load', r, { once: true }));
      const deadline = Date.now() + 15000;
      while (!document.querySelector('.app-shell') && Date.now() < deadline) await new Promise((r) => setTimeout(r, 25));
      await document.fonts.ready;
      // Wait for ANIMATIONS, not just frames. The shell fades each page in, and
      // two requestAnimationFrames land while '.ac-page-frame' is still at
      // opacity 0.06 — every colour underneath it composites to something the
      // user never sees. A geometry sweep survives that; a colour sweep cannot.
      const settleDeadline = Date.now() + 8000;
      while (Date.now() < settleDeadline) {
        const running = document.getAnimations().filter((a) => a.playState === 'running');
        if (running.length === 0) break;
        try { await Promise.race([Promise.all(running.map((a) => a.finished)), new Promise((r) => setTimeout(r, 500))]); }
        catch { /* an animation cancelled mid-wait is not an error here */ }
      }
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    })()`,
  });
}

async function evaluate(cdp, expression) {
  const { result, exceptionDetails } = await cdp.send('Runtime.evaluate', {
    expression, returnByValue: true, awaitPromise: true,
  });
  if (exceptionDetails) throw new Error(exceptionDetails.text + ' ' + (exceptionDetails.exception?.description ?? ''));
  return result.value;
}

/** Runs in the page. Returns one record per glyph-painting text node. */
const MEASURE = `(() => {
  const parse = (css) => {
    const m = String(css).match(/rgba?\\(([^)]+)\\)/);
    if (!m) return null;
    const p = m[1].split(',').map((x) => parseFloat(x.trim()));
    return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
  };
  const over = (fg, bg) => ({
    r: fg.r * fg.a + bg.r * (1 - fg.a),
    g: fg.g * fg.a + bg.g * (1 - fg.a),
    b: fg.b * fg.a + bg.b * (1 - fg.a),
    a: 1,
  });
  const lum = (c) => {
    const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
    return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
  };
  const ratio = (a, b) => {
    const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
    return (hi + 0.05) / (lo + 0.05);
  };
  // The composited backdrop: walk up collecting every painted layer, stop at the
  // first fully opaque one, then composite back down in paint order.
  // A gradient is painted as background-IMAGE, so backgroundColor reports
  // rgba(0,0,0,0) and a colour-only walk sees straight through it to whatever is
  // behind. That is not a rounding error: this app paints dark gradient panels
  // that the light theme never overrode, and a tick drawn on one of them was
  // scored against the light card BEHIND it and passed. Average the gradient's
  // own colour stops and treat the result as the layer it visually is.
  const gradientLayer = (bgImage) => {
    if (!bgImage || bgImage === 'none' || !/gradient\\(/.test(bgImage)) return null;
    const stops = bgImage.match(/rgba?\\([^)]+\\)/g);
    if (!stops || stops.length === 0) return null;
    const parsed = stops.map(parse).filter(Boolean);
    if (parsed.length === 0) return null;
    const n = parsed.length;
    return {
      r: parsed.reduce((a, c) => a + c.r, 0) / n,
      g: parsed.reduce((a, c) => a + c.g, 0) / n,
      b: parsed.reduce((a, c) => a + c.b, 0) / n,
      a: parsed.reduce((a, c) => a + c.a, 0) / n,
    };
  };
  const backdrop = (el) => {
    const layers = [];
    for (let n = el; n; n = n.parentElement) {
      const s = getComputedStyle(n);
      const c = parse(s.backgroundColor);
      const g = gradientLayer(s.backgroundImage);
      const o = parseFloat(s.opacity);
      // Paint order within one element: background-color first, image over it.
      if (c && c.a > 0) layers.push(o < 1 ? { ...c, a: c.a * o } : c);
      if (g && g.a > 0) layers.push(o < 1 ? { ...g, a: g.a * o } : g);
      const opaque = (g && g.a >= 1) || (c && c.a >= 1);
      if (opaque && o >= 1) return layers.reverse().reduce((acc, l) => over(l, acc), { r: 255, g: 255, b: 255, a: 1 });
    }
    // Nothing opaque above: the viewport ground is the canvas colour.
    const root = parse(getComputedStyle(document.documentElement).backgroundColor)
      ?? parse(getComputedStyle(document.body).backgroundColor)
      ?? { r: 255, g: 255, b: 255, a: 1 };
    const base = root.a >= 1 ? root : { r: 255, g: 255, b: 255, a: 1 };
    return layers.reverse().reduce((acc, l) => over(l, acc), base);
  };
  const out = [];
  const seen = new Set();
  for (const el of document.querySelectorAll('*')) {
    if (seen.has(el)) continue;
    // Only nodes that own a glyph: direct text children with visible characters.
    let text = '';
    for (const n of el.childNodes) if (n.nodeType === 3) text += n.nodeValue;
    text = text.replace(/\\s+/g, ' ').trim();
    if (!text) continue;
    const s = getComputedStyle(el);
    if (s.visibility === 'hidden' || s.display === 'none' || parseFloat(s.opacity) === 0) continue;
    const box = el.getBoundingClientRect();
    if (box.width < 1 || box.height < 1) continue;
    const fg = parse(s.color);
    if (!fg || fg.a === 0) continue;
    const bg = backdrop(el);
    const composited = over(fg, bg);
    const r = ratio(composited, bg);
    // WCAG large text is 18.66px bold or 24px; those get 3:1, not 4.5:1.
    const px = parseFloat(s.fontSize);
    const weight = parseInt(s.fontWeight, 10) || 400;
    const large = px >= 24 || (px >= 18.66 && weight >= 700);
    // WCAG 2.2 SC 1.4.3 Contrast (Minimum), Incidental: "Text or images of text
    // that are part of an inactive user interface component ... have no contrast
    // requirement." A disabled control is dimmed ON PURPOSE, and raising its
    // contrast to pass would make it look enabled. These are REPORTED as exempt
    // rather than dropped, so the exemption is visible and cannot become a place
    // to hide a real failure.
    const inactive = Boolean(el.closest('[disabled], [aria-disabled="true"], fieldset:disabled'));
    out.push({
      inactive,
      tag: el.tagName.toLowerCase(),
      cls: (el.className && typeof el.className === 'string' ? el.className : '').split(' ').filter(Boolean).slice(0, 3).join('.'),
      text: text.slice(0, 42),
      fg: s.color, bg: 'rgb(' + [bg.r, bg.g, bg.b].map(Math.round).join(', ') + ')',
      ratio: Math.round(r * 100) / 100, large, px, weight,
    });
  }
  return out;
})()`;

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = 9333 + Math.floor(Math.random() * 300);
  const profile = mkdtempSync(join(tmpdir(), 'aethercore-contrast-'));
  const chrome = spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check', '--disable-gpu',
    '--force-device-scale-factor=1', '--hide-scrollbars', 'about:blank',
  ], { stdio: 'ignore' });

  const failures = [];
  let measured = 0;
  let exemptCount = 0;
  let cdp;
  try {
    cdp = await connect(port);
    await cdp.send('Page.enable');
    await cdp.send('Runtime.enable');
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: args.width, height: 900, deviceScaleFactor: 1, mobile: false });

    for (const theme of args.themes) {
      for (const locale of args.locales) {
        for (const page of args.pages) {
          await cdp.send('Page.navigate', { url: `${args.base}/${args.entry}` });
          await settle(cdp);
          await evaluate(cdp, `(() => {
            localStorage.setItem('aethercore.locale', ${JSON.stringify(locale)});
            localStorage.setItem('aethercore.theme', ${JSON.stringify(theme)});
          })()`);
          await cdp.send('Page.navigate', { url: `${args.base}/${args.entry}` });
          await settle(cdp);
          const reached = await evaluate(cdp, `(async () => {
            const b = document.querySelector('button[data-nav-item][data-page=' + JSON.stringify(${JSON.stringify(page)}) + ']');
            if (!b) return false;
            b.click();
            await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
            return document.querySelector('.app-shell')?.dataset.page === ${JSON.stringify(page)};
          })()`);
          if (!reached) throw new Error(`could not route to page "${page}"`);
          await settle(cdp);

          const nodes = await evaluate(cdp, MEASURE);
          measured += nodes.length;
          const exempt = nodes.filter((n) => n.inactive && n.ratio < (n.large ? 3 : args.min));
          exemptCount += exempt.length;
          const bad = nodes.filter((n) => !n.inactive && n.ratio < (n.large ? 3 : args.min));
          // The worst READING, excluding exempt inactive controls: a PASS line
          // reporting worst=2.23:1 reads like a contradiction, and the 2.23 is a
          // disabled button that is meant to be dim. Exempt nodes are counted and
          // printed on their own line above, so nothing is hidden by this.
          const worst = nodes.filter((n) => !n.inactive)
            .reduce((a, n) => (a && a.ratio <= n.ratio ? a : n), null);
          console.log(
            `${bad.length === 0 ? 'PASS' : 'FAIL'}  ${`${page}/${locale}/${theme}`.padEnd(30)} `
            + `nodes=${String(nodes.length).padStart(4)} below=${String(bad.length).padStart(4)} `
            + `exempt=${String(exempt.length).padStart(2)} `
            + `worst=${worst ? worst.ratio.toFixed(2) : 'n/a'}:1`,
          );
          for (const n of exempt) {
            console.log(`        EXEMPT (inactive control) ${n.ratio.toFixed(2)}:1  ${n.tag}${n.cls ? '.' + n.cls : ''}  "${n.text}"`);
          }
          for (const n of bad.slice(0, 8)) {
            console.log(`        ${n.ratio.toFixed(2)}:1  ${n.tag}${n.cls ? '.' + n.cls : ''}  ${n.fg} on ${n.bg}  "${n.text}"`);
          }
          if (bad.length > 8) console.log(`        … and ${bad.length - 8} more`);
          for (const n of bad) failures.push({ page, locale, theme, ...n });
        }
      }
    }
  } finally {
    cdp?.close();
    chrome.kill();
    // Chrome unlinks its profile asynchronously after SIGTERM, so an immediate
    // rmdir races it and throws ENOTEMPTY *after* the measurement is complete —
    // losing the summary to a cleanup failure. Cleanup is not a measurement:
    // retry briefly, then leave the temp directory to the OS.
    for (let attempt = 0; attempt < 20; attempt += 1) {
      try { await rm(profile, { recursive: true, force: true }); break; }
      catch { await new Promise((r) => setTimeout(r, 100)); }
    }
  }

  const distinct = new Set(failures.map((f) => `${f.tag}.${f.cls}|${f.fg}|${f.bg}`));
  console.log(`\nmeasured ${measured} text node(s); ${failures.length} below threshold, ${distinct.size} distinct`
    + `; ${exemptCount} exempt as inactive controls (WCAG 1.4.3 Incidental)`);
  for (const theme of args.themes) {
    const t = failures.filter((f) => f.theme === theme);
    console.log(`  theme=${theme.padEnd(6)} ${String(t.length).padStart(4)} below, `
      + `${new Set(t.map((f) => `${f.tag}.${f.cls}|${f.fg}|${f.bg}`)).size} distinct`);
  }
  if (args.json) await writeFile(args.json, JSON.stringify(failures, null, 2));
  if (failures.length) {
    console.log(`\nFAIL — ${failures.length} text node(s) below WCAG AA (${args.min}:1 body, 3:1 large).`);
    process.exitCode = 1;
  } else {
    console.log('\nPASS — every rendered text node meets WCAG AA in every theme measured.');
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
