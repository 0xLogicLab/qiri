"""Exercise real inference through an existing Qiri profile without persisting its credentials."""
from pathlib import Path
import hashlib
import json
import os
import shlex
import subprocess
import tempfile
import time
import tomllib
from urllib.parse import urlparse

root = Path(__file__).resolve().parents[1]
profile_home = Path.home() / '.qiri-codex'
config = tomllib.loads((profile_home / 'config.toml').read_text())
settings = config | config.get('profiles', {}).get(config.get('profile'), {})
provider = dict(config['model_providers'][settings['model_provider']])
endpoint = provider['base_url']
model = settings['model']
host = urlparse(endpoint).hostname or ''
assert not any(host == domain or host.endswith('.' + domain) for domain in ['openai.com', 'chatgpt.com', 'openai.org', 'chatgpt-staging.com']), 'The profile must select a custom endpoint'
env = dict(os.environ)
env_file = Path.home() / '.qiri' / '.env'
profile_env = {}
if env_file.is_file():
    for line in env_file.read_text().splitlines():
        line = line.strip().removeprefix('export ')
        if not line or line.startswith('#') or '=' not in line:
            continue
        name, value = line.split('=', 1)
        if name.isidentifier():
            parsed = shlex.split(value, comments=True)
            if len(parsed) == 1:
                profile_env[name] = parsed[0]
key_name = provider.get('env_key')
needed = [key_name] + list(provider.get('env_http_headers', {}).values())
for name in needed:
    if name and not env.get(name) and profile_env.get(name):
        env[name] = profile_env[name]
if key_name:
    assert env.get(key_name), 'The configured credential environment variable is unavailable'
proxy = profile_env.get('QIRI_PROXY') or env.get('QIRI_PROXY')
if proxy:
    for name in ['http_proxy', 'https_proxy', 'HTTP_PROXY', 'HTTPS_PROXY']:
        env.setdefault(name, proxy)
secrets = [env[name] for name in needed if name and env.get(name)]
secrets += list(provider.get('http_headers', {}).values())
if provider.get('experimental_bearer_token'):
    secrets.append(provider['experimental_bearer_token'])
def toml_value(value):
    if isinstance(value, str): return json.dumps(value)
    if isinstance(value, bool): return str(value).lower()
    if isinstance(value, dict): return '{' + ', '.join(json.dumps(k) + ' = ' + toml_value(v) for k, v in value.items()) + '}'
    if isinstance(value, list): return '[' + ', '.join(map(toml_value, value)) + ']'
    return str(value)
def redact(text):
    for secret in sorted([v for v in secrets if v], key=len, reverse=True):
        text = text.replace(secret, '[REDACTED]')
    return text.replace(endpoint, '<configured_endpoint>').replace(host, '<configured_host>').replace(model, '<configured_model>').replace(str(Path.home()), '<home>')
started = time.monotonic()
with tempfile.TemporaryDirectory(prefix='.real-profile-', dir=root / 'strip-evidence') as temporary:
    home = Path(temporary)
    workspace = home / 'task'
    workspace.mkdir()
    selected = {k: v for k, v in provider.items() if k in ['base_url', 'env_key', 'env_key_instructions', 'experimental_bearer_token', 'http_headers', 'env_http_headers', 'query_params', 'wire_api', 'request_max_retries', 'stream_idle_timeout_ms']}
    selected.update(name='Local profile', requires_openai_auth=False)
    lines = [f'model = {toml_value(model)}', 'model_provider = "profile_local"', '[model_providers.profile_local]']
    lines.extend(f'{key} = {toml_value(value)}' for key, value in selected.items())
    (home / 'config.toml').write_text('\n'.join(lines) + '\n')
    env['CODEX_HOME'] = str(home)
    binary = root / 'strip-evidence/delivery-target/debug/codex'
    command = [str(binary), 'exec', '--json', '--skip-git-repo-check', '--sandbox', 'workspace-write', 'Run a shell command that prints QIRI_REAL_EXEC_OK, then reply QIRI_REAL_EXEC_DONE. Do not inspect or modify files.']
    try:
        result = subprocess.run(command, cwd=workspace, env=env, capture_output=True, text=True, timeout=180)
        stdout, stderr, exit_code = result.stdout, result.stderr, result.returncode
    except subprocess.TimeoutExpired as error:
        stdout = (error.stdout or b'').decode() if isinstance(error.stdout, bytes) else error.stdout or ''
        stderr = (error.stderr or b'').decode() if isinstance(error.stderr, bytes) else error.stderr or ''
        exit_code = 124
    events = []
    for line in stdout.splitlines():
        try: events.append(json.loads(line))
        except json.JSONDecodeError: pass
    completed_commands = [event.get('item', {}) for event in events if event.get('type') == 'item.completed' and event.get('item', {}).get('type') == 'command_execution']
    tool_ok = any(item.get('exit_code') == 0 and 'QIRI_REAL_EXEC_OK' in item.get('aggregated_output', '') for item in completed_commands)
    final_ok = any(event.get('type') == 'item.completed' and event.get('item', {}).get('type') == 'agent_message' and 'QIRI_REAL_EXEC_DONE' in event['item'].get('text', '') for event in events)
    summary = {'mode': 'real inference using existing Qiri profile', 'endpoint_is_loopback': host in ['localhost', '127.0.0.1', '::1'], 'credentials_persisted': False, 'binary_sha256': hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest(), 'exit_code': exit_code, 'tool_executed': tool_ok, 'final_marker': final_ok, 'elapsed_seconds': round(time.monotonic() - started, 2)}
    (root / 'strip-evidence/real-profile-exec.json').write_text(json.dumps(summary, indent=2) + '\n')
    (root / 'strip-evidence/real-profile-exec.stdout').write_text(redact(stdout))
    (root / 'strip-evidence/real-profile-exec.stderr').write_text(redact(stderr))
    print(json.dumps(summary))
    raise SystemExit(0 if exit_code == 0 and tool_ok and final_ok else 1)
