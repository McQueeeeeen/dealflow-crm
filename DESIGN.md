# DealFlow design direction

Built from the owner's reference: a dark, quiet task dashboard with a sidebar, rounded row
"pills" instead of table lines, small status pills, tick-mark progress and one violet action
colour.

## Principles

- Dark by default. Light theme is one click away and remembered.
- Neutral charcoal carries the structure. Colour is spent on meaning only: violet for the
  primary action, a temperature scale for pipeline stages, green for won, red for overdue.
- Rows, not rules. Lists and tables are stacks of rounded rows on a slightly lighter tone, so
  each record reads as one object.
- Real numbers only. Stat cards show live figures from the API, never invented deltas.

## Tokens

| Token | Dark | Light | Use |
|---|---|---|---|
| canvas | `#161616` | `#f4f4f5` | Page |
| sidebar | `#121212` | `#fafafa` | Navigation rail |
| surface | `#1d1d1d` | `#ffffff` | Panels, columns, stat cards |
| row | `#252525` | `#f4f4f5` | List rows, table rows, deal cards |
| line | `#2b2b2b` | `#e4e4e7` | Borders |
| ink / muted | `#ededed` / `#959595` | `#18181b` / `#63636b` | Text |
| accent | `#6a55f0` | `#5b45e8` | Primary button, active state |
| link | `#a395ff` | `#5b45e8` | Text links (AA on every surface) |
| won / open | `#4ade80` / `#7ab0ff` | `#15803d` / `#1d4ed8` | Status pills |

Surfaces carry a 1px top highlight (`shadow-highlight`) instead of drop shadows. Only floating
layers (menus, dialogs, a dragged card) get a real shadow.

## Stage temperature

Open stages take a colour from cold blue to hot orange by their position, interpolated in OKLCH
(`lib/heat.ts`), so stages an admin adds later get a colour automatically. Won uses the won
green, lost is neutral.

The stage track is a strip of tick marks, three per open stage, filled up to the current stage.
The same ticks animate once on the sign-in page; that is the only unprompted motion.

## Type

Inter, one family. Headings 600 with slight negative tracking, body 14px, tabular figures for
every amount and date.

## Layout

```
+-----------+-------------------------------------------+
| DealFlow  | [icon] Section                            |
| Org name  +-------------------------------------------+
|-----------| Welcome back, Dana          [Add Follow-Up]|
| Today     | 3 follow-ups due today, 1 overdue.         |
| Pipeline  | [ stat ] [ stat ] [ stat ]                 |
| Companies | ( row                                $ )   |
| Contacts  | ( row                                $ )   |
| Reports   |                                            |
|-----------|                                            |
| Theme     |                                            |
| Account   |                                            |
+-----------+-------------------------------------------+
```

Below `lg` the sidebar becomes a top bar with a scrolling tab row.

## Motion

Page changes use view transitions: sections cross-fade, opening a record slides forward, the
deal title morphs from list to page. The sidebar is pinned out of the transition. Everything
respects `prefers-reduced-motion`.
