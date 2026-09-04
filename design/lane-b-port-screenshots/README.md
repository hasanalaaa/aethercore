# Lane B — the Svelte port, delivery screenshots

Produced by `phase21-workspace/apps/ui/tools/layout-sweep.mjs` in headless
Chrome at device scale 1. Regenerate with:

    cd phase21-workspace/apps/ui
    npm run fixture          # in one terminal
    npm run sweep -- --pages overview,deepScan,drivers --locales en,ar --widths 1280,1024,960

## `populated-*` — the app with real data

Driven through `layout-fixture.html`, which installs the sanctioned test
transport and replays a representative kernel stream: connected session, 148
devices, 12,480 journal events, a plan awaiting authorization, deep-scan
findings with evidence, an insight with citations, a refused driver candidate.

Populated screenshots are the point. Four responsive screenshots once passed
only because the app had no data and nothing could overflow.

## `no-service-*` — the app with nothing attached

Opened on `index.html` with no maintenance service present. This is what a new
user sees first, and it is where the honest empty state has to hold: "Not
collected yet", `—` for a reading never taken, and no fake zero anywhere.
`no-service-performance-1280-en-dark.png` shows the four meters at rest.

## `sweep-*.json`

The raw per-configuration measurements behind both sets — overflowX, clipped
element count, control/prose overlap count, direction, and the counts of denied
elements, evidence chips and em dashes. 66/66 pass in each.
