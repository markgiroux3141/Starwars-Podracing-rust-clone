"""Register new ports in game::PORTED and crates/oracle/functions.txt.

Both lists are kept in address order. Run from the repo root:

    .venv/Scripts/python tools/register_ports.py MODULE ADDR:comment [ADDR:comment ...]

e.g. `tools/register_ports.py misc "80006E50:[o+0x100] |= a1"` adds
`misc::func_80006E50`. Text editing only; it reads nothing ROM-derived.
"""
import re
import sys

mod = sys.argv[1]
new = [a.split(':', 1) for a in sys.argv[2:]]

p = 'crates/game/src/lib.rs'
s = open(p, encoding='utf-8').read()
start = s.index('pub const PORTED: &[Ported] = &[\n') + len('pub const PORTED: &[Ported] = &[\n')
end = s.index('];', start)
lines = [l for l in s[start:end].splitlines() if l.strip()]
for a, _ in new:
    if f'"func_{a}"' in s[start:end]:
        sys.exit(f'func_{a} is already in PORTED')
    lines.append(f'    Ported {{ vram: 0x{a[:4]}_{a[4:]}, name: "func_{a}", func: {mod}::func_{a} }},')
lines.sort(key=lambda l: int(re.search(r'vram: 0x([0-9A-F_]+)', l).group(1).replace('_', ''), 16))
s = s[:start] + '\n'.join(lines) + '\n' + s[end:]
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = 'crates/oracle/functions.txt'
s = open(p, encoding='utf-8').read()
head, body = s.split('\n\n', 1)
entries = [l for l in body.splitlines() if l.strip()]
for a, what in new:
    entries.append(f'func_{a}  # {what} ({mod}::func_{a})')
entries.sort(key=lambda l: int(l.split()[0][5:], 16))
open(p, 'w', encoding='utf-8', newline='\n').write(head + '\n\n' + '\n'.join(entries) + '\n')
