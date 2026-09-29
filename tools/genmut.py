"""Generate tools/mutants.py JSON from compact specs anchored at a function.

    .venv/Scripts/python tools/genmut.py SPEC.json OUT.json

SPEC: {"defaults": {...}, "mutants": [{"fn": "func_X", "line": "exact line
text (stripped)", "nth": 0, "new": "replacement line (stripped)", "filter":
..., "why": ...}, ...]}. The pattern runs from `fn func_X(` to the end of
the nth matching line in that function, so it is unique in the file.
Entries may name their own `file` (else `defaults.file`). tools/fmagen.py
writes such entries for fused multiply-adds. Text editing only; it reads
nothing ROM-derived.
"""
import json, sys

spec = json.load(open(sys.argv[1], encoding='utf-8'))
out = {'defaults': spec['defaults'], 'mutants': []}
cache = {}
for m in spec['mutants']:
    path = m.get('file', spec['defaults'].get('file'))
    text = cache.setdefault(path, open(path, encoding='utf-8').read().replace('\r\n', '\n'))
    start = text.index(f'fn {m["fn"]}(')
    end = text.index('\n}\n', start)
    body = text[start:end]
    lines = body.split('\n')
    hits = [i for i, l in enumerate(lines) if l.strip() == m['line']]
    if len(hits) <= m.get('nth', 0):
        sys.exit(f'{m["fn"]}: line not found: {m["line"]}')
    i = hits[m.get('nth', 0)]
    indent = lines[i][:len(lines[i]) - len(lines[i].lstrip())]
    old = '\n'.join(lines[:i + 1])
    new = '\n'.join(lines[:i] + [indent + m['new']])
    e = {k: v for k, v in m.items() if k not in ('fn', 'line', 'nth', 'new')}
    e.update({'old': old, 'new': new})
    out['mutants'].append(e)
json.dump(out, open(sys.argv[2], 'w', encoding='utf-8'), indent=1)
print(len(out['mutants']), 'mutants')
