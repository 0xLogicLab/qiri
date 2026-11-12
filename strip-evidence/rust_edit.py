"""Small source-edit helper: find balanced Rust blocks without counting strings/comments."""
import re
TOKEN = re.compile(r'''//[^\n]*|/\*.*?\*/|r(?P<hashes>\#*)".*?"(?P=hashes)|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])' ''', re.S | re.X)
def mask(source):
    return TOKEN.sub(lambda m: ''.join('\n' if c == '\n' else ' ' for c in m[0]), source)
def block_end(source, start):
    clean = mask(source)
    opening = clean.index('{', start)
    depth = 1
    for pos in range(opening + 1, len(clean)):
        depth += (clean[pos] == '{') - (clean[pos] == '}')
        if depth == 0:
            return pos + 1
    raise ValueError('unbalanced Rust block')
def remove_fn(source, name):
    found = re.search(r'(?m)^([ \t]*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+' + re.escape(name) + r'(?:\s*<|\s*\()', source)
    if not found:
        raise ValueError('missing function: ' + name)
    return source[:found.start()] + source[block_end(source, found.start()):]
def replace_fn_body(source, name, body):
    found = re.search(r'(?m)^([ \t]*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+' + re.escape(name) + r'(?:\s*<|\s*\()', source)
    if not found:
        raise ValueError('missing function: ' + name)
    opening = mask(source).index('{', found.start())
    return source[:opening+1] + '\n' + body + '\n' + found[1] + '}' + source[block_end(source, found.start()):]
