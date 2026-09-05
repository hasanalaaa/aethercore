/**
 * One SVG in, the whole shipped icon set out.
 *
 * Before this, `apps/desktop/icons/` held a committed raster set whose only
 * ancestor was a 1024px PNG sitting beside it. Nothing recorded which file
 * produced which, nothing could regenerate them, and replacing the artwork
 * meant hand-exporting seventeen files at seventeen sizes.
 *
 *   node tools/icon-pipeline/build-icons.mjs --source design/icon/aethercore-mark.svg
 *   node tools/icon-pipeline/build-icons.mjs --check     # regenerate to a temp dir
 *                                                        # and diff against what is
 *                                                        # committed; exit 1 on drift
 *
 * Rasterisation is Chrome headless, which this repo already requires for the UI
 * verification tools — no new dependency, and the same engine that renders the
 * product renders its icon. The `.ico` and `.icns` containers are assembled here
 * in plain JS: both are short, documented formats, and every library that does
 * it would be a dependency carried for two functions.
 *
 * The artwork itself is an OWNER DECISION and is not settled (DBT-P36-004).
 * This pipeline does not choose it. `--source` takes whatever SVG the owner
 * picks; `icons/SOURCE.json` records which one produced the committed set, by
 * path and sha256, so the set always has exactly one traceable ancestor.
 */
import { spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve, dirname, basename } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const CHROME = process.env.CHROME_BIN ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const ICONS = join(ROOT, 'apps', 'desktop', 'icons');
// The artwork lives in design/ at the repository root, one level above this
// workspace, so SOURCE.json records paths relative to the repo -- an absolute
// path would name one machine's home directory and resolve nowhere else.
const REPO = resolve(ROOT, '..');

/**
 * Every raster the product ships, and who consumes it. Anything not on this
 * list is not generated; anything on it is regenerated on every run, so the set
 * cannot drift into a mix of two artworks.
 */
const PNG_TARGETS = [
  ['32x32.png', 32],                // tauri
  ['64x64.png', 64],                // tauri
  ['128x128.png', 128],             // tauri
  ['128x128@2x.png', 256],          // tauri
  ['icon.png', 1024],               // tauri, and the window icon
  ['StoreLogo.png', 50],            // MSIX / Store listing
  ['Square30x30Logo.png', 30],
  ['Square44x44Logo.png', 44],
  ['Square71x71Logo.png', 71],
  ['Square89x89Logo.png', 89],
  ['Square107x107Logo.png', 107],
  ['Square142x142Logo.png', 142],
  ['Square150x150Logo.png', 150],
  ['Square284x284Logo.png', 284],
  ['Square310x310Logo.png', 310],
];
/** Windows .ico. 256 is the largest Explorer asks for; the rest are shell sizes. */
const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256];
/** macOS .icns, as (chunk type, pixel size) — the modern PNG-bearing types. */
const ICNS_CHUNKS = [
  ['icp4', 16], ['icp5', 32], ['ic11', 32], ['ic12', 64],
  ['ic07', 128], ['ic13', 256], ['ic08', 256], ['ic14', 512],
  ['ic09', 512], ['ic10', 1024],
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * One browser for the whole set, driven over CDP.
 *
 * The obvious form -- `chrome --headless --screenshot` once per size -- works
 * and is unusable: each launch builds a fresh profile, and twenty sizes took
 * over two minutes each on this machine. `Emulation.setDeviceMetricsOverride`
 * plus `Page.captureScreenshot` gives the same pixels from one process.
 */
async function connect(profile) {
  // Chrome picks the port and writes it to DevToolsActivePort. Choosing one
  // here and hoping it is free is how this tool first failed: a stale headless
  // process from an earlier run already held the guessed port.
  let port;
  for (let i = 0; i < 300 && !port; i += 1) {
    try { port = Number(readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]); }
    catch { await sleep(100); }
  }
  if (!port) throw new Error('chrome never wrote DevToolsActivePort');
  // `--headless=new ... about:blank` does not reliably leave a page target in
  // /json/list -- under load this machine returned an empty list every time, and
  // that is what made the first two versions of this tool look like they hung.
  // Ask for one explicitly instead of waiting for one that may never appear.
  let info;
  for (let i = 0; i < 100 && !info; i += 1) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      info = targets.find((t) => t.type === 'page');
      if (!info) info = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
    } catch { /* not up yet */ }
    if (!info) await sleep(100);
  }
  if (!info?.webSocketDebuggerUrl) throw new Error('chrome devtools endpoint never came up');
  const socket = new WebSocket(info.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { socket.onopen = ok; socket.onerror = fail; });
  // A dropped socket must fail the calls in flight. Without this the first
  // version of this tool waited forever on a promise Chrome would never answer,
  // which looked exactly like a slow render and cost two debugging rounds.
  let closed = null;
  socket.onerror = (event) => { closed = new Error(`devtools socket error: ${event?.message ?? 'unknown'}`); };
  socket.onclose = () => { closed ??= new Error('devtools socket closed'); };
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
  socket.addEventListener('close', () => {
    for (const entry of pending.values()) entry.fail(closed ?? new Error('devtools socket closed'));
    pending.clear();
  });
  return {
    send: (method, params = {}) => new Promise((ok, fail) => {
      const id = (nextId += 1);
      pending.set(id, { ok, fail });
      socket.send(JSON.stringify({ id, method, params }));
    }),
    close: () => socket.close(),
  };
}

async function renderAll(svgPath, sizes) {
  const profile = mkdtempSync(join(tmpdir(), 'aethercore-icon-chrome-'));
  // Transparency is set over CDP, not with --default-background-color=00000000.
  // That flag is accepted and then kills the devtools socket the moment it is
  // opened on this Chrome (152.0.7977.76): the same script runs in 2 s without
  // it and dies at the WebSocket handshake with it. Measured both ways.
  const chrome = spawn(CHROME, [
    '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
    '--no-first-run', '--disable-gpu', '--hide-scrollbars',
    'about:blank',
  ], { stdio: 'ignore' });
  const out = new Map();
  let cdp;
  try {
    cdp = await connect(profile);
    await cdp.send('Page.enable');
    await cdp.send('Runtime.enable');
    // The tile has rounded corners; without this they come back white.
    await cdp.send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 0, g: 0, b: 0, a: 0 } });
    // The SVG declares width/height in px; a wrapper page lets it fill the
    // viewport exactly at every size instead of rendering at its declared one.
    // Strip width/height from the ROOT element only, so the artwork scales to the
    // viewport. A global replace also ate `<rect width="64" height="64">` and the
    // tile rendered with no background at all -- caught by looking at a 128px PNG,
    // not by any of the size assertions, which were all still green.
    const svg = readFileSync(svgPath, 'utf8').replace(
      /<svg\b[^>]*>/,
      (tag) => tag.replace(/\s(?:width|height)="[^"]*"/g, ''),
    );
    // A file:// wrapper, not a data: URL. Chrome refuses to navigate a top-level
    // frame to a data: URL and tears the target down, which drops the devtools
    // socket mid-run -- the failure that made this tool look like it hung.
    const wrapper = join(profile, 'icon-source.html');
    writeFileSync(wrapper, `<!doctype html><meta charset="utf-8"><style>html,body{margin:0;padding:0;background:transparent;overflow:hidden}svg{display:block;width:100vw;height:100vh}</style>${svg}`);
    const page = `file://${wrapper}`;
    for (const size of sizes) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: size, height: size, deviceScaleFactor: 1, mobile: false });
      await cdp.send('Page.navigate', { url: page });
      await sleep(60);
      // captureBeyondViewport:true is what layout-sweep.mjs uses and what this
      // Chrome tolerates; false dropped the devtools socket on the first shot.
      const shot = await cdp.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
      const png = Buffer.from(shot.data, 'base64');
      const [w, h] = pngSize(png);
      if (w !== size || h !== size) throw new Error(`rendered ${w}x${h}, expected ${size}x${size}`);
      out.set(size, png);
    }
  } finally {
    cdp?.close();
    chrome.kill();
    // Chrome unlinks its own profile files as it exits; removing the directory
    // the same millisecond races that and throws ENOTEMPTY.
    await new Promise((done) => { chrome.once('exit', done); setTimeout(done, 5000); });
    rmSync(profile, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  }
  return out;
}

/** IHDR is always the first chunk: 8-byte signature, 4 length, 4 type, then w/h. */
function pngSize(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG');
  return [buf.readUInt32BE(16), buf.readUInt32BE(20)];
}

function buildIco(pngs) {
  const count = pngs.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);       // reserved
  header.writeUInt16LE(1, 2);       // type 1 = icon
  header.writeUInt16LE(count, 4);
  const dir = Buffer.alloc(16 * count);
  let offset = 6 + 16 * count;
  pngs.forEach(({ size, data }, i) => {
    const at = i * 16;
    dir.writeUInt8(size >= 256 ? 0 : size, at + 0);   // 0 means 256
    dir.writeUInt8(size >= 256 ? 0 : size, at + 1);
    dir.writeUInt8(0, at + 2);                        // palette entries
    dir.writeUInt8(0, at + 3);                        // reserved
    dir.writeUInt16LE(1, at + 4);                     // colour planes
    dir.writeUInt16LE(32, at + 6);                    // bits per pixel
    dir.writeUInt32LE(data.length, at + 8);
    dir.writeUInt32LE(offset, at + 12);
    offset += data.length;
  });
  return Buffer.concat([header, dir, ...pngs.map((p) => p.data)]);
}

function buildIcns(bySize) {
  const chunks = ICNS_CHUNKS.map(([type, size]) => {
    const data = bySize.get(size);
    const head = Buffer.alloc(8);
    head.write(type, 0, 4, 'ascii');
    head.writeUInt32BE(8 + data.length, 4);           // length INCLUDES the header
    return Buffer.concat([head, data]);
  });
  const body = Buffer.concat(chunks);
  const head = Buffer.alloc(8);
  head.write('icns', 0, 4, 'ascii');
  head.writeUInt32BE(8 + body.length, 4);
  return Buffer.concat([head, body]);
}

async function generate(svgPath, outDir) {
  const sizes = new Set([...PNG_TARGETS.map(([, s]) => s), ...ICO_SIZES, ...ICNS_CHUNKS.map(([, s]) => s)]);
  const bySize = await renderAll(svgPath, [...sizes].sort((a, b) => a - b));
  const written = [];
  mkdirSync(outDir, { recursive: true });
  for (const [name, size] of PNG_TARGETS) {
    writeFileSync(join(outDir, name), bySize.get(size));
    written.push(name);
  }
  writeFileSync(join(outDir, 'icon.ico'), buildIco(ICO_SIZES.map((size) => ({ size, data: bySize.get(size) }))));
  written.push('icon.ico');
  writeFileSync(join(outDir, 'icon.icns'), buildIcns(bySize));
  written.push('icon.icns');
  return written;
}

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

async function main() {
  const argv = process.argv.slice(2);
  const check = argv.includes('--check');
  const sourceArg = argv[argv.indexOf('--source') + 1];
  const manifestPath = join(ICONS, 'SOURCE.json');

  let source;
  if (argv.includes('--source')) {
    source = resolve(process.cwd(), sourceArg);
  } else if (existsSync(manifestPath)) {
    source = resolve(REPO, JSON.parse(readFileSync(manifestPath, 'utf8')).source);
  } else {
    throw new Error('--source <file.svg> is required the first time; afterwards icons/SOURCE.json records it');
  }
  if (!existsSync(source)) throw new Error(`source not found: ${source}`);
  const sourceBytes = readFileSync(source);

  if (check) {
    const temp = mkdtempSync(join(tmpdir(), 'aethercore-icons-check-'));
    try {
      const written = await generate(source, temp);
      const drift = written.filter((name) => {
        const committed = join(ICONS, name);
        return !existsSync(committed) || sha256(readFileSync(committed)) !== sha256(readFileSync(join(temp, name)));
      });
      console.log(`checked ${written.length} generated file(s) against ${ICONS}`);
      if (drift.length) {
        for (const name of drift) console.log(`  DRIFT  ${name}`);
        console.log(`\n${drift.length} file(s) differ from what this source produces. Re-run without --check.`);
        process.exitCode = 1;
        return;
      }
      console.log('PASS — every committed icon is what this source renders.');
    } finally {
      rmSync(temp, { recursive: true, force: true });
    }
    return;
  }

  const written = await generate(source, ICONS);
  const relSource = source.startsWith(REPO) ? source.slice(REPO.length + 1) : source;
  writeFileSync(manifestPath, `${JSON.stringify({
    schema: 'aethercore.icon-source.v1',
    // Recorded so the set has one traceable ancestor. The artwork itself is an
    // owner decision (DBT-P36-004); this only says which file made these files.
    source: relSource,
    sourceSha256: sha256(sourceBytes),
    provisional: true,
    generatedBy: 'tools/icon-pipeline/build-icons.mjs',
    files: Object.fromEntries(written.map((n) => [n, sha256(readFileSync(join(ICONS, n)))])),
  }, null, 2)}\n`);
  console.log(`source        ${relSource}  sha256 ${sha256(sourceBytes)}`);
  console.log(`written       ${written.length} file(s) into apps/desktop/icons/`);
  for (const name of written) {
    const bytes = readFileSync(join(ICONS, name));
    console.log(`  ${name.padEnd(24)} ${String(bytes.length).padStart(9)} B  sha256 ${sha256(bytes).slice(0, 16)}…`);
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
