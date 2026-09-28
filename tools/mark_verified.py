"""Mark ports rust_verified in symbols/functions.csv.

Run from the repo root:

    .venv/Scripts/python tools/mark_verified.py MODULE ADDR:notes [ADDR:notes ...]

Sets status rust_verified, test_kind unit_diff and confidence high (for the
behaviour; names stay as they are), and notes to "NOTES (game::MODULE)".
An entry `ADDR=name:notes` also renames the function (name_source ours).
Text editing only; it reads nothing ROM-derived.
"""
import csv
import io
import sys

mod = sys.argv[1]
todo = {}
for arg in sys.argv[2:]:
    head, notes = arg.split(':', 1)
    addr, _, name = head.partition('=')
    todo[f'0x{int(addr, 16):08X}'] = (name, notes)

p = 'symbols/functions.csv'
rows = list(csv.DictReader(open(p, encoding='utf-8', newline='')))
fields = list(rows[0].keys())
for r in rows:
    key = f"0x{int(r['vram'], 16):08X}"
    if key in todo:
        name, notes = todo.pop(key)
        if name:
            r['name'], r['name_source'] = name, 'ours'
        r['status'], r['test_kind'], r['confidence'] = 'rust_verified', 'unit_diff', 'high'
        r['notes'] = f'{notes} (game::{mod})'
if todo:
    sys.exit(f'not in functions.csv: {", ".join(todo)}')
out = io.StringIO()
w = csv.DictWriter(out, fieldnames=fields, lineterminator='\n')
w.writeheader()
w.writerows(rows)
open(p, 'w', encoding='utf-8', newline='').write(out.getvalue())
