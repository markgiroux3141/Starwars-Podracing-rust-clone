"""Turn a translator draft's body into port style (session 10).

    .venv/Scripts/python tools/tidy_draft.py DRAFT.rs DOC.txt > PORT.rs

DRAFT is `cargo xtask translate` output (kept in a scratch directory: it is
derived from the generated C), DOC the port's doc comment.

Inlines `c1cs = X;` immediately followed by `if c1cs {` / `if !c1cs {`,
drops `// L_...` label comments and `0 as u32` casts, drops `let mut c1cs`
if nothing else uses it, and prepends the doc. Other c1cs uses are kept
and reported on stderr, to fix by hand, as are `lo`/`hi` locals (ports use
`let (lo, _) = multu(..)`). An FCR31 idiom is left expanded: replace it with
`fpu::to_unsigned_s` by hand. Text editing only.
"""
import re
import sys

draft, doc = sys.argv[1], sys.argv[2]
s = open(draft, encoding='utf-8').read()
body = s[s.index('pub unsafe extern'):]
lines = body.split('\n')
out = []
i = 0
while i < len(lines):
    l = lines[i]
    m = re.match(r'(\s*)c1cs = (.*);$', l)
    if m and i + 1 < len(lines):
        mm = re.match(r'(\s*)if (!?)c1cs \{$', lines[i + 1])
        if mm:
            cond = m.group(2)
            if mm.group(2):
                cond = f'!({cond})'
            out.append(f'{mm.group(1)}if {cond} {{')
            i += 2
            continue
    if re.match(r'\s*// L_[0-9A-F]{8}$', l):
        i += 1
        continue
    out.append(l.replace('(0 as u32)', '(0)'))
    i += 1
body = '\n'.join(out)
rest = body.replace('let mut c1cs = false;', '')
if 'c1cs' in rest:
    print(f'{draft}: c1cs left (fix by hand)', file=sys.stderr)
else:
    body = '\n'.join(l for l in body.split('\n') if l.strip() != 'let mut c1cs = false;')
if 'let (mut lo, mut hi)' in body:
    print(f'{draft}: lo/hi locals (fix by hand)', file=sys.stderr)
d = open(doc, encoding='utf-8').read().rstrip('\n')
sys.stdout.reconfigure(encoding='utf-8')
print(d)
print(body.rstrip('\n'))
