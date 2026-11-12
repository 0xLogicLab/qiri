"""Compare final libtest failures with the unmodified strip baseline."""
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parents[1]
evidence = root / "strip-evidence"


def parse_log(path):
    text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", path.read_text(errors="replace"))
    matches = list(re.finditer(
        r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
        text,
    ))
    result = matches[-1] if matches else None
    failures = sorted(set(re.findall(r"^---- (.+?) stdout ----$", text, re.M)))
    return {
        "log": path.name,
        "completed": result is not None,
        "passed": int(result[2]) if result else None,
        "failed": int(result[3]) if result else None,
        "ignored": int(result[4]) if result else None,
        "filtered": int(result[6]) if result else None,
        "failures": failures,
    }


baseline_paths = {
    "codex_core": "baseline-final-codex_core.log",
    "codex_guardian_v2": "baseline-final-codex_guardian_v2.log",
    "codex_tui": "baseline-final-codex_tui.log",
    "codex_config": "baseline-config-tests.log",
}
rows = []
for test in json.loads((evidence / "delivery-tests.json").read_text()):
    current = parse_log(evidence / test["log"])
    baseline = parse_log(evidence / baseline_paths[test["crate"]]) if test["crate"] in baseline_paths else None
    new = sorted(set(current["failures"]) - set(baseline["failures"] if baseline else []))
    rows.append({
        "crate": test["crate"], "current": current, "baseline": baseline,
        "new_failures": new, "skipped_tests": test["skipped_tests"],
        "acceptable": current["completed"] and not new
        and (current["failed"] == 0 or (baseline is not None and baseline["completed"])),
    })
summary = {
    "baseline": "7c3786d", "suites": rows,
    "all_suites_completed": all(row["current"]["completed"] for row in rows),
    "no_new_failures": all(row["acceptable"] for row in rows),
    "total_passed": sum(row["current"]["passed"] or 0 for row in rows),
    "total_failed": sum(row["current"]["failed"] or 0 for row in rows),
}
(evidence / "delivery-baseline-comparison.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps({k: v for k, v in summary.items() if k != "suites"}, indent=2))
for row in rows:
    if not row["acceptable"]:
        print(row["crate"], row["new_failures"], "completed", row["current"]["completed"])
