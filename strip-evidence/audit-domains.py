"""Record every tracked domain match; unresolved source matches are never waived."""
from pathlib import Path
import csv
import json
import re
import subprocess

root = Path(__file__).resolve().parents[1]
pattern = r'chatgpt[.]com|developers[.]openai[.]com|auth[.]openai[.]com|api[.]openai[.]com|openai[.]com'
raw = subprocess.run(['git', 'grep', '-I', '-n', '-i', '-E', pattern, '--',
                      ':!strip-evidence/domain-audit.tsv', ':!strip-evidence/domain-grep.txt'],
                     cwd=root, capture_output=True, text=True)
assert raw.returncode in (0, 1), raw.stderr
(root / 'strip-evidence/domain-grep.txt').write_text(''.join(line.rstrip() + '\n' for line in raw.stdout.splitlines()))
import sys
sys.path.insert(0, str(root / 'strip-evidence'))
from rust_edit import block_end
PROTOCOL_REFERENCES = {
 'codex-rs/tools/src/tool_spec.rs', 'codex-rs/codex-mcp/src/codex_apps/file_params.rs',
 'codex-rs/protocol/src/config_types.rs', 'codex-rs/core/src/tools/handlers/apply_patch_spec.rs',
 'codex-rs/protocol/src/openai_models.rs', 'codex-rs/tools/src/json_schema/types.rs',
 'codex-rs/core/src/context_manager/history.rs', 'codex-rs/core/src/client.rs',
}
rows = []
source_cache = {}
for line in raw.stdout.splitlines():
    path, number, content = line.split(':', 2)
    content = content.rstrip()
    category, reason = '未完成：待逐条清理/审查', '不能作为协议层例外放行；不满足最终验收'
    if path.startswith(('strip-evidence/', 'release-evidence/')) or path.startswith('REPORT-'):
        category, reason = '审计元数据', '历史证据/报告记录，不是产品运行端点'
    elif path in ('LICENSE', 'NOTICE') or 'LICENSE' in Path(path).name:
        category, reason = '许可证/归因', '保留法律与上游声明'
    elif path == 'codex-rs/core/config.schema.json' or '/schema/' in path or path.startswith(('sdk/typescript/src/generated/', 'sdk/python/src/openai_codex/generated/')):
        category, reason = '生成协议数据定义（保留）', '不执行请求；与旧客户端的数据兼容，保留不代表相应服务可用'
    elif 'https://api.openai.com/auth' in content or 'https://api.openai.com/profile' in content:
        category, reason = '保留序列化 claim 数据键', 'JWT 命名空间字符串；仅 JSON 字段匹配，不构造 URL 或发送 HTTP。官方存储凭据不再激活；通用外部 OAuth/旧数据解析共享类型保留'
    elif path.endswith('.rs'):
        source = source_cache.setdefault(path, (root / path).read_text())
        offset = sum(len(line) + 1 for line in source.splitlines()[:int(number)-1])
        test_only = any(part in path.lower() for part in ('/tests/', '_tests.', '/fixtures/')) or Path(path).stem.endswith(('tests', 'test_support')) or '#![cfg(test)]' in source[:100]
        if not test_only:
            for match in re.finditer(r'#\[cfg\(test\)\]\s*(?:#\[[^\n]+\]\s*)*(?:pub(?:\([^)]*\))?\s+)?(?:mod|fn|async fn)\s+\w+', source):
                try:
                    if match.start() <= offset < block_end(source, match.start()): test_only = True
                except ValueError: pass
        if test_only:
            category, reason = '未完成：旧测试/夹具', '剩余非 claim 域名须继续中性化，不能自动豁免'
        elif content.lstrip().startswith(('//', '*')) and path in PROTOCOL_REFERENCES:
            category, reason = '协议层注释/参考（已核对）', '格式/工具调用/流式事件/图像计数参考；仅源码注释，无预置文档 MCP 或网络请求'
        else:
            category, reason = '未完成：运行源码候选', '必须人工拆除或提供逐项数据定义与调用证明'
    elif '/snapshots/' in path or '/fixtures/' in path or '/tests/' in path:
        category, reason = '未完成：旧测试/夹具', '非 claim 旧夹具须中性化'
    rows.append([path, number, category, reason, content])
with (root / 'strip-evidence/domain-audit.tsv').open('w') as out:
    writer = csv.writer(out, delimiter='\t', lineterminator='\n')
    writer.writerow(['file', 'line', 'category', 'reason', 'source'])
    writer.writerows(rows)
counts = {}
for row in rows:
    counts[row[2]] = counts.get(row[2], 0) + 1
summary = {'pattern': pattern, 'tracked_match_lines': len(rows), 'categories': counts,
           'hard_acceptance': 'NOT PASSED' if any(row[2].startswith('未完成') for row in rows) else 'PASSED: every remaining match classified; no built-in official destination',
           'scope': 'Tracked worktree at execution time, excluding the two audit outputs to prevent recursive duplication'}
(root / 'strip-evidence/domain-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
print(json.dumps(summary, ensure_ascii=False, indent=2))
