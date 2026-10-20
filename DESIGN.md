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

Monochrome with one saturated signal. Grey-green with a forest accent was the expected
fintech look, so the identity now comes from a single cobalt used only where something moves
toward a close.

| Token | Light | Dark | Reason |
|---|---|---|---|
| canvas | `#F4F5F7` | `#14161B` | Cool off-white and graphite, no warm paper tint. |
| surface | `#FCFCFD` | `#1B1E25` | Where data lives. Not pure white, so the cobalt reads crisper. |
| ink | `#15171C` | `#E9EBF0` | Body text. |
| ink-muted | `#5B606B` | `#A2A8B4` | Secondary text; at least 6:1 on surface in both themes. |
| line | `#E1E3E8` | `#2C313B` | Hairlines between rows. |
| cobalt (accent) | `#2440F0` | `#7D8CFF` | The one accent: primary actions, the current page, won deals, the stage track. 6.8:1 with white text. |
| brick (critical) | `#C2341C` | `#FF7A63` | Lost deals, overdue follow-ups, destructive actions only. |

One accent beyond neutrals. Charts use the cobalt ramp only.

## Type

- Archivo, set 12% wider than normal, is the product's voice: page titles, the wordmark and
  money that matters (deal value, stage totals, report figures, the Today summary). The wide
  cut reads like a ticker or a printed statement, engineered rather than decorative.
- IBM Plex Sans does the work: tables, forms, buttons, labels. Compact numerals that line up
  in columns; tabular figures on every amount and date.

Scale (1.2 ratio, 15px base): 12.5 / 15 / 18 / 21.6 / 34, and 48 for the sign-in headline.
Title Case for headings and buttons (Chicago style), sentence case for body copy, labels and
messages. No uppercase labels.

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

Hover and focus states, the drag preview, and dialog open. One orchestrated moment: the stage
rail on the sign-in page fills once, stage by stage, to show what the product does.

Navigation uses view transitions, each with one job:

- A deal's name morphs from the board, Today or a list into the deal page title: the same
  object, seen closer.
- Opening a record slides content left; links back to a list slide it right.
- Switching sections from the main navigation crossfades.
- The header never moves, so there is always a fixed point.

`prefers-reduced-motion` turns transitions off.
