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
REVIEWED_PROTOCOL_COMMENTS = {('codex-rs/tools/src/tool_spec.rs', 81), ('codex-rs/codex-mcp/src/codex_apps/file_params.rs', 7), ('codex-rs/protocol/src/config_types.rs', 59), ('codex-rs/tools/src/tool_spec.rs', 38), ('codex-rs/tools/src/tool_spec.rs', 35), ('codex-rs/core/src/tools/handlers/apply_patch_spec.rs', 8), ('codex-rs/protocol/src/openai_models.rs', 70), ('codex-rs/tools/src/json_schema/types.rs', 14)}
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
    elif '/schema/' in path or path.startswith(('sdk/typescript/src/generated/', 'sdk/python/src/openai_codex/generated/')):
        category, reason = '生成协议数据定义（保留）', '不执行请求；与旧客户端的数据兼容，保留不代表相应服务可用'
    elif any(part in path.lower() for part in ('/tests/', '_tests.', '/snapshots/', '/fixtures/')):
        category, reason = '未完成：旧测试/夹具', '尚未随其所属剩余服务删除或中性化'
    elif path.endswith('.rs'):
        source = source_cache.setdefault(path, (root / path).read_text().splitlines())
        before = '\n'.join(source[:int(number)])
        if '#[cfg(test)]' in before:
            category, reason = '未完成：内联测试', '尚未随其所属剩余服务删除或中性化'
        elif content.lstrip().startswith(('//', '*')):
            if (path, int(number)) in REVIEWED_PROTOCOL_COMMENTS:
                category, reason = '协议层注释/参考（已核对）', '已逐条核对：描述工具调用、推理、结构化输出或 Apps 文件元数据格式；注释不执行请求'
            else:
                category, reason = '注释（待人工确认）', '未在本次逐条核对清单中，不得自动豁免'
        else:
            category, reason = '未完成：运行源码候选', '可能包含端点/域名特例/账号数据；必须继续人工拆除，当前不通过'
    rows.append([path, number, category, reason, content])
with (root / 'strip-evidence/domain-audit.tsv').open('w') as out:
    writer = csv.writer(out, delimiter='\t', lineterminator='\n')
    writer.writerow(['file', 'line', 'category', 'reason', 'source'])
    writer.writerows(rows)
counts = {}
for row in rows:
    counts[row[2]] = counts.get(row[2], 0) + 1
summary = {'pattern': pattern, 'tracked_match_lines': len(rows), 'categories': counts,
           'hard_acceptance': 'NOT PASSED: unresolved runtime candidates remain',
           'scope': 'Tracked worktree at execution time, excluding the two audit outputs to prevent recursive duplication'}
(root / 'strip-evidence/domain-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
print(json.dumps(summary, ensure_ascii=False, indent=2))
