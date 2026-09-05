/**
 * Layout sweep — screenshots and layout measurements of the POPULATED shell.
 *
 * The shell renders empty states when no maintenance service is present, so a
 * browser opened on `index.html` exercises almost none of the real layout. Every
 * responsive regression this project has shipped hid behind exactly that: four
 * responsive screenshots once passed only because there was no data to overflow.
 * This drives `layout-fixture.html`, which installs the sanctioned test transport
 * and replays a representative kernel stream, so what is measured is the shell
 * with data in it.
 *
 * It reports measurements, not opinions. Every check has an expected value and
 * prints the observed number beside it.
 *
 *   node tools/layout-sweep.mjs --out ../../output/sweep
 *   node tools/layout-sweep.mjs --widths 1280,1024,960 --locales en,ar
 *
 * Requires a running dev server (npm run dev) and Google Chrome.
 */
import { spawn } from 'node:child_process';
import { mkdir, writeFile, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = process.env.CHROME_BIN
  ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

function parseArgs(argv) {
  // Both themes by default. Every measurement of this app through the whole
  // design port ran `themes: ['dark']`, so no light artifact was ever produced
  // and nobody saw that feature-layout.css paints dark surfaces the light theme
  // never overrides — near-black text on near-black cards (DBT-P47-001).
  const args = { widths: [1280, 1024, 960], locales: ['en', 'ar'], themes: ['dark', 'light'], out: 'output/sweep', base: 'http://127.0.0.1:1420', pages: ['overview'], entry: 'layout-fixture.html' };
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.replace(/^--/, '');
    const value = argv[i + 1];
    if (!key || value === undefined) continue;
    if (key === 'widths') args.widths = value.split(',').map(Number);
    else if (key === 'locales') args.locales = value.split(',');
    else if (key === 'themes') args.themes = value.split(',');
    else if (key === 'pages') args.pages = value.split(',');
    else if (key in args) args[key] = value;
  }
  return args;
}

/** Minimal CDP client. Node's global WebSocket is enough; no dependency needed. */
async function connect(port) {
  let info;
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      const targets = await response.json();
      info = targets.find((target) => target.type === 'page');
      if (info) break;
    } catch { /* chrome not up yet */ }
    await new Promise((r) => setTimeout(r, 100));
  }
  if (!info) throw new Error('chrome devtools endpoint never came up');

  const socket = new WebSocket(info.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { socket.onopen = ok; socket.onerror = fail; });
  // After the handshake an unhandled 'error' event terminates the process, and
  // Chrome drops the socket whenever it tears a target down between navigations.
  // Fail the in-flight calls instead, so a long sweep reports rather than dies.
  let closed = null;
  socket.onerror = (event) => { closed = new Error(`devtools socket error: ${event?.message ?? 'unknown'}`); };
  socket.onclose = () => { closed ??= new Error('devtools socket closed'); };

  let nextId = 0;
  const pending = new Map();
  const events = new Map();
  socket.onmessage = (message) => {
    const frame = JSON.parse(message.data);
    if (frame.id !== undefined) {
      const entry = pending.get(frame.id);
      pending.delete(frame.id);
      if (!entry) return;
      if (frame.error) entry.fail(new Error(`${frame.error.message} (${entry.method})`));
      else entry.ok(frame.result);
      return;
    }
    for (const handler of events.get(frame.method) ?? []) handler(frame.params);
  };
  socket.addEventListener('close', () => {
    for (const entry of pending.values()) entry.fail(closed ?? new Error('devtools socket closed'));
    pending.clear();
  });

  const send = (method, params = {}) => new Promise((ok, fail) => {
    if (closed) { fail(closed); return; }
    const id = (nextId += 1);
    pending.set(id, { ok, fail, method });
    try { socket.send(JSON.stringify({ id, method, params })); }
    catch (error) { pending.delete(id); fail(error); }
  });
  const on = (method, handler) => events.set(method, [...(events.get(method) ?? []), handler]);
  return { send, on, close: () => socket.close() };
}

/** Waits for the page to be quiet: load fired, fonts settled, two frames drawn. */
async function settle(cdp) {
  await cdp.send('Runtime.evaluate', {
    awaitPromise: true,
    expression: `(async () => {
      if (document.readyState !== 'complete') await new Promise((r) => addEventListener('load', r, { once: true }));
      // The fixture mounts the app from a dynamic import, so 'load' fires before
      // there is a shell to measure. Wait for the shell itself, not for the document.
      const deadline = Date.now() + 15000;
      while (!document.querySelector('.app-shell') && Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 25));
      }
      await document.fonts.ready;
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

/**
 * The layout assertions, run inside the page.
 *
 * Overlap is measured between real rendered boxes rather than inferred from CSS,
 * because the failure this exists to catch — a button placed physically over the
 * body text at 960 — is invisible to any rule-level check.
 */
const MEASURE = `(() => {
  const doc = document.documentElement;
  const overflow = Math.max(0, doc.scrollWidth - doc.clientWidth);

  const clipped = [];
  const overlaps = [];
  const nodes = [...document.querySelectorAll('main *')].filter((el) => {
    const style = getComputedStyle(el);
    return style.display !== 'none' && style.visibility !== 'hidden' && el.getClientRects().length > 0;
  });

  for (const el of nodes) {
    // Text cut off by its own box: scroll extent beyond the padding box while
    // overflow is hidden or clipped.
    const style = getComputedStyle(el);
    const hiddenX = style.overflowX === 'hidden' || style.overflowX === 'clip';
    const hiddenY = style.overflowY === 'hidden' || style.overflowY === 'clip';
    const ellipsis = style.textOverflow === 'ellipsis';
    if (hiddenY && el.scrollHeight - el.clientHeight > 1 && el.textContent.trim()) {
      clipped.push({ tag: el.tagName.toLowerCase(), cls: el.className?.toString().slice(0, 60), by: el.scrollHeight - el.clientHeight, axis: 'y' });
    }
    if (hiddenX && !ellipsis && el.scrollWidth - el.clientWidth > 1 && el.textContent.trim()) {
      clipped.push({ tag: el.tagName.toLowerCase(), cls: el.className?.toString().slice(0, 60), by: el.scrollWidth - el.clientWidth, axis: 'x' });
    }
  }

  // A control sitting on top of prose. Only leaf controls against leaf text, and
  // only when neither is an ancestor of the other, so ordinary nesting is quiet.
  const controls = [...document.querySelectorAll('main button, main a, main [role="button"], main input')];
  const prose = [...document.querySelectorAll('main p, main h1, main h2, main h3, main h4, main li, main td')]
    .filter((el) => el.textContent.trim().length > 12);
  for (const control of controls) {
    const a = control.getBoundingClientRect();
    if (!a.width || !a.height) continue;
    for (const text of prose) {
      if (control.contains(text) || text.contains(control)) continue;
      const b = text.getBoundingClientRect();
      if (!b.width || !b.height) continue;
      const w = Math.min(a.right, b.right) - Math.max(a.left, b.left);
      const h = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
      if (w > 2 && h > 2) {
        overlaps.push({
          control: (control.textContent || control.getAttribute('aria-label') || control.tagName).trim().slice(0, 40),
          text: text.textContent.trim().slice(0, 40),
          area: Math.round(w * h),
        });
      }
    }
  }

  const band = document.querySelector('.policy-band');
  const bandBox = band?.getBoundingClientRect();

  return {
    overflowX: overflow,
    scrollWidth: doc.scrollWidth,
    clientWidth: doc.clientWidth,
    clipped: clipped.slice(0, 12),
    clippedCount: clipped.length,
    overlaps: overlaps.slice(0, 12),
    overlapCount: overlaps.length,
    dir: doc.getAttribute('dir'),
    lang: doc.getAttribute('lang'),
    locale: doc.dataset.locale,
    theme: doc.dataset.theme,
    policyBandVisible: Boolean(band) && bandBox.width > 0 && bandBox.height > 0,
    deniedChips: document.querySelectorAll('.policy-denied-chip').length,
    evidenceChips: document.querySelectorAll('[data-evidence-chip]').length,
    emptyStates: document.querySelectorAll('.empty-state').length,
    // A meter at rest must read em dash, never a zero nobody measured.
    emDashes: (document.querySelector('main')?.textContent.match(/—/g) ?? []).length,
  };
})()`;

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const outDir = resolve(args.out);
  await mkdir(outDir, { recursive: true });

  const profile = join(tmpdir(), `aethercore-sweep-${process.pid}`);
  const port = 9222 + (process.pid % 500);
  const chrome = spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check', '--disable-gpu',
    '--force-device-scale-factor=1', '--hide-scrollbars', 'about:blank',
  ], { stdio: 'ignore' });

  const results = [];
  let cdp;
  try {
    cdp = await connect(port);
    await cdp.send('Page.enable');
    await cdp.send('Runtime.enable');

    for (const page of args.pages) {
      for (const locale of args.locales) {
        for (const theme of args.themes) {
          for (const width of args.widths) {
            await cdp.send('Emulation.setDeviceMetricsOverride', {
              width, height: 900, deviceScaleFactor: 1, mobile: false,
            });
            // Preferences are read from storage during boot, so they must be set
            // before the document that reads them is created.
            await cdp.send('Page.navigate', { url: `${args.base}/${args.entry}` });
            await settle(cdp);
            await evaluate(cdp, `(() => {
              localStorage.setItem('aethercore.locale', ${JSON.stringify(locale)});
              localStorage.setItem('aethercore.theme', ${JSON.stringify(theme)});
            })()`);
            await cdp.send('Page.navigate', { url: `${args.base}/${args.entry}` });
            await settle(cdp);
            // Navigate the way a user does, by pressing the rail button, so the
            // measured page is one the app actually routed to.
            const reached = await evaluate(cdp, `(async () => {
              const button = document.querySelector('button[data-nav-item][data-page=' + JSON.stringify(${JSON.stringify(page)}) + ']');
              if (!button) return false;
              button.click();
              await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
              return document.querySelector('.app-shell')?.dataset.page === ${JSON.stringify(page)};
            })()`);
            if (!reached) throw new Error(`could not route to page "${page}"`);
            await settle(cdp);

            const measured = await evaluate(cdp, MEASURE);
            const name = `${page}-${width}-${locale}-${theme}`;
            const shot = await cdp.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
            await writeFile(join(outDir, `${name}.png`), Buffer.from(shot.data, 'base64'));
            results.push({ name, width, locale, theme, page, ...measured });

            const status = measured.overflowX === 0 && measured.overlapCount === 0 && measured.clippedCount === 0 ? 'PASS' : 'FAIL';
            console.log(
              `${status}  ${name.padEnd(28)} overflowX=${measured.overflowX} `
              + `clipped=${measured.clippedCount} overlaps=${measured.overlapCount} `
              + `dir=${measured.dir} band=${measured.policyBandVisible} `
              + `denied=${measured.deniedChips} evidence=${measured.evidenceChips} `
              + `empty=${measured.emptyStates} emdash=${measured.emDashes}`,
            );
            for (const entry of measured.clipped) console.log(`        clipped ${entry.axis} by ${entry.by}px: ${entry.tag}.${entry.cls}`);
            for (const entry of measured.overlaps) console.log(`        overlap ${entry.area}px²: "${entry.control}" over "${entry.text}"`);
          }
        }
      }
    }
    await writeFile(join(outDir, 'sweep.json'), JSON.stringify(results, null, 2));
  } finally {
    cdp?.close();
    chrome.kill();
    await rm(profile, { recursive: true, force: true }).catch(() => {});
  }

  const failed = results.filter((r) => r.overflowX !== 0 || r.overlapCount !== 0 || r.clippedCount !== 0);
  console.log(`\n${results.length - failed.length}/${results.length} pass · artifacts in ${outDir}`);
  process.exitCode = failed.length ? 1 : 0;
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
