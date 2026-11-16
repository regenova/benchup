//! `benchup`: read and edit BenchUp project files from a shell or an AI agent.
//!
//! Every command prints JSON on stdout. Exit status: 0 success, 1 the tool
//! reported an error (the file is unchanged), 2 usage or I/O error.
mod bridge;
mod launch;
mod mcp;
mod project;

use benchup_tools::{definitions, is_mutating, mcp_tools};
use project::Project;
use serde_json::{Value, json};
use std::io::{Read, stdin, stdout};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
benchup: read and edit BenchUp (.bup) projects. Lengths are millimetres, Z is up.
New here? Run `benchup guide` first: conventions, workflow and woodworking recipes.

usage:
  benchup new <file.bup> [--name NAME] [--force]   create an empty project
  benchup guide                                    how to model furniture with these tools
  benchup tools [NAME] [--json] [--names]          list tools (--names: names only), or show one
                                                   tool's JSON schema
  benchup scene <file.bup> [FILTER]                scene summary (objects, stock, joints); FILTER
                                                   is get_scene's JSON, e.g. '{\"depth\":0}'
  benchup get <file.bup> <id>                      one entity in detail, with its faces
  benchup render <file.bup> [-o OUT.png] [--view VIEW] [--size WxH]
                                                   a PNG of the model (iso, front, back, left,
                                                   right, top, bottom; default iso, 1000x750;
                                                   64 to 2400 pixels a side)
  benchup call <file.bup> <tool> [ARGS|-]          run a tool; ARGS is a JSON object (- reads stdin)
  benchup call <file.bup> --batch <FILE|->         run JSON lines {\"tool\": .., \"arguments\": {..}}
                                                   as one all-or-nothing undo step; \"$N.created[0]\"
                                                   in arguments is a value from call N (from 1)
  benchup undo <file.bup>                          undo the last change
  benchup redo <file.bup>                          redo the last undone change
  benchup mcp <file.bup>                           serve the tools over MCP (stdio)
  benchup bridge [--no-mcp] [--port PORT] [--allow-origin ORIGIN]...
                                                   serve the project open in BenchUp over MCP
                                                   (stdio); BenchUp connects to it on 127.0.0.1
  benchup open <file.bup> [--app EXECUTABLE]       open the project in the BenchUp desktop app
                                                   (or in EXECUTABLE, default $BENCHUP_APP)

live mode: scene, get, call, undo, redo and render take --live instead of <file.bup>
and act on the project open in BenchUp, e.g. `benchup call --live create_board '{..}'`.
Set BENCHUP_AGENT_TOKEN to the pairing token in BenchUp (Settings → Agent connection).
A bridge is started in the background when none is running (BENCHUP_BRIDGE_PORT
changes its port). Each change is one undo step, \"Agent: ...\", in BenchUp.

options:
  --live           act on the project open in BenchUp, through `benchup bridge`
  --dry-run        (call) report the results without changing the file
  --brief          (call) leave out the object summaries of changing calls
  --allow-shared   edit the local copy of a shared project

Every change is one undo step in BenchUp and the file is saved atomically.
Each command starts at the top level of the model: a group opened with
enter_object stays open only until the end of that command, so put enter_object
and the calls that need it in one --batch.
Output is JSON. Exit status: 0 ok, 1 tool error (file unchanged), 2 usage or I/O error.";

/// What a command produced: JSON to print and whether it succeeded.
struct Output {
    value: Value,
    ok: bool,
}

fn ok(value: Value) -> Output {
    Output { value, ok: true }
}

/// Splits `args` into positionals and the flags this command accepts.
struct Args {
    positional: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

fn parse_args(args: Vec<String>, flags: &[&str], valued: &[&str]) -> Result<Args, String> {
    let mut out = Args { positional: Vec::new(), flags: Vec::new(), values: Vec::new() };
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        if flags.contains(&arg.as_str()) {
            out.flags.push(arg);
        } else if valued.contains(&arg.as_str()) {
            let value = it.next().ok_or(format!("{arg} needs a value"))?;
            out.values.push((arg, value));
        } else if arg.starts_with("--") {
            return Err(format!("unknown option {arg}"));
        } else {
            out.positional.push(arg);
        }
    }
    Ok(out)
}

impl Args {
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
    /// Exactly `n` positionals, or a usage error.
    fn expect(&self, n: usize) -> Result<&[String], String> {
        if self.positional.len() == n { Ok(&self.positional) } else { Err(USAGE.into()) }
    }
}

fn read_input(source: &str) -> Result<String, String> {
    if source == "-" {
        let mut text = String::new();
        stdin().read_to_string(&mut text).map_err(|e| format!("cannot read stdin: {e}"))?;
        Ok(text)
    } else {
        std::fs::read_to_string(source).map_err(|e| format!("cannot read {source}: {e}"))
    }
}

fn parse_object(text: &str, what: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(text) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(_) => Err(format!("{what} must be a JSON object")),
        Err(e) => Err(format!("{what} is not valid JSON: {e}")),
    }
}

/// One batch line: `{"tool": "move", "arguments": {...}}` (`name` is accepted for `tool`).
fn parse_batch(text: &str) -> Result<Vec<(String, Value)>, String> {
    let mut calls = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let entry = parse_object(line, &format!("batch line {}", number + 1))?;
        let name = entry
            .get("tool")
            .or_else(|| entry.get("name"))
            .and_then(Value::as_str)
            .ok_or(format!("batch line {}: missing \"tool\"", number + 1))?;
        let arguments = entry.get("arguments").cloned().unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            return Err(format!("batch line {}: \"arguments\" must be an object", number + 1));
        }
        calls.push((name.to_owned(), arguments));
    }
    if calls.is_empty() {
        return Err("the batch has no calls".into());
    }
    Ok(calls)
}

fn apply(project: &mut Project, calls: &[(String, Value)]) -> Result<Output, String> {
    run_calls(project, calls, true, false)
}

fn run_calls(project: &mut Project, calls: &[(String, Value)], save: bool, batch: bool) -> Result<Output, String> {
    let (value, ok) = benchup_tools::batch::report(calls, project.run(calls, save)?, batch);
    Ok(Output { value, ok })
}

/// Pretty JSON with the answer first: `ok`, then the error and hint of a
/// failure, then the rest in name order.
fn pretty(value: &Value) -> String {
    const FIRST: [&str; 6] = ["ok", "error", "hint", "failed_call", "tool", "dry_run"];
    let Value::Object(map) = value else { return serde_json::to_string_pretty(value).unwrap_or_default() };
    let keys = FIRST.iter().copied().filter(|k| map.contains_key(*k)).chain(map.keys().map(String::as_str).filter(|k| !FIRST.contains(k)));
    let fields: Vec<String> = keys
        .map(|key| format!("  {}: {}", json!(key), serde_json::to_string_pretty(&map[key]).unwrap_or_default().replace('\n', "\n  ")))
        .collect();
    if fields.is_empty() { "{}".into() } else { format!("{{\n{}\n}}", fields.join(",\n")) }
}

fn tools(args: Args) -> Result<Output, String> {
    // `agent_activity` is answered by BenchUp itself, not by the tool layer,
    // so it only exists with --live; list it anyway, marked as such, since
    // the guide asks agents to call it.
    let activity = mcp::agent_activity_tool();
    let activity_name = activity["name"].as_str().unwrap_or_default().to_owned();
    match args.positional.as_slice() {
        [] if args.flag("--json") => {
            let mut list = mcp_tools();
            list.push(activity);
            Ok(ok(Value::Array(list)))
        }
        [] if args.flag("--names") => {
            let mut names: Vec<String> = definitions().into_iter().map(|d| d.name).collect();
            names.push(activity_name);
            Ok(ok(json!(names)))
        }
        [] => {
            let mut list: Vec<Value> = definitions()
                .into_iter()
                .map(|d| json!({"name": d.name, "changes_model": is_mutating(&d.name), "description": d.description}))
                .collect();
            list.push(json!({"name": activity_name, "changes_model": false, "live_only": true, "description": activity["description"]}));
            Ok(ok(Value::Array(list)))
        }
        [name] if *name == activity_name => Ok(ok(json!({"name": activity_name, "live_only": true,
            "description": activity["description"], "inputSchema": activity["inputSchema"]}))),
        [name] => definitions()
            .into_iter()
            .find(|d| &d.name == name)
            .map(|d| ok(json!({"name": d.name, "description": d.description, "inputSchema": d.input_schema})))
            .ok_or(format!("unknown tool {name}; run `benchup tools` for the list")),
        _ => Err(USAGE.into()),
    }
}

/// Runs one call in the project open in BenchUp (`--live`).
fn live(name: &str, arguments: Value) -> Result<Output, String> {
    let (value, ok) = bridge::cli_request(name, arguments)?;
    Ok(Output { value, ok })
}

/// The positionals after `<file.bup>`, which `--live` commands leave out.
fn after_file(args: &Args) -> Result<&[String], String> {
    let skip = usize::from(!args.flag("--live"));
    args.positional.get(skip..).ok_or_else(|| USAGE.to_owned())
}

fn run(mut argv: Vec<String>) -> Result<Output, String> {
    if argv.is_empty() || matches!(argv[0].as_str(), "-h" | "--help" | "help") {
        println!("{USAGE}");
        std::process::exit(if argv.is_empty() { 2 } else { 0 });
    }
    let command = argv.remove(0);
    let open = |args: &Args| Project::open(&args.positional[0], args.flag("--allow-shared"));
    match command.as_str() {
        "new" => {
            let args = parse_args(argv, &["--force"], &["--name"])?;
            let path = Path::new(&args.expect(1)?[0]);
            let doc = Project::create(path, args.value("--name"), args.flag("--force"))?;
            Ok(ok(json!({"ok": true, "created": path.display().to_string(), "name": doc.name})))
        }
        "tools" => tools(parse_args(argv, &["--json", "--names"], &[])?),
        "scene" => {
            let args = parse_args(argv, &["--allow-shared", "--live"], &[])?;
            let filter = match after_file(&args)? {
                [] => json!({}),
                [filter] => parse_object(filter, "FILTER")?,
                _ => return Err(USAGE.into()),
            };
            if args.flag("--live") {
                return live("get_scene", filter);
            }
            apply(&mut open(&args)?, &[("get_scene".into(), filter)])
        }
        "render" => {
            let args = parse_args(argv, &["--allow-shared", "--live"], &["-o", "--output", "--view", "--size"])?;
            let view = args.value("--view").unwrap_or("iso");
            let (width, height) = match args.value("--size") {
                None => (1000, 750),
                Some(size) => size
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
                    .ok_or("--size is WIDTHxHEIGHT in pixels, e.g. 1200x900")?,
            };
            let output = args.value("-o").or(args.value("--output")).map(std::path::PathBuf::from);
            let (png, info, output) = if args.flag("--live") {
                args.expect(0)?;
                let answer = live("render_view", json!({"view": view, "width": width, "height": height}))?;
                if !answer.ok {
                    return Ok(answer);
                }
                let png = bridge::decode_base64(answer.value["png_base64"].as_str().unwrap_or_default())?;
                let output = output.unwrap_or_else(|| format!("benchup-live.{view}.png").into());
                (png, answer.value["info"].clone(), output)
            } else {
                let path = &args.expect(1)?[0];
                let project = open(&args)?;
                let rendered = benchup_tools::render::render(&project.doc.model, view, width, height)?;
                let output = output.unwrap_or_else(|| Path::new(path).with_extension(format!("{view}.png")));
                (rendered.png, rendered.info, output)
            };
            std::fs::write(&output, &png).map_err(|e| format!("cannot write {}: {e}", output.display()))?;
            let mut value = if info.is_object() { info } else { json!({}) };
            value["ok"] = json!(true);
            value["file"] = json!(output.display().to_string());
            Ok(ok(value))
        }
        "get" => {
            let args = parse_args(argv, &["--allow-shared", "--live"], &[])?;
            let [id] = after_file(&args)? else { return Err(USAGE.into()) };
            let id: u64 = id.parse().map_err(|_| "the id must be a number".to_owned())?;
            if args.flag("--live") {
                return live("get_object", json!({"id": id}));
            }
            apply(&mut open(&args)?, &[("get_object".into(), json!({"id": id}))])
        }
        "guide" => {
            println!("{}", benchup_tools::GUIDE);
            std::process::exit(0);
        }
        "call" => {
            let args = parse_args(argv, &["--allow-shared", "--dry-run", "--brief", "--live"], &["--batch"])?;
            let calls = match (args.value("--batch"), after_file(&args)?) {
                (Some(source), []) => parse_batch(&read_input(source)?)?,
                (None, [tool]) => vec![(tool.clone(), json!({}))],
                (None, [tool, arguments]) => {
                    let text = if arguments == "-" { read_input("-")? } else { arguments.clone() };
                    vec![(tool.clone(), parse_object(&text, "ARGS")?)]
                }
                _ => return Err(USAGE.into()),
            };
            let batch = args.value("--batch").is_some();
            let dry_run = args.flag("--dry-run");
            let mut output = if !args.flag("--live") {
                run_calls(&mut open(&args)?, &calls, !dry_run, batch)?
            } else if batch || dry_run {
                let list: Vec<Value> = calls.iter().map(|(tool, arguments)| json!({"tool": tool, "arguments": arguments})).collect();
                // `single`: one call takes "$" text as written and answers
                // with its own result, as without --live.
                live("run_batch", json!({"calls": list, "dry_run": dry_run, "single": !batch}))?
            } else {
                let (tool, arguments) = calls.into_iter().next().ok_or(USAGE)?;
                live(&tool, arguments)?
            };
            if dry_run && output.value.is_object() {
                output.value["dry_run"] = json!(true);
            }
            if args.flag("--brief") {
                benchup_tools::batch::brief(&mut output.value);
            }
            Ok(output)
        }
        "undo" | "redo" => {
            let args = parse_args(argv, &["--allow-shared", "--live"], &[])?;
            if args.flag("--live") {
                args.expect(0)?;
                return live("undo", json!({"redo": command == "redo"}));
            }
            args.expect(1)?;
            let value = open(&args)?.step_history(command == "redo")?;
            let ok = value["ok"] == true;
            Ok(Output { value, ok })
        }
        "open" => {
            let args = parse_args(argv, &[], &["--app"])?;
            let path = &args.expect(1)?[0];
            let app = args.value("--app").map(str::to_owned).or_else(|| {
                std::env::var("BENCHUP_APP").ok().filter(|app| !app.is_empty())
            });
            Ok(ok(launch::open(path, app.as_deref())?))
        }
        "bridge" => bridge_command(argv),
        "mcp" => {
            let args = parse_args(argv, &["--allow-shared"], &[])?;
            args.expect(1)?;
            let mut project = open(&args)?;
            eprintln!("benchup: serving {} over MCP on stdio", project.doc.name);
            mcp::serve(stdin().lock(), stdout().lock(), &mut project).map_err(|e| e.to_string())?;
            std::process::exit(0);
        }
        _ => Err(format!("unknown command {command}\n\n{USAGE}")),
    }
}

/// `benchup bridge`: listens for BenchUp on 127.0.0.1 and serves MCP on
/// stdio, or only listens (`--no-mcp`) for `--live` commands.
fn bridge_command(argv: Vec<String>) -> Result<Output, String> {
    let mut no_mcp = false;
    let mut config = bridge::Config::new(bridge::token_from_env()?);
    config.port = bridge::port_from_env()?;
    let mut idle_exit = None;
    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--no-mcp" => no_mcp = true,
            "--port" => config.port = value("--port")?.parse().map_err(|_| "--port must be a port number")?,
            "--allow-origin" => config.extra_origins.push(value("--allow-origin")?),
            "--idle-exit" => {
                let seconds: u64 = value("--idle-exit")?.parse().map_err(|_| "--idle-exit is a number of seconds")?;
                idle_exit = Some(std::time::Duration::from_secs(seconds.max(1)));
            }
            _ => return Err(format!("unknown option {arg}\n\n{USAGE}")),
        }
    }
    let target = match bridge::Hub::start(config.clone()) {
        Ok(hub) => {
            eprintln!("benchup bridge: waiting for BenchUp on ws://127.0.0.1:{}", hub.port());
            if let Some(idle) = idle_exit {
                hub.exit_when_idle(idle);
            }
            bridge::Target::Hub(hub)
        }
        // Another bridge holds the port: serve MCP through it, and take the
        // port over if it exits.
        Err(bridge::StartError::InUse(port)) if !no_mcp => {
            eprintln!("benchup bridge: another bridge is listening on port {port}; forwarding to it");
            let client = bridge::Client::new(port, config.token.clone(), "agent");
            let mut backend = bridge::Live::forwarding(client, config);
            mcp::serve(stdin().lock(), stdout().lock(), &mut backend).map_err(|e| e.to_string())?;
            std::process::exit(0);
        }
        Err(error) => return Err(error.to_string()),
    };
    if no_mcp {
        loop {
            std::thread::park();
        }
    }
    let mut backend = bridge::Live::new(target);
    mcp::serve(stdin().lock(), stdout().lock(), &mut backend).map_err(|e| e.to_string())?;
    std::process::exit(0);
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{}", pretty(&output.value));
            if output.ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Err(message) => {
            eprintln!("benchup: {message}");
            ExitCode::from(2)
        }
    }
}
