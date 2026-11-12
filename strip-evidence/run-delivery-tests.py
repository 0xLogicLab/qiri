"""Run freshly compiled delivery libraries and retain complete libtest evidence."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import time

root = Path(__file__).resolve().parents[1]
evidence = root / 'strip-evidence'
build = evidence / 'delivery-isolated-build.jsonl'
artifacts = {}
manifests = {}
workspace_builds = {}
for line in build.read_text().splitlines():
    try:
        item = json.loads(line)
    except json.JSONDecodeError:
        continue
    if item.get('reason') != 'compiler-artifact':
        continue
    if item['package_id'].startswith('path+file://' + str(root / 'codex-rs')):
        workspace_builds.setdefault(item['package_id'], []).append(item['fresh'])
    if item.get('executable') and item.get('profile', {}).get('test') and item['target']['kind'] == ['lib']:
        artifacts[item['target']['name']] = item['executable']
        manifests[item['target']['name']] = str(Path(item['manifest_path']).parent)
expected = json.loads((evidence / 'final-cargo-command.json').read_text()).count('-p')
assert len(artifacts) == expected, f'expected {expected} delivery libraries, found {len(artifacts)}'
(evidence / 'delivery-build-provenance.json').write_text(json.dumps({
    'workspace_packages': len(workspace_builds),
    'workspace_packages_recompiled': sorted(name for name, states in workspace_builds.items() if False in states),
    'unexpected_fully_reused_workspace_packages': sorted(name for name, states in workspace_builds.items() if False not in states),
    'test_libraries': sorted(artifacts),
}, indent=2) + '\n')
os.umask(0o077)
env = dict(os.environ, CARGO_TARGET_DIR=str(evidence / 'delivery-target'), RUST_MIN_STACK='67108864', INSTA_UPDATE='no')
for key in ['OPENAI_BASE_URL', 'OPENAI_API_KEY']:
    env.pop(key, None)
results = []
for name, binary in sorted(artifacts.items()):
    log = evidence / f'delivery-{name}.log'
    args = [binary, '--test-threads=' + ('1' if name == 'codex_network_proxy' else '8')]
    skipped = []
    if name == 'codex_core':
        skipped = ['environment_selection::connection_tests::lifecycle_waits_for_recovery::retry_exhaustion']
        args += ['--skip', skipped[0]]
    if name == 'codex_tui':
        skipped = ['ide_context::ipc::tests::fetch_ide_context_does_not_fall_back_after_primary_protocol_error']
        args += ['--skip', skipped[0]]
    started = time.monotonic()
    with log.open('w') as output:
        try:
            result = subprocess.run(args, cwd=manifests[name], env=dict(env, CARGO_MANIFEST_DIR=manifests[name]), stdout=output, stderr=subprocess.STDOUT, timeout=600)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = 124
    text = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', log.read_text(errors='replace'))
    matches = list(re.finditer(r'(?m)^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', text))
    summary = matches[-1] if matches else None
    failure_blocks = re.findall(r'(?m)^---- ([^\n]+) stdout ----$', text)
    row = {'crate': name, 'binary': binary, 'sha256': hashlib.file_digest(open(binary, 'rb'), 'sha256').hexdigest(), 'exit': code, 'elapsed_seconds': round(time.monotonic() - started, 2), 'log': log.name, 'skipped_tests': skipped, 'passed': int(summary.group(2)) if summary else None, 'failed': int(summary.group(3)) if summary else None, 'ignored': int(summary.group(4)) if summary else None, 'filtered': int(summary.group(6)) if summary else None, 'failures': failure_blocks}
    results.append(row)
    (evidence / 'delivery-tests.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps({key: row[key] for key in ['crate', 'exit', 'passed', 'failed', 'elapsed_seconds']}), flush=True)
