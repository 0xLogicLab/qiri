import subprocess, re, collections, json
from pathlib import Path
root=Path.cwd()
# Release evidence is excluded to avoid auditing the audit recursively.
paths=[p for p in subprocess.check_output(['git','ls-files','-z']).decode().split('\0') if p and not p.startswith('release-evidence/') and p != 'REPORT-RELEASE.md']
patterns={'service-chat':'chatgpt[.]com','service-docs':'developers[.]openai[.]com','private-pcl':'llmapi[.]pcl','private-research':'research-os[.]online','private-tailnet':'100[.]75[.]'}
counts=collections.Counter(); rows=[]; ids=[]
for name in paths:
    data=Path(name).read_bytes()
    if b'\0' in data: continue
    for lineno,line in enumerate(data.decode('utf8',errors='replace').splitlines(),1):
        for label,pat in patterns.items():
            if re.search(pat,line,re.I):
                counts[label]+=1
                if 'skills/src/assets/' in name: kind='内置技能资料'; reason='修改嵌入技能会改变工具指引行为，保留并列为未清理项'
                elif name.endswith('.md'): kind='上游技术资料'; reason='第三方服务技术参考，保留并列为未清理项'
                elif 'test' in name or name.endswith('.snap'): kind='兼容性测试'; reason='保留上游路由、认证或输出契约测试及快照'
                else: kind='内部实现'; reason='端点、认证信任域、网络策略或显示字符串；修改需另行行为变更评估'
                rows.append((label,name,str(lineno),kind,reason))
        matches=list(re.finditer('openai',line,re.I))
        for match in matches:
            if name=='LICENSE': kind='许可证'; reason='Apache-2.0 原文及适用声明'
            elif name=='NOTICE' or re.search(r'copyright.*openai',line,re.I): kind='版权头'; reason='保留上游版权'
            elif name.startswith(('docs/','README','CHANGELOG','SECURITY')) and not re.search(r'requires_openai_auth',line,re.I): kind='归因声明/协议说明'; reason='上游出处或兼容 API 规范名称'
            elif name.endswith('.md') or '/skills/src/assets/' in name: kind='上游技术资料'; reason='继承的组件文档或嵌入技能；未冒充 QIRI 自有服务'
            else: kind='内部实现名'; reason='保留依赖、协议、API、端点、发布标识、测试或维护配置兼容性'
            ids.append((name,str(lineno),str(match.start()+1),kind,reason))
Path('release-evidence/domain-audit.tsv').write_text('pattern_id\tfile\tline\tcategory\treason\n'+''.join('\t'.join(r)+'\n' for r in rows))
Path('release-evidence/identifier-audit.tsv').write_text('file\tline\tcolumn\tcategory\treason\n'+''.join('\t'.join(r)+'\n' for r in ids))
summary={'patterns':dict(counts),'remaining_domain_lines':len(rows),'identifier_occurrences':len(ids),'identifier_lines':len(set((r[0],r[1]) for r in ids)),'identifier_categories':dict(collections.Counter(r[3] for r in ids)),'scope':'tracked product tree excluding REPORT-RELEASE.md and release-evidence/'}
Path('release-evidence/audit-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(summary,ensure_ascii=False,indent=2))
