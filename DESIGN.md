# DealFlow design direction

Written by the agent from the owner's brief ("calm and precise, a tool people work in all day").
Agent-authored direction tends toward default taste, so every choice below carries its reason.

Dial: ENERGY 1 / RHYTHM 2 / MOTION 1

Reading this as: a signed-in sales workspace for small B2B sales teams, in a quiet
ledger-like visual language, dial ENERGY 1 / RHYTHM 2 / MOTION 1.

## Subject

Sales reps and their managers. The two decisions the product supports every day:
which deal to push next, and which follow-up is about to slip. Money and dates are the
primary content, so numbers are set with the same care as text.

## Palette

| Token | Light | Dark | Reason |
|---|---|---|---|
| canvas | `#F3F4F1` | `#151816` | Grey with a faint green cast, like ruled ledger paper, so white surfaces read as sheets on a desk. |
| surface | `#FFFFFF` | `#1D211F` | Where data lives. |
| ink | `#1D2320` | `#E6EAE7` | Body text. Green-leaning to match the accent instead of a stock near-black. |
| ink-muted | `#5A635E` | `#A3ADA7` | Secondary text; 6:1 on surface in light, 7:1 in dark. |
| line | `#DCE0DB` | `#323834` | Hairlines between rows. |
| evergreen (accent) | `#17654C` | `#5FBF98` | The one accent: primary actions, the current nav item, won deals. Money closing is the product's purpose, so the accent means "moving toward won". |
| brick (critical) | `#A63D2F` | `#E8806F` | Lost deals, overdue tasks, destructive actions. Reserved for "needs attention now". |

Two colors beyond neutrals. Charts use the evergreen ramp only.

## Type

IBM Plex Sans for everything, with tabular figures on every amount and date. Plex was drawn
for business software: compact, mechanical numerals that line up in columns, and a lower
x-height than the usual UI defaults, which keeps dense tables calm. One family, weights 400,
500 and 600.

Scale (1.2 ratio, 15px base): 12.5 / 15 / 18 / 21.6 / 26. Sentence case everywhere.
No uppercase labels.

## Layout

- Horizontal top navigation, not a sidebar: the pipeline board needs the full width.
- The default screen is **Today** (overdue and due follow-ups, deals expected to close this
  month), not a stats dashboard. Reports live on their own page for managers.
- Lists are tables with hairline rows, not cards. Cards appear only on the pipeline board,
  where a deal is a physical thing you drag.
- Working surfaces (lists, follow-ups, notes) sit on white sheets. Reference sections
  (details, stage path, history, people) sit directly on the canvas under a top rule, so the
  page does not turn into a grid of identical boxes.
- Money leads: stage columns show their total value first and the deal count second.
- Left-aligned text, right-aligned amounts.

## Identity motif

The **stage track**: a row of short segments, one per open stage, filled up to where the
deal currently is. It appears in every deal list outside the board (Today, company and
contact pages) and, as the stage rail, across the deal page: full width, named
segments that move the deal when clicked. The board leaves it out because the
column already says the stage. It is information (how far along this deal is), repeated
so the product has one recognizable gesture.

## Shape and depth

Radius 6px on controls and cards, 10px on dialogs. Shadow only on things that float above
the page: a card being dragged and open dialogs. Everything else is flat with hairlines.

## Motion

Hover and focus states, the drag preview, and dialog open. Nothing moves on its own.
`prefers-reduced-motion` turns transitions off.
