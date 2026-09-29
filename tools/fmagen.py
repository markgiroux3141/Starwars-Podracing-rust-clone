"""Generate fused-multiply-add mutants for a straight-line float port.

    .venv/Scripts/python tools/fmagen.py FILE FUNC [FUNC ...] > SPEC.json

For every `f[d].set_fl(f[a].fl() + f[b].fl())` (or `-`) in the functions
whose operand is a product `f[x] = f[p] * f[q]`, it emits a mutant that
fuses that product into the add (`p.mul_add(q, c)`, negated factors for
subtractions). The factors are named by a register that still holds them
at the add, or by a load from memory that still holds them, found by
tracking values symbolically through the function (loads, stores, moves,
arithmetic, and `sp` adjustments). Products whose factors are gone are
reported on stderr and skipped.

The output is a list of tools/genmut.py entries ({"fn", "line", "nth",
"new", "why"}, anchored at the function); put them in a spec's
"mutants" with "defaults" (file, test, filter) and run genmut.py. Only straight-line
code is understood: a branch makes the tracking unreliable, so functions
with `if`/`loop` are refused. Text analysis only; it reads nothing
ROM-derived.
"""
import json
import re
import sys

path, funcs = sys.argv[1], sys.argv[2:]
text = open(path, encoding='utf-8').read().replace('\r\n', '\n')

LOAD = re.compile(r'f\[(\d+)\]\.set_u32l\(lw\(m, g\[(\w+)\], (-?0x[0-9A-Fa-f]+|-?\d+)\) as u32\);$')
STORE = re.compile(r'sw\(m, g\[(\w+)\], (-?0x[0-9A-Fa-f]+|-?\d+), u64::from\(f\[(\d+)\]\.u32l\(\)\)\);$')
BIN = re.compile(r'f\[(\d+)\]\.set_fl\(f\[(\d+)\]\.fl\(\) ([-+*/]) f\[(\d+)\]\.fl\(\)\);$')
NEG = re.compile(r'f\[(\d+)\]\.set_fl\(-f\[(\d+)\]\.fl\(\)\);$')
MOVE = re.compile(r'f\[(\d+)\]\.set_u32l\(f\[(\d+)\]\.u32l\(\)\);$')
SETF = re.compile(r'f\[(\d+)\]\.(set_u32l|set_fl|set_d|u64)')
SP_ADJ = re.compile(r'g\[SP\] = addu\(g\[SP\], \(?(-?0x[0-9A-Fa-f]+|-?\d+)(?:i64\) as u64)?\);$')


def num(s):
    return int(s, 0)


out = []
for fn in funcs:
    start = text.index(f'fn {fn}(')
    end = text.index('\n}\n', start)
    body = text[start:end].split('\n')[1:]
    if any(re.match(r'\s*(if|loop|while|for|match)\b', l) or "'" in l for l in body):
        sys.exit(f'{fn}: not straight-line code')
    regs = {}
    mem = {}
    sp = 0
    seen = {}

    def val(r):
        return regs.get(r, ('init', r))

    def addr(base, off):
        return (base, off + sp) if base == 'SP' else (base, off)

    def name(v):
        """An expression for value v at this point, or None."""
        for r, rv in regs.items():
            if rv == v:
                return f'f[{r}].fl()'
        if v[0] == 'init':
            return f'f[{v[1]}].fl()' if v[1] not in regs else None
        # Memory that holds v now: a slot stored with it, or v's own
        # address if it was loaded from there and never overwritten.
        slots = [a for a, mv in mem.items() if mv == v]
        if v[0] == 'mem' and v[1] not in mem:
            slots.append(v[1])
        for base, off in slots:
            o = off - sp if base == 'SP' else off
            return f'f32::from_bits(lw(m, g[{base}], {hex(o) if o >= 0 else "-" + hex(-o)}) as u32)'
        return None

    for line in body:
        l = line.strip()
        key = l
        nth = seen.get(key, 0)
        seen[key] = nth + 1
        if m := SP_ADJ.match(l):
            sp += num(m.group(1))
        elif m := LOAD.match(l):
            d, base, off = int(m.group(1)), m.group(2), num(m.group(3))
            a = addr(base, off)
            regs[d] = mem.get(a, ('mem', a))
        elif m := STORE.match(l):
            base, off, s = m.group(1), num(m.group(2)), int(m.group(3))
            mem[addr(base, off)] = val(s)
        elif m := BIN.match(l):
            d, a, op, b = int(m.group(1)), int(m.group(2)), m.group(3), int(m.group(4))
            va, vb = val(a), val(b)
            if op in '+-':
                for side, v in (('a', va), ('b', vb)):
                    if v[0] != '*':
                        continue
                    p, q = name(v[1]), name(v[2])
                    other = f'f[{b}].fl()' if side == 'a' else f'f[{a}].fl()'
                    if p is None or q is None:
                        print(f'{fn}: {l} ({side}): factors gone', file=sys.stderr)
                        continue
                    if op == '+':
                        new = f'f[{d}].set_fl({p}.mul_add({q}, {other}));'
                    elif side == 'a':
                        new = f'f[{d}].set_fl({p}.mul_add({q}, -{other}));'
                    else:
                        new = f'f[{d}].set_fl((-{p}).mul_add({q}, {other}));'
                    out.append({'fn': fn, 'line': l, 'nth': nth, 'new': new, 'why': f'fma {side}: {l}'})
            regs[d] = (op, va, vb)
        elif m := NEG.match(l):
            regs[int(m.group(1))] = ('neg', val(int(m.group(2))))
        elif m := MOVE.match(l):
            regs[int(m.group(1))] = val(int(m.group(2)))
        elif m := SETF.search(l):
            regs[int(m.group(1))] = ('opaque', len(out), l)
json.dump(out, sys.stdout, indent=1)
print(f'{len(out)} mutants', file=sys.stderr)
