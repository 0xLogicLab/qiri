"""Loopback Responses protocol smoke; this is a simulated model, not real inference."""
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[1]
requests = []
marker_seen = False

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        global marker_seen
        payload = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        requests.append({'path': self.path, 'model': payload.get('model')})
        assert self.path == '/v1/responses', self.path
        if len(requests) == 1:
            names = [tool.get('name') for tool in payload.get('tools', [])]
            if 'exec_command' in names:
                name, args = 'exec_command', {'cmd': 'printf QIRI_LOCAL_EXEC_OK', 'max_output_tokens': 1000}
            elif 'shell_command' in names:
                name, args = 'shell_command', {'command': 'printf QIRI_LOCAL_EXEC_OK'}
            else:
                assert 'shell' in names, names
                name, args = 'shell', {'command': ['bash', '-lc', 'printf QIRI_LOCAL_EXEC_OK']}
            event = {'type': 'response.output_item.done', 'item': {
                'type': 'function_call', 'call_id': 'local_smoke_call',
                'name': name, 'arguments': json.dumps(args)}}
        else:
            marker_seen = marker_seen or any(
                i.get('type') == 'function_call_output' and 'QIRI_LOCAL_EXEC_OK' in str(i)
                for i in payload.get('input', []))
            event = {'type': 'response.output_item.done', 'item': {
                'type': 'message', 'role': 'assistant', 'id': 'msg_local',
                'content': [{'type': 'output_text', 'text': 'QIRI_LOCAL_EXEC_OK'}]}}
        completed = {'type': 'response.completed', 'response': {
            'id': f'local_response_{len(requests)}',
            'usage': {'input_tokens': 1, 'output_tokens': 1, 'total_tokens': 2}}}
        body = ''.join('event: ' + e['type'] + '\ndata: ' + json.dumps(e) + '\n\n'
                       for e in [event, completed]).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
with tempfile.TemporaryDirectory(prefix='qiri-exec-', dir=ROOT / 'strip-evidence') as temp:
    env = os.environ.copy()
    # Isolate the child application's credential/config store; do not touch user settings.
    env['CODEX_HOME'] = temp
    binary = ROOT / 'codex-rs/target/debug/codex'
    provider = json.dumps({'name': 'loopback', 'base_url': f'http://127.0.0.1:{server.server_port}/v1',
                           'wire_api': 'responses', 'supports_websockets': False})
    # TOML inline table, with all values supplied here rather than from user configuration.
    provider = '{ name = "loopback", base_url = "http://127.0.0.1:%d/v1", wire_api = "responses", supports_websockets = false }' % server.server_port
    command = [str(binary), 'exec', '--json', '--skip-git-repo-check',
               '-c', 'model_provider="smoke"', '-c', 'model="qiri-local-smoke"',
               '-c', 'sandbox_mode="danger-full-access"',
               '-c', 'approval_policy="never"', '-c', 'model_providers.smoke=' + provider,
               'Run printf QIRI_LOCAL_EXEC_OK with the shell tool, then report its output.']
    result = subprocess.run(command, cwd=temp, env=env, capture_output=True, text=True, timeout=180)
    (ROOT / 'strip-evidence/local-exec.stdout').write_text(result.stdout)
    (ROOT / 'strip-evidence/local-exec.stderr').write_text(result.stderr)
    report = {'kind': 'simulated-loopback-model', 'exit_code': result.returncode,
              'requests': requests, 'tool_output_seen_by_model': marker_seen,
              'final_marker': 'QIRI_LOCAL_EXEC_OK' in result.stdout}
    (ROOT / 'strip-evidence/local-exec.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    assert result.returncode == 0 and marker_seen and report['final_marker']
server.shutdown()
