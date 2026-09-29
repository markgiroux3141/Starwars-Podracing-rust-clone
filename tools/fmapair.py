"""Generate fused-multiply-add mutants for float ports with control flow.

    .venv/Scripts/python tools/fmapair.py FILE FUNC [FUNC ...] > SPEC.json

For every product `f[x].set_fl(f[p].fl() * f[q].fl())` in the functions,
it finds each later `f[d].set_fl(f[a].fl() +/- f[b].fl())` that reads
`f[x]` before `f[x]` is written again, in the same block as the product or
one nested inside it (the scan stops where the product's block closes).
The mutant captures the factors just before the product (`let fma =
(p, q);`, so loops that reload them before the add still work) and fuses
them into the add: `fma.0.mul_add(fma.1, c)`, with negated factors for a
subtracted product.

It follows the product through memory: an `sw` of it to `g[R] + off` and
a later `lw` back from the same place, `g[R]` not written in between (a
frame spill, or an output reread). It doesn't consider aliasing between
different base registers, and otherwise doesn't track values as
tools/fmagen.py does; in exchange it handles loops and branches. It misses
a product whose add sits outside the product's block (after a loop or an
`if` ends), and reports it on stderr.

The output is a list of tools/genmut.py entries ({"fn", "line", "nth",
"new", "also", "why"}); put them in a spec's "mutants" with "defaults" and
run genmut.py. Text analysis only; it reads nothing ROM-derived.
"""
import json
import re
import sys

path, funcs = sys.argv[1], sys.argv[2:]
text = open(path, encoding='utf-8').read().replace('\r\n', '\n')

MUL = re.compile(r'f\[(\d+)\]\.set_fl\(f\[(\d+)\]\.fl\(\) \* f\[(\d+)\]\.fl\(\)\);$')
ADD = re.compile(r'f\[(\d+)\]\.set_fl\(f\[(\d+)\]\.fl\(\) ([-+]) f\[(\d+)\]\.fl\(\)\);$')
WRITE = re.compile(r'f\[(\d+)\]\.(?:set_u32l|set_fl|set_d|u64 =)')
SPILL = re.compile(r'sw\(m, g\[(\w+)\], (-?0x[0-9A-Fa-f]+|-?\d+), u64::from\(f\[(\d+)\]\.u32l\(\)\)\);$')
RELOAD = re.compile(r'f\[(\d+)\]\.set_u32l\(lw\(m, g\[(\w+)\], (-?0x[0-9A-Fa-f]+|-?\d+)\) as u32\);$')
GWRITE = re.compile(r'g\[(\w+)\] =')


def indent(l):
    return len(l) - len(l.lstrip())


out = []
for fn in funcs:
    start = text.index(f'fn {fn}(')
    end = text.index('\n}\n', start)
    body = text[start:end].split('\n')[1:]
    seen = {}
    nths = []
    for l in body:
        k = l.strip()
        nths.append(seen.get(k, 0))
        seen[k] = nths[-1] + 1
    for i, l in enumerate(body):
        m = MUL.match(l.strip())
        if not m:
            continue
        x, p, q = m.groups()
        used = False
        # Registers holding the product, and frame slots it was spilled to.
        holders, slots = {x}, set()
        for j in range(i + 1, len(body)):
            lj = body[j]
            if lj.strip() and indent(lj) < indent(l):
                break
            s = lj.strip()
            if st := SPILL.match(s):
                base, off, r = st.groups()
                (slots.add if r in holders else slots.discard)((base, off))
                continue
            if ld := RELOAD.match(s):
                r, base, off = ld.groups()
                (holders.add if (base, off) in slots else holders.discard)(r)
                if not holders and not slots:
                    break
                continue
            if gw := GWRITE.match(s):
                slots = {sl for sl in slots if sl[0] != gw.group(1)}
            a = ADD.match(s)
            if a and (a.group(2) in holders or a.group(4) in holders):
                d, lhs, op, rhs = a.groups()
                if rhs in holders and op == '-':
                    new = f'f[{d}].set_fl((-fma.0).mul_add(fma.1, f[{lhs}].fl()));'
                elif rhs in holders:
                    new = f'f[{d}].set_fl(fma.0.mul_add(fma.1, f[{lhs}].fl()));'
                elif op == '-':
                    new = f'f[{d}].set_fl(fma.0.mul_add(fma.1, -f[{rhs}].fl()));'
                else:
                    new = f'f[{d}].set_fl(fma.0.mul_add(fma.1, f[{rhs}].fl()));'
                out.append({
                    'fn': fn, 'line': lj.strip(), 'nth': nths[j], 'new': new,
                    'also': [{'line': l.strip(), 'nth': nths[i],
                              'new': f'let fma = (f[{p}].fl(), f[{q}].fl()); {l.strip()}'}],
                    'why': f'fma: {l.strip()} into {lj.strip()} (line {j - i} after)'})
                used = True
            w = WRITE.search(s)
            if w:
                holders.discard(w.group(1))
                if not holders and not slots:
                    break
        if not used:
            print(f'{fn}: {l.strip()} (nth {nths[i]}): no add in its block', file=sys.stderr)
json.dump(out, sys.stdout, indent=1)
print(f'{len(out)} mutants', file=sys.stderr)
