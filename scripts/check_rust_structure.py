#!/usr/bin/env python3
"""Static structural validation of SoundPilot Rust sources via tree-sitter.
Checks: parse succeeds, brace balance, no TODO/stub markers, and structural
assertions the CI contract depends on. Not a compiler — a guard."""
import sys, pathlib
sys.path.insert(0, "/tmp/pylibs")
import tree_sitter_rust as tsrust
from tree_sitter import Language, Parser

RUST_LANG = Language(tsrust.language())
parser = Parser(RUST_LANG)

SRC = pathlib.Path("/data/projects/SoundPilot/src-tauri/src")
errors = []

def parse_file(p):
    tree = parser.parse(p.read_bytes())
    errs = [n for n in tree.root_node.children if n.type == "ERROR"]
    if errs:
        for n in errs:
            errors.append(f"{p.name}:{n.start_point[0]+1}: parse ERROR node")
    return tree

files = sorted(SRC.glob("*.rs"))
assert files, "no rust files found"
for f in files:
    src = f.read_text()
    # brace/paren balance
    for open_c, close_c in [("{","}"),("(",")"),("[","]")]:
        # crude string/comment/char-literal-stripped balance
        import re
        cleaned = re.sub(r"r?#*\"(?:[^\"\\]|\\.)*\"", '""', src)  # strings
        cleaned = re.sub(r"'(?:[^'\\]|\\.)'", "'x'", cleaned)     # char literals
        cleaned = re.sub(r"//[^\n]*", "", cleaned)
        cleaned = re.sub(r"/\*.*?\*/", "", cleaned, flags=re.S)
        if cleaned.count(open_c) != cleaned.count(close_c):
            errors.append(f"{f.name}: unbalanced {open_c}{close_c} "
                          f"({cleaned.count(open_c)} vs {cleaned.count(close_c)})")
    if "TODO" in src or "unimplemented!" in src or "todo!()" in src:
        errors.append(f"{f.name}: contains TODO/unimplemented marker")
    parse_file(f)

# ---------- structural assertions ----------
lib = (SRC / "lib.rs").read_text()
for cmd in ["get_config","save_config","get_status","list_processes","pause_monitor",
            "resume_monitor","refresh_volumes","set_manual_volume","pick_executable",
            "import_config","export_config","quit_app"]:
    if f"fn {cmd}(" not in lib:
        errors.append(f"lib.rs: missing command {cmd}")
    if cmd not in lib.split("generate_handler![",1)[1] if "generate_handler![" in lib else True:
        pass
# all commands registered in handler
handler_block = lib.split("generate_handler![",1)[1].split("]",1)[0]
for cmd in ["get_config","save_config","get_status","list_processes","pause_monitor",
            "resume_monitor","refresh_volumes","set_manual_volume","pick_executable",
            "import_config","export_config","quit_app"]:
    if cmd not in handler_block:
        errors.append(f"lib.rs: {cmd} not in generate_handler")

mon = (SRC / "monitor.rs").read_text()
for needle in ["丢弃过期音量任务", "generation != state.current_generation()"]:
    if needle not in mon:
        errors.append(f"monitor.rs: missing {needle!r}")

conf = (SRC / "config.rs").read_text()
for needle in ['rename_all = "camelCase"', "start_paused: false", "json.tmp", "json.bak"]:
    if needle not in conf:
        errors.append(f"config.rs: missing {needle!r}")

# token leakage guards: log statements must not interpolate the token variable
import re as _re
for f in files:
    for i, line in enumerate(f.read_text().splitlines(), 1):
        if ("log_event" in line or "println!" in line or "eprintln!" in line):
            # 拦截把 token 变量本身放进格式化参数的行为
            if _re.search(r"(log_event|println!|eprintln!)\([^)]*\{[^}]*\}\s*,", line) and _re.search(r"\btoken\b", line.split(",", 1)[-1]):
                errors.append(f"{f.name}:{i}: token 值进入日志: {line.strip()[:90]}")

if errors:
    print("FAILED:")
    for e in errors:
        print(" -", e)
    sys.exit(1)
print(f"OK: {len(files)} files structurally valid:", ", ".join(f.name for f in files))
