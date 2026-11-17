#!/usr/bin/env python3
"""Prepare a round of the agent evaluation (see README.md).

  python3 crates/benchup-cli/evals/setup.py --round 5 [--cases 01,13] [--binary target/release/benchup]
  python3 crates/benchup-cli/evals/setup.py --round 7 --live --token TOKEN [--port 47615] [--cases 01,13]

Creates /tmp/benchup-eval<round>/ with the binary in bin/, one folder per
case with its seed project, and prompts/<case>.md with the exact prompt to
give each agent. Prints a JSON list of {id, dir, prompt, file}. Run it from the
repository root; it never builds anything.

With --live, the agents edit the project open in BenchUp through
`benchup ... --live` instead of the file: open each case's `file` in BenchUp
(new cases get an empty project to open) and run the cases one at a time,
since one BenchUp serves one bridge. The requests stay the same.
"""
import argparse, json, os, shlex, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CLI_ACCESS = ("- The `benchup` CLI is on that PATH. Learn it ONLY from `benchup --help`, what the help points you to "
              "(e.g. `benchup guide`, `benchup tools`, `benchup tools <name>`) and the outputs/errors it gives you.")
MCP_ACCESS = ("- You reach the project ONLY through BenchUp's MCP server, as an MCP client would. Use the client "
              "`python3 {bin}/mcp_call.py FILE list` (tools and schemas), `... FILE call TOOL 'JSON'` and "
              "`... FILE render VIEW OUT.png`. Start with the server's get_guide tool. Do NOT run the `benchup` "
              "command directly.")
LIVE_ACCESS = ("- The project is open in the BenchUp app on this computer: {opened}. Edit it there, live, with "
               "`benchup` commands that take `--live` instead of a file; the pairing token and port are already set by "
               "the command prefix. Do not create, read or write .bup files on disk, and do not start or stop a bridge "
               "yourself. Learn the CLI ONLY from `benchup --help`, what the help points you to (e.g. `benchup guide`, "
               "`benchup tools`, `benchup tools <name>`) and the outputs/errors it gives you.")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--round", required=True, type=int)
    parser.add_argument("--cases", help="comma-separated case number prefixes, e.g. 01,13 (default: all)")
    parser.add_argument("--binary", default=str(ROOT / "target/release/benchup"))
    parser.add_argument("--out", help="default /tmp/benchup-eval<round>")
    parser.add_argument("--live", action="store_true", help="agents edit the project open in BenchUp (--live)")
    parser.add_argument("--token", help="with --live: the pairing token shown in BenchUp")
    parser.add_argument("--port", type=int, help="with --live: the bridge port set in BenchUp (default 47615)")
    args = parser.parse_args()
    if args.live and not args.token:
        sys.exit("--live needs --token, the pairing token from BenchUp (Settings → Agent connection)")
    if args.live and any(c["mode"] == "mcp" for c in json.loads((HERE / "cases.json").read_text())["cases"]
                         if not args.cases or any(c["id"].startswith(w) for w in args.cases.split(","))):
        sys.exit("--live runs CLI cases only; leave MCP cases out with --cases")
    binary = Path(args.binary)
    if not binary.is_file():
        sys.exit(f"no benchup binary at {binary}; build it with `cargo build --release -p benchup-cli` first")
    manifest = json.loads((HERE / "cases.json").read_text())
    wanted = args.cases.split(",") if args.cases else None
    cases = [c for c in manifest["cases"] if not wanted or any(c["id"].startswith(w) for w in wanted)]
    out = Path(args.out or f"/tmp/benchup-eval{args.round}")
    if out.exists():
        shutil.rmtree(out)
    (out / "bin").mkdir(parents=True)
    (out / "prompts").mkdir()
    shutil.copy2(binary, out / "bin/benchup")
    shutil.copy2(HERE / "mcp_call.py", out / "bin/mcp_call.py")
    template = (HERE / "prompt.md").read_text()
    listing = []
    for case in cases:
        folder = out / case["id"]
        folder.mkdir()
        seed = manifest["seeds"][case["seed"]]
        if seed:
            shutil.copy2(ROOT / seed, folder / case["file"])
        elif args.live:
            # BenchUp needs a project to open; the request still asks to create it.
            subprocess.run([str(out / "bin/benchup"), "new", str(folder / case["file"]),
                            "--name", Path(case["file"]).stem.capitalize()], check=True, capture_output=True)
        mcp = case["mode"] == "mcp"
        exports = f"PATH={out / 'bin'}:$PATH"
        if args.live:
            exports += f" BENCHUP_AGENT_TOKEN={shlex.quote(args.token)}"
            if args.port:
                exports += f" BENCHUP_BRIDGE_PORT={args.port}"
            opened = (f"`{case['file']}`" if seed else
                      f"an empty project named `{Path(case['file']).stem.capitalize()}`, standing for `{case['file']}`")
            access = LIVE_ACCESS.format(opened=opened)
        else:
            access = (MCP_ACCESS if mcp else CLI_ACCESS).format(bin=out / "bin")
        prompt = template.format(
            request=case["request"], dir=folder, bin=out / "bin", root=out.parent / "benchup-eval*",
            access=access, exports=exports, unit="MCP tool calls" if mcp else "benchup commands",
        )
        if not seed and not args.live:
            prompt = prompt.replace("Work only there.", "Work only there. The project does not exist yet; create it there.")
        path = out / "prompts" / f"{case['id']}.md"
        path.write_text(prompt)
        listing.append({"id": case["id"], "title": case["title"], "dir": str(folder), "prompt": str(path),
                        "file": str(folder / case["file"])})
    print(json.dumps(listing, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main()
