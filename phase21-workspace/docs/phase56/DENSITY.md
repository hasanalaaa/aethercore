# P56 — density, measured

"Everything has far too much text" is a judgement. This file is the number
behind it, so the next pass can move it rather than argue about it.

Instrument: `apps/ui/tools/measure-density.mjs`. It reads the **rendered DOM**,
not the source — a `pk()` pair in a catalog is not evidence that a string
reached a screen. Twelve screens × two languages × two data states, at 1280px,
against the fixture server.

## What each column counts

| column | definition |
|---|---|
| **prose** | words inside visible leaf runs of **≥ 5 words** that are not technical. Five is the threshold because every label this product has — "Action items", "Core service log", "CPU headroom" — is four words or fewer, and every subtitle and body paragraph is longer. It is the measurement's one judgement call and it is stated so it can be argued with |
| **runs** | how many such prose runs. Words ÷ runs is the average sentence length |
| **data** | readings: a run in the mono face or inside `.technical-isolate`/`code`/`kbd`, an em dash at rest, or a short run carrying a digit |
| **labels** | short non-technical runs that name a thing |
| **sizes** | distinct computed `font-size` values across every visible run on the screen |
| **elev** | distinct (background-color, box-shadow) pairs across elements that paint a surface of their own. Two cards with one fill and one shadow are **one** elevation; a card, an inset well and a chip are three |
| **w/reading** | prose words per data reading. The complaint as one number: how many words of explanation the screen spends per measurement it shows |

## BEFORE — the tree at `24d6961`, populated

Raw: `measure/before-populated.json`.

| screen | prose | runs | data | labels | sizes | elev | w/reading |
|---|---:|---:|---:|---:|---:|---:|---:|
| overview | 63 | 7 | 90 | 34 | **15** | **20** | 0.70 |
| deepScan | **230** | 24 | 14 | 51 | **17** | 9 | 16.43 |
| drivers | 103 | 14 | 62 | 74 | 14 | 15 | 1.66 |
| repair | 166 | 13 | 9 | 52 | 14 | 12 | **18.44** |
| cleanup | 199 | 17 | 20 | 25 | 12 | 11 | 9.95 |
| startup | 145 | 12 | 22 | 66 | 12 | 10 | 6.59 |
| performance | 89 | 6 | 13 | 21 | 13 | 6 | 6.85 |
| hardware | **219** | 16 | 75 | 75 | 14 | 9 | 2.92 |
| crash | 169 | 14 | 19 | 36 | 13 | 8 | 8.89 |
| activity | 142 | 12 | 16 | 30 | 14 | 12 | 8.88 |
| fleet | 6 | 1 | 4 | 9 | 7 | 5 | 1.50 |
| settings | 65 | 4 | 21 | 39 | **15** | 10 | 3.10 |
| **TOTAL (en)** | **1,596** | 140 | 365 | — | — | — | **4.37** |
| **TOTAL (ar)** | **1,309** | 115 | 402 | — | — | — | **3.26** |

Shell (the sidebar, repeated on every screen): **47 prose words**, 8 prose runs,
29 labels, 9 type sizes, 8 elevations.

## BEFORE — the same tree, empty

Raw: `measure/before-empty.json`. This is the state the owner meets first, and
it is where the product explains itself hardest.

| screen | prose | runs | data | sizes | elev | w/reading |
|---|---:|---:|---:|---:|---:|---:|
| overview | **167** | 9 | 32 | 16 | 12 | 5.22 |
| deepScan | 55 | 5 | 4 | 11 | 8 | 13.75 |
| drivers | 91 | 9 | 13 | **17** | 10 | 7.00 |
| repair | 110 | 6 | 4 | 14 | 9 | **27.50** |
| cleanup | 104 | 8 | 10 | 15 | 9 | 10.40 |
| startup | 107 | 7 | 9 | 15 | 9 | 11.89 |
| performance | 75 | 5 | 16 | 15 | 7 | 4.69 |
| hardware | 100 | 6 | 9 | 15 | 9 | 11.11 |
| crash | 143 | 10 | 9 | 16 | 9 | 15.89 |
| activity | **184** | 15 | 7 | 13 | 11 | **26.29** |
| fleet | 36 | 4 | 5 | 10 | 7 | 7.20 |
| settings | 107 | 7 | 7 | **17** | 10 | 15.29 |
| **TOTAL (en)** | **1,279** | 91 | 125 | — | — | **10.23** |
| **TOTAL (ar)** | **1,144** | 79 | 124 | — | — | **9.23** |

## What the numbers actually say

Three findings, and only one of them is the one the owner named.

**1. The Overview is not the wordiest screen — it is the least differentiated.**
Populated it carries 63 prose words, the second-lowest of twelve, because P51
already cut it. But it carries **15 distinct type sizes and 20 distinct surface
elevations** — the most of any screen in the product. That is the defect behind
"too much text": not volume, *undifferentiated* volume. The brief predicted this
— "density is correct; *undifferentiated* density is the defect" — and the
instrument confirms it against the one screen the owner judges the product on.

Splitting those 20 surfaces by whether they are neutral (a structural layer) or
role-tinted (a chip carrying a semantic colour) separates the two problems:

| screen | surfaces | neutral | role-tinted |
|---|---:|---:|---:|
| overview | 20 | **6** | 14 |
| drivers | 15 | 6 | 9 |
| repair | 12 | 4 | 8 |
| activity | 12 | 4 | 8 |
| settings | 10 | 4 | 6 |
| cleanup / hardware / deepScan | 9–11 | 3 | 6–8 |
| startup | 10 | 3 | 7 |
| crash | 8 | 2 | 6 |
| performance | 6 | 2 | 4 |
| fleet | 5 | 1 | 4 |

Role-tinted fills are semantics and are supposed to be plural — a critical chip
and a healthy chip must differ. **Neutral levels are structure, and
`apps/ui/DESIGN_LANGUAGE.md` already states the rule: "One elevation step per
interaction layer."** The Overview renders six. That is not a new opinion; it is
an existing rule being broken on the product's front screen.

The fifteen type sizes are the same story. `design-tokens.css` defines a scale of
**seven** sizes. Rendered, the Overview shows 9.5, 10, 10.56, 11, 11.5, 12.5, 13,
13.12, 13.5, 14, 15, 17, 27, 34, 44 px. Eight of the fifteen are literals written
outside the scale, and the clusters — 9.5/10/10.56, 12.5/13/13.12/13.5/14 — carry
no distinction a reader could name.

**2. The explaining happens when there is nothing to show.** Empty, the Overview
goes from 63 prose words to **167** — it more than doubles — and the whole
product goes from 4.37 words per reading to 10.23. Four of the five longest runs
on the empty Overview are UI narration: what this screen does, where action items
come from, what a log line is, which screen to start from. An honest empty state
owes the reader one fact — *not collected yet* — and one route. It does not owe
them a tour.

**3. The sidebar spends 47 words on twelve nouns.** Every entry carries a
description under its label, on every screen, permanently. "System posture at a
glance" explains "Overview" to someone who is already looking at it.

Worst offenders by volume are `deepScan` (230), `hardware` (219) and `cleanup`
(199) populated; by ratio, `repair` (18.44 words per reading) and, empty,
`activity` (26.29) and `repair` (27.50). Those are Part 3's targets. The
Overview is Part 1's, and its target is hierarchy.

## AFTER

See [DENSITY-AFTER.md](DENSITY-AFTER.md) — written after the Overview rebuild,
against the same instrument at the same width.
