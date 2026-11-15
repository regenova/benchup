# Command line and AI agents

[Documentation](README.md) · [Agent guide](agent-guide.md) · [Developer README](../README.md)

`benchup` reads and edits BenchUp `.bup` projects from a shell or an AI coding agent (Claude Code, Codex, Cursor…), without opening the app. With [`benchup bridge`](#live-editing-in-the-open-app), it also edits the project open in BenchUp, on the desktop and on the web. It reads every project format the app saves, including files with a viewport preview. Lengths are millimetres and Z is up.

## Installation

The command is built from this repository with Rust 1.95 or newer:

```sh
cargo install --locked --path crates/benchup-cli   # installs `benchup` in ~/.cargo/bin
```

Make sure `~/.cargo/bin` is on your `PATH`. The installed command does not update itself: run the same command again after pulling changes. The desktop app is installed separately.

## Commands

```sh
benchup guide                               # conventions, workflow and woodworking recipes
benchup new shelf.bup --name "Shelf"        # create an empty project
benchup tools                               # list the tools
benchup tools create_board                  # one tool's JSON schema
benchup scene shelf.bup                     # scene summary: objects, sizes, grain, stock, joints
benchup scene shelf.bup '{"depth":0,"fields":["name","size_mm"]}'   # a filtered summary (get_scene's arguments)
benchup get shelf.bup 79                    # one entity in detail, with its faces, profiles and finish
benchup render shelf.bup --view front       # a PNG of the model: shelf.front.png (-o OUT.png, --size 1200x900)

benchup call shelf.bup create_board '{"name":"Side","origin_mm":[0,0,0],"size_mm":[18,300,900],"grain_axis":"z"}'
echo '{"ids":[79],"delta_mm":[0,0,10]}' | benchup call shelf.bup move -

benchup call shelf.bup --batch ops.jsonl    # several calls as ONE undo step, all-or-nothing
benchup call shelf.bup --batch ops.jsonl --dry-run --brief   # try it without saving, compact output
benchup call shelf.bup check_model          # overlaps without joints, joints that cannot be cut, ...
benchup call shelf.bup get_cut_list         # boards by material and size
benchup call shelf.bup get_estimate         # finishing, joinery and router time; wood by material
benchup undo shelf.bup
benchup redo shelf.bup

benchup mcp shelf.bup                       # the same tools over MCP (stdio), plus run_batch, undo and render_view

benchup open shelf.bup                      # open it in the BenchUp desktop app
benchup open shelf.bup --app target/debug/benchup-app   # or in a specific executable

benchup bridge                              # MCP (stdio) for the project open in BenchUp
benchup scene --live                        # --live: the same commands on the open project
benchup call --live create_board '{"name":"Side","origin_mm":[0,0,0],"size_mm":[18,300,900]}'
benchup call --live --batch ops.jsonl       # one undo step in BenchUp, all-or-nothing
```

A batch file has one call per line. Blank lines and lines starting with `#` are skipped:

```jsonl
{"tool": "create_board", "arguments": {"name": "Left",  "origin_mm": [0, 0, 0],    "size_mm": [18, 300, 900]}}
{"tool": "create_board", "arguments": {"name": "Right", "origin_mm": [782, 0, 0],  "size_mm": [18, 300, 900]}}
{"tool": "create_board", "arguments": {"name": "Shelf", "origin_mm": [18, 0, 400], "size_mm": [764, 300, 18]}}
```

In a batch, `"$N.field"` is a value from the result of call N (counting from 1): `"$1.created[0]"`, `"$3.joint_id"`, `"$2.face_id"`. A reference to a list of ids inside a list is spliced in, so `"ids": ["$1.created", "$2.created"]` joins both; other lists, such as a point, stay one item. Text that is not exactly a reference, such as `"$12.50 hardware"`, is kept as written, and a single call never resolves references:

```jsonl
{"tool": "create_board", "arguments": {"name": "Side",  "origin_mm": [0, 0, 0],   "size_mm": [18, 300, 900]}}
{"tool": "create_board", "arguments": {"name": "Shelf", "origin_mm": [9, 0, 400], "size_mm": [500, 300, 18]}}
{"tool": "add_joint", "arguments": {"a": "$2.created[0]", "b": "$1.created[0]", "kind": "tongue_groove"}}
```

## Behaviour

- **Output:** JSON on stdout. Exit status is `0` on success, `1` when a tool reported an error (`{"ok": false, "error", "hint"}`, file unchanged), and `2` for a usage or I/O error (message on stderr).
- **MCP:** each tool call is applied and saved on its own. `run_batch {calls, dry_run}` runs several calls as one undo step with `"$N.field"` references, like `--batch` (with `dry_run: true`, like `--dry-run`); `undo` (`redo: true`) steps through the project history; `render_view` returns an image.
- **Previews:** `benchup render` draws the model from a standard view (`iso`, `front`, `back`, `left`, `right`, `top`, `bottom`) with joints and router profiles cut, edges, and dimensions with their values, without a display or GPU. Its JSON output lists dimensions whose labels overlap and dimensions that point along the view. Over MCP, the `render_view` tool returns the same image.
- **History:** each `call` is one entry in the project history, labelled “Agent edit” (in the app's language); a batch is one entry. You can undo it in BenchUp or with `benchup undo`.
- **Atomic saves:** the file is written through a temporary file and a rename. If the file changed on disk since it was read (for example, BenchUp saved it), the command refuses to overwrite it. `benchup mcp` reads the file again before each call, so it carries on from what BenchUp saved.
- **Views and cameras:** the CLI keeps the camera of each view saved in the file, so editing a project does not move the user's views. It cannot list or change saved views yet, and `benchup render` draws the standard views (iso, front, back, left, right, top, bottom), not saved cameras; use BenchUp's [views](controls.md#views-and-cameras) and Tools → Screenshot for those.
- **Shared projects:** local copies of shared projects are refused unless you pass `--allow-shared`, because edits would diverge from the server.
- **Opening in the app:** `benchup open` hands the file to the installed desktop app (by bundle id on macOS, through the `.bup` file association on Linux and Windows). On macOS, when BenchUp is not running yet, it starts the app with the file as a launch argument. `--app <executable>`, or the `BENCHUP_APP` environment variable, starts that executable instead, which is handy for a development build. The app must be installed separately: [download it](https://github.com/BenchUp3D/editor/releases/latest).
- **Files and reloading:** without `--live`, the CLI edits the file, not a running BenchUp window. BenchUp versions after 0.7.1 that have the project open reload it about a second after the CLI saves, keeping the camera. If the project has unsaved changes in the app, BenchUp asks whether to keep them or reload from disk; keeping them and saving overwrites the CLI's edits. With BenchUp 0.7.1 or older, run `benchup open` again after a change to reload it, and save or discard your app edits before editing with the CLI. To edit the open project directly, use [live editing](#live-editing-in-the-open-app).

## Live editing in the open app

`benchup bridge` lets an agent edit the project open in BenchUp, on the desktop or in the browser, while you watch. There is no server: the bridge runs on your computer and listens on `127.0.0.1` only, and BenchUp connects out to it (a browser tab cannot listen on a port).

```
agent ──MCP (stdio) or benchup … --live──▶ benchup bridge (127.0.0.1:47615) ◀──WebSocket── BenchUp
```

1. In BenchUp, open **Settings → Agent connection** and turn on **Allow agent connection**. It is off by default; the setting is remembered.
2. Copy the pairing token shown there (or one of the setup snippets) and give it to the agent as `BENCHUP_AGENT_TOKEN`. The token stays on that device; **New token** replaces it and disconnects agents that use the old one.
3. Connect the agent:

   ```sh
   # MCP clients such as Claude Code
   claude mcp add benchup -e BENCHUP_AGENT_TOKEN=<token> -- benchup bridge

   # agents that run commands
   export BENCHUP_AGENT_TOKEN=<token>
   benchup scene --live
   ```

   `scene`, `get`, `call` (with `--batch` and `--dry-run`), `undo`, `redo` and `render` take `--live` instead of a file. When no bridge is running, the first `--live` command starts one in the background, which exits after 30 minutes without calls. A `benchup bridge` that finds another bridge on its port forwards MCP calls through it, and listens on the port itself once that bridge exits. `BENCHUP_BRIDGE_PORT`, or `benchup bridge --port`, changes the port; set the same port in BenchUp.

How it behaves:

- **Undo:** each changing call is one step in BenchUp's history, labelled `Agent: <tool>`; a batch (`--batch` or MCP `run_batch`) is one step. The designer can undo it as usual, and `benchup undo --live` steps through the same history.
- **One BenchUp at a time:** the bridge serves the first BenchUp window or tab that connects. Another one with the same token is refused (its Agent connection shows the reason and keeps retrying) until the first closes or turns the connection off.
- **Who is in control:** reading calls always run, even while changes queued before them wait, and BenchUp answers calls while its window is minimized or its tab is in the background. A changing call, or one that changes the selection, runs at once unless the designer is in the middle of an operation (drawing, pushing, typing a value…) or has taken control. Then BenchUp asks them to **Grant access**, which cancels their operation and lets the agent continue, or **Deny access**, which refuses the waiting changes with an error the agent can read. While the agent is editing, the status bar shows **Take control**: after that, its changes wait for a grant until the designer chooses **Let the agent edit**.
- **Shared projects:** the agent is treated like the designer. View-only projects stay read-only, edits need the same edit lease, and its steps are attributed to the designer. An edit made while the server is still confirming a new selection waits for the confirmation instead of failing.
- **Scope and selection:** every call starts in the scope the designer is in, so put `enter_object` and the calls that need it in one batch. The agent sees the designer's selection (`get_selection`), so the designer can say "change this".
- **Project switches:** every result names the open project. A call that was waiting when another project was opened is refused, so the agent can look again with `get_scene`. A collaborator's new revision of the same shared project does not refuse waiting calls.
- **Timeouts and size:** a call waits up to 15 seconds for BenchUp to connect and up to 120 seconds for its answer, then fails with a hint. A call larger than 16 MB is refused by the bridge; split it into smaller batches. An answer larger than 32 MB comes back as an error (with `applied: true` when the call still changed the model); filter `get_scene` or render a smaller view.
- **Security:** only programs that know the token can use the bridge, and the token is never sent: both sides prove they know it with a challenge-response (HMAC-SHA256 over fresh nonces). Browsers must come from `https://benchup3d.github.io` or a local development server (`--allow-origin` adds another), so other web pages cannot reach it. Pass the token in the environment, never as a command-line argument, which other users on the computer can see.
- **Web browsers:** Chrome and Safari may ask whether the site may connect to devices on your local network the first time BenchUp connects to the bridge; allow it.

## Using it from an agent

Agents should read `benchup guide` first (over MCP, the `get_guide` tool); it is the [agent guide](agent-guide.md). It explains the conventions (the front faces -Y), which board is `a` in a joint, the batch workflow, woodworking recipes and current limitations.

To make Claude Code reach for the CLI on its own, install the skill:

```sh
cp -r crates/benchup-cli/skill/benchup ~/.claude/skills/
```

With Claude Code, either let it call the CLI directly ("use `benchup` to add a shelf to shelf.bup"), or register the MCP server:

```sh
claude mcp add benchup -- benchup mcp /path/to/shelf.bup
```

To work on the project open in BenchUp instead, register the bridge with the token from Settings → Agent connection (see [Live editing](#live-editing-in-the-open-app)):

```sh
claude mcp add benchup -e BENCHUP_AGENT_TOKEN=<token> -- benchup bridge
```
