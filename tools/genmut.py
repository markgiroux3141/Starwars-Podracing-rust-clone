"""Generate tools/mutants.py JSON from compact specs anchored at a function.

    .venv/Scripts/python tools/genmut.py SPEC.json OUT.json

SPEC: {"defaults": {...}, "mutants": [{"fn": "func_X", "line": "exact line
text (stripped)", "nth": 0, "new": "replacement line (stripped)", "filter":
..., "why": ...}, ...]}. The pattern runs from `fn func_X(` to the end of
the nth matching line in that function, so it is unique in the file.
An entry may also carry `"also": [{"line", "nth", "new"}, ...]`, earlier
lines of the same function replaced in the same mutant. Entries may name
their own `file` (else `defaults.file`). tools/fmagen.py and
tools/fmapair.py write such entries for fused multiply-adds. Text editing
only; it reads nothing ROM-derived.
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

    def find(line, nth):
        hits = [i for i, l in enumerate(lines) if l.strip() == line]
        if len(hits) <= nth:
            sys.exit(f'{m["fn"]}: line not found: {line}')
        return hits[nth]

    def indented(i, text):
        return lines[i][:len(lines[i]) - len(lines[i].lstrip())] + text

    i = find(m['line'], m.get('nth', 0))
    old = '\n'.join(lines[:i + 1])
    changed = lines[:i] + [indented(i, m['new'])]
    for a in m.get('also', []):
        j = find(a['line'], a.get('nth', 0))
        if j >= i:
            sys.exit(f'{m["fn"]}: "also" line after the main one: {a["line"]}')
        changed[j] = indented(j, a['new'])
    new = '\n'.join(changed)
    e = {k: v for k, v in m.items() if k not in ('fn', 'line', 'nth', 'new', 'also')}
    e.update({'old': old, 'new': new})
    out['mutants'].append(e)
json.dump(out, open(sys.argv[2], 'w', encoding='utf-8'), indent=1)
print(len(out['mutants']), 'mutants')
