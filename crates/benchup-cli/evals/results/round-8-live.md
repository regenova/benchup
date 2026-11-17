# Round 8, live (17 November 2026)

A short round to confirm the fixes made after [round 7](round-7-live.md), on the seven cases those findings came from. As in round 7, each agent edited the project open in a desktop build of BenchUp through `benchup … --live`, with the prompts from `setup.py --live`. Each cell is the number of `benchup` commands, with errors in brackets; results were checked independently with `check_model` and an iso render taken from the app.

| # | Case | R7 desktop | R8 desktop | Result in R8 |
|---|---|---|---|---|
| 01 | Lengthen the desk | 29 (0) | 28 (0) | ✅ followed the new notch recipe (push, split the side face, push, merge) without dry-run exploration |
| 04 | Chest of drawers | 27 (1) | 24 (0) | ✅ wooden runners from the recipe, butted against the sides; no `agent_activity` error |
| 05 | Back in a rabbet and plinth | 21 (0) | 26 (0) | ✅ went straight to the routed rabbet; 5 of its commands checked the profile's orientation |
| 07 | Bedside table with curved sides | 22 (1) | 21 (0) | ✅ no batch broken by `agent_activity` |
| 08 | Leaning ladder shelf | 34 (0) | **24 (0)** | ✅ dimensions placed once, instead of three times |
| 10 | Modules, components and tags | 27 (1) | 26 (1) | ⚠️ partial on purpose, as in every round; tagged before grouping and left the divider outside both modules, as the guide now says. The error was a deliberate trial |
| 12 | A shelf that does not fit | 15 (0) | 18 (1) | ✅ `check_model` now names both boards the shelf runs through, the divider included. The error was a deliberate call to see `measure_space`'s fields |
| | **Total** | **175 (3)** | **167 (2)** | 6 ✅, 1 ⚠️ |

No agent hit `unknown tool agent_activity`, and none of the remaining errors came from the tools.

## Fixed after round 7

- `benchup tools` lists `agent_activity`, marked `live_only`, and `benchup tools agent_activity` shows its schema. Inside a batch, BenchUp skips it with a note instead of failing the batch; in file mode it fails with a message that says it is live-only.
- `check_model` reports a board that runs right through another even when a housing joint (`tongue_groove`, `rabbet`, `groove`) joins them. Through tenons and cross halvings are not affected.
- `render` marks a text note that runs off the image with `cut_off`, and lists them in `texts_cut_off` with a hint.
- `add_dimension` documents that, with `axis`, the line goes through the midpoint of its points.
- The guide covers: `agent_activity` as a call of its own, dry-run ids, the notch case of lengthening, the routed rabbet first in the joint table, drawer runners and the front height of N equal drawers, a plinth without a bottom panel, and organising a model into modules.

## Still open

- `add_profile` and `get_object` do not say which face got `width_mm` and which `depth_mm` (05, since round 6). Answering it needs the router geometry from the core.
- `flip_joint` and other tools that take `ids` also accept `"id"`, on purpose, but the schema does not show it; only some tools accept it, so the schema cannot say so in one place.
- `check_model only_new` reports existing issues as new when boards are recreated (10).
- A way to stop a housing short of the front edge, for tops and bottoms housed flush with the top of the sides (04).
- `benchup guide` is now about 31 KB; agents read it from a saved file. A table of contents or `benchup guide <section>` would help.
- The cut list gives a rotated or shaped board's blank as its bounding box (08).
