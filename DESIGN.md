# DealFlow design direction

Written by the agent from the owner's brief ("calm and precise, a tool people work in all day").
Agent-authored direction tends toward default taste, so every choice below carries its reason.

Dial: ENERGY 2 / RHYTHM 2 / MOTION 1

Reading this as: a signed-in sales workspace for small B2B sales teams, in a soft, colour-coded
product language where colour means one thing only: how hot a deal is.

## Subject

Sales people talk about cold leads and hot deals. The design takes that literally. The pipeline
is a temperature scale, and colour is reserved for it, so a glance at any screen says how close
the money is.

## Thermal scale

Each open stage gets a colour from its position in the pipeline, interpolated in OKLCH so steps
look even. Stages an admin adds later get a colour automatically.

| Point | Colour | Meaning |
|---|---|---|
| cold | `oklch(0.62 0.16 255)` blue | First stage, a new lead |
| cool | `oklch(0.70 0.12 200)` teal | Early qualification |
| warm | `oklch(0.80 0.15 85)` amber | Proposal territory |
| hot | `oklch(0.70 0.18 50)` orange | Last open stage |
| won | `oklch(0.63 0.21 32)` flame | Closed won |
| lost | neutral grey | The deal went cold |

The scale appears wherever stage matters: the bar and wash on each board column, stage tracks
in lists, the stage rail on a deal, funnel bars, won amounts and the rail on the sign-in page.
It never decorates anything else.

## Neutrals and actions

| Token | Light | Dark | Reason |
|---|---|---|---|
| canvas | `#EEF1F5` | `#12151C` | Cool grey so the warm end of the scale stands out. |
| surface | `#FDFDFE` | `#191D26` | Cards and panels. |
| ink | `#1B2130` | `#E8EBF1` | Text, and primary actions: buttons and the current page are ink pills, so colour stays free for temperature. |
| ink-muted | `#5A6275` | `#A1A9B8` | Secondary text; at least 5:1 on canvas. |
| won | `#B8461B` | `#FF9B66` | Won amounts as text; 5.3:1 on surface. |
| focus | `#2F6FE4` | `#7AA7FF` | Keyboard focus ring only. |
| critical | `#C0392B` | `#FF7B6B` | Overdue follow-ups and destructive actions. |

## Type

Manrope for everything: soft, open shapes that match the rounded components, with tabular
figures for columns of money. Titles and large amounts use weight 800 with tight tracking;
body text 400 and 500.

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

The thermal bar: a short rounded bar in a stage's temperature colour. It caps every board
column, fills the stage tracks and the deal's stage rail, and heats up from Lead to Won once on
the sign-in page.

## Shape and depth

Radius 10px on controls, 14px on cards and panels, 18px on dialogs; navigation and status marks
are pills. Deal cards on the board carry a soft shadow because they are physical objects you
drag; a dragged card and dialogs lift higher. Everything else is flat with hairlines.

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
