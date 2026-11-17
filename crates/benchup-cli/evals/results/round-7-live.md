# Round 7, live (16 November 2026)

This round is item 3 of [#55](https://github.com/BenchUp3D/benchup/issues/55): the twelve original cases (01–12) rerun against the project open in BenchUp instead of a file, through `benchup … --live` and `benchup bridge`. Each agent got the usual prompt, prepared with `setup.py --live`, so it learned the tools only from `benchup --help`, the guide and the tool output, and was told the project was open in the app. The requests were not changed.

The cases ran one at a time on each surface, because one BenchUp serves one bridge:

- **Desktop:** a `dev-desktop` build of `main` on macOS, started for each case with its own `BENCHUP_DATA_DIR`, the agent connection turned on and the case's file passed on the command line.
- **Web, Chrome 157:** the same commit served by `trunk serve` on `http://127.0.0.1:8090`, with its own bridge port (47616) and token. Each case's file was opened through File → Open.
- **Web, Safari:** not run yet (see below).

Each cell is the number of `benchup` commands, with those that returned an error in brackets. Live counts include two to four `agent_activity` calls per case, which file mode does not have. Every result was checked independently afterwards with `check_model` and an iso render taken live from the app.

| # | Case | R5 (file) | R7 desktop | R7 Chrome | Result in R7 (both surfaces) |
|---|---|---|---|---|---|
| 01 | Lengthen the desk | 23 (1) | 29 (0) | 32 (0) | ✅ lengthened at the front, top kept full width past the post |
| 02 | Five shelves to four | 17 (0) | 21 (0) | 20 (0) | ✅ gaps of 345.6 mm, joints flipped in a separate batch |
| 03 | Dining table | 18 (0) | 20 (1) | 21 (0) | ✅ 8/8 joints |
| 04 | Chest of drawers | 20 (0) | 27 (1) | 24 (0) | ✅ 24/24 joints, one component |
| 05 | Back in a rabbet and plinth | 43 (2) | 21 (0) | 20 (0) | ✅ routed rabbet, back whole, plinth in two halves |
| 06 | Solid oak 19 mm | 25 (3) | 24 (0) | 20 (0) | ✅ `change_thickness` without workarounds |
| 07 | Bedside table with curved sides | 16 (0) | 22 (1) | 22 (1) | ✅ 6/6 joints, 2 profiles |
| 08 | Leaning ladder shelf | 27 (1) | 34 (0) | 26 (0) | ✅ 8/8 stopped housings |
| 09 | Workshop dimensions | 18 (0) | 13 (0) | 16 (0) | ✅ the ten reversed joints of the seed are reported, not touched |
| 10 | Modules, components and tags | 25 (2) | 27 (1) | 27 (0) | ⚠️ partial on purpose, alternative saved, as in every round |
| 11 | Cut list and sheets | 12 (0) | 13 (0) | 15 (0) | ✅ 2 sheets, dry run only, project unchanged |
| 12 | A shelf that does not fit | 14 (0) | 15 (0) | 12 (0) | ✅ refused, 491 mm alternative saved |
| | **Total** | **258 (9)** | **266 (4)** | **255 (1)** | 11 ✅, 1 ⚠️ on each surface |

Live mode behaved like file mode. No agent hit a bridge or connection error, and no call timed out. The extra commands are the `agent_activity` reports and, on the desktop, one agent (08) that placed its dimensions three times. The error in 10 on the desktop was a deliberate trial.

## What was checked about live mode

- **Undo:** a batch is one step in BenchUp, for example `Agent: push_pull, split_face, merge_faces, …`, and `benchup undo --live` / `redo --live` step through it.
- **Dry runs:** `call --live --dry-run` leaves the open model and its undo history untouched (checked by counting objects before and after, and by undoing the previous step). Agents in 02, 04 and 11 relied on this.
- **Reconnection:** in Chrome, `trunk` reloaded the page right after case 02. BenchUp restored the project from local storage and connected to the bridge again by itself; the change was kept.
- **Opening another project:** on the web, File → Open while the agent's changes were unsaved showed the unsaved-changes modal (Discard changes, Save, Cancel), as for any other change.
- **Status:** the web status bar showed "Agente conectado" (the app was in Spanish) while the bridge served it.

Not covered here: the grant/deny prompt and Take control (they need someone drawing in the app at the time), shared projects and leases (they need the server).

## Transport from the published web app

- **Origin:** `https://benchup3d.github.io/benchup/` now redirects to `https://benchup.io`, so the published app loads from that origin. The bridge only allowed `https://benchup3d.github.io` and answered the published app with HTTP 403. Fixed in this branch: the bridge allows both origins (`bridge.rs`, with a test).
- **Chrome 157:** a WebSocket from `https://benchup.io` to `ws://127.0.0.1` waits for the local network access permission (`local-network-access` is `prompt`). Nobody answered the prompt in this run; after about 240 s the socket failed with code 1006. Still to do: allow the prompt and confirm the connection opens, then check that BenchUp says what is happening while the prompt waits.
- **Safari:** still to do. The question from #73 is whether Safari lets an `https` page open `ws://127.0.0.1`. The local build on `http://127.0.0.1` does not answer it.

## Findings

### Tool bugs and unclear output (benchup-tools / CLI)

1. **`agent_activity` is missing from `benchup tools`**, and `benchup tools agent_activity` fails with "unknown tool agent_activity", although the guide tells agents to call it in live mode (03, 04, 07 and others).
2. **`agent_activity` inside a `--batch` fails** with "unknown tool agent_activity" and aborts the batch; the guide's "use `{"phase":"preparing"}` while preparing changes" made an agent put it there, and it then had to renumber every `$N` (07, desktop).
3. **`add_joint` accepts a `tongue_groove` whose `a` runs right through `b`**, and `check_model` then names only one of the two boards the shelf passes through (12, desktop).
4. **`flip_joint` accepts `{"id": …}`** although its schema only has `ids` and `additionalProperties: false`, without a warning (10, Chrome).
5. **`check_model only_new`** still reports existing issues as new when the boards are recreated (10, desktop; the `group` case from round 5 is fixed).
6. **`push_pull` on a face that borders a notch** stretches the neighbouring faces and returns `created: []`; it also returns both `face_id` and `new_face_id` with the same value (01).
7. **`change_thickness`** reports an intermediate `length_mm` (481.5) in `joint_adjustments` and the final one (481) in `butt_adjustments` (06, Chrome).
8. **`add_dimension` with `axis`** anchors the line at the midpoint of its points when they differ off the axis, so offsets need to be compensated by hand; ends attach only when given to about five decimals on rotated geometry (08).
9. **`render`** clips long text notes at the image edge without saying so (03, 04, 08), and lists edge-on dimensions of other planes as overlaps (09).
10. **`add_profile` and `get_object`** do not say which face received `width_mm` and which `depth_mm` (05, both surfaces, as in round 6).

### Guide gaps (docs/agent-guide.md)

- Say that `agent_activity` is a live-only call that is not listed by `benchup tools` and must not go in a batch, and give its phases.
- Say that dry-run ids match the real run in live mode; the guide says they differ (04).
- The "Lengthening a piece" recipe should cover an end face next to a notch: push, split the new side face at the notch line, push the piece out (01, both surfaces).
- The `rabbet` row of the joint table still reads as a rabbet but cuts a tongue in a slot; lead with the routed profile (05).
- The drawer recipe should say how drawers are supported (runners, rails) and give the gap arithmetic for N equal drawers (04).
- The plinth recipe assumes a bottom panel; the bookcase has none (05).
- Organising an existing model: parts shared between modules, and the order of tag, rename and group calls since grouping changes ids (10).
- `benchup guide` is about 29 KB, and every agent had to read it from a saved file; a table of contents or `benchup guide <section>` would help.

### Live-mode setup

- The pairing token is the only thing an agent needs besides the command, and every agent found the `--live` mode from `benchup --help` alone.
- The project name in results is the file stem (`table`), not a title (`Table`); agents noticed but were not confused.
