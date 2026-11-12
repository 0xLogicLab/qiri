"""Rerun changed suites and suites whose first run lacked delivery helpers."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[1]
evidence = root / "strip-evidence"
for name in ["delivery-retest-build", "delivery-helper-build"]:
    path = evidence / (name + ".exit")
    while not path.exists():
        time.sleep(5)
    assert path.read_text().strip() == "0", (name, path.read_text())

assert (evidence / "delivery-target/debug/codex").is_file(), "CLI helper must exist"
path = evidence / "delivery-tests.json"
rows = json.loads(path.read_text())
if not (evidence / "delivery-tests-first.json").exists():
    (evidence / "delivery-tests-first.json").write_text(json.dumps(rows, indent=2) + "\n")
artifacts = {}
for line in (evidence / "delivery-retest-build.jsonl").read_text().splitlines():
    item = json.loads(line)
    if item.get("executable") and item.get("profile", {}).get("test"):
        artifacts[item["target"]["name"]] = item["executable"]
packages = {
    "codex_app_server": "app-server", "codex_app_server_daemon": "app-server-daemon",
    "codex_mcp": "codex-mcp", "codex_guardian_v2": "ext/guardian-v2",
    "codex_mcp_extension": "ext/mcp", "codex_model_provider": "model-provider",
    "codex_tui": "tui",
}
env = dict(os.environ, CARGO_TARGET_DIR=str(evidence / "delivery-target"),
           INSTA_UPDATE="no", RUST_MIN_STACK="67108864")
for key in ["OPENAI_BASE_URL", "OPENAI_API_KEY"]:
    env.pop(key, None)
for row in rows:
    name = row["crate"]
    if name not in packages or (len(sys.argv) > 1 and name not in sys.argv[1:]):
        continue
    manifest = root / "codex-rs" / packages[name]
    binary = artifacts.get(name, row["binary"])
    threads = 8 if name in ["codex_tui", "codex_app_server"] else 1
    args = [binary, f"--test-threads={threads}"]
    for skipped in row["skipped_tests"]:
        args += ["--skip", skipped]
    log = evidence / row["log"]
    if not (evidence / (log.stem + "-first.log")).exists():
        (evidence / (log.stem + "-first.log")).write_bytes(log.read_bytes())
    started = time.monotonic()
    with log.open("w") as output:
        try:
            result = subprocess.run(args, cwd=manifest,
                                    env=dict(env, CARGO_MANIFEST_DIR=str(manifest)),
                                    stdout=output, stderr=subprocess.STDOUT, timeout=600)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = 124
    text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", log.read_text(errors="replace"))
    matches = list(re.finditer(
        r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", text))
    summary = matches[-1] if matches else None
    row.update(binary=binary, exit=code, elapsed_seconds=round(time.monotonic() - started, 2),
               sha256=hashlib.file_digest(open(binary, "rb"), "sha256").hexdigest(),
               passed=int(summary[2]) if summary else None,
               failed=int(summary[3]) if summary else None,
               ignored=int(summary[4]) if summary else None,
               filtered=int(summary[6]) if summary else None,
               failures=re.findall(r"^---- (.+?) stdout ----$", text, re.M),
               test_threads=threads)
    path.write_text(json.dumps(rows, indent=2) + "\n")
    print(json.dumps({k: row[k] for k in ["crate", "exit", "passed", "failed"]}), flush=True)
