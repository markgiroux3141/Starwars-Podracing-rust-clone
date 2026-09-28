"""Run mutants: apply one textual mutation, run one test binary, restore.

Run from the repo root:

    .venv/Scripts/python tools/mutants.py MUTANTS.json [--show] [N ...]

`--show` prints the end of each cargo run's output; `N ...` runs only
those mutants (by index).

MUTANTS.json is a list of objects:

    {"file": "crates/game/src/misc.rs", "old": "...", "new": "...",
     "test": "jump_tables", "features": "", "filter": "", "timeout": 300,
     "why": "..."}

`file`, `test`, `features` and `filter` may instead be given once in a
top-level object: {"defaults": {...}, "mutants": [...]}. `old` must occur
exactly once in the file (a pattern that lands in another function would
look like a missed or a caught mutant), and `\\n` in patterns matches the
file's own line endings (CRLF or LF). Each mutant runs `cargo test -p
difftest --test TEST [--features F] [-- FILTER]` and counts as caught if the
tests fail, or if the run takes longer than `timeout` seconds (a mutant
that makes a loop endless). A build failure is reported on its own, not
counted as caught.

Afterwards the file is restored byte for byte, proptest-regressions files
the run created are deleted, and existing ones are restored. Exit status is
1 if any mutant was missed or didn't build.

Text editing only; it reads nothing ROM-derived.
"""
import glob
import json
import os
import subprocess
import sys
import time

REGRESSIONS = 'crates/*/tests/*.proptest-regressions'


def snapshot():
    return {p: open(p, 'rb').read() for p in glob.glob(REGRESSIONS)}


def restore(before):
    for p in glob.glob(REGRESSIONS):
        if p not in before:
            os.remove(p)
    for p, data in before.items():
        if open(p, 'rb').read() != data:
            open(p, 'wb').write(data)


def run_one(m, show):
    path = m['file']
    original = open(path, 'rb').read()
    text = original.decode('utf-8')
    crlf = '\r\n' in text
    old, new = m['old'], m['new']
    if crlf:
        old, new = old.replace('\n', '\r\n'), new.replace('\n', '\r\n')
    n = text.count(old)
    if n != 1:
        return f'BAD PATTERN ({n} matches)', 0.0
    before = snapshot()
    cmd = ['cargo', 'test', '-p', 'difftest', '--test', m['test']]
    if m.get('features'):
        cmd += ['--features', m['features']]
    if m.get('filter'):
        cmd += ['--', m['filter']]
    t = time.time()
    try:
        open(path, 'wb').write(text.replace(old, new).encode('utf-8'))
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, encoding='utf-8', errors='replace',
                               timeout=m.get('timeout', 300))
        except subprocess.TimeoutExpired:
            # subprocess.run kills cargo; the test binary it started may
            # outlive it, so kill that too.
            subprocess.run(['taskkill', '/F', '/T', '/IM', m['test'] + '-*'], capture_output=True)
            return 'caught/timeout', time.time() - t
    finally:
        open(path, 'wb').write(original)
        restore(before)
    dt = time.time() - t
    out = r.stdout + r.stderr
    if show:
        print('\n'.join(out.splitlines()[-25:]))
    if r.returncode == 0:
        return 'MISSED', dt
    if 'could not compile' in out:
        return 'BUILD ERROR', dt
    if 'test result:' not in out:
        # A panic in an `extern "C"` port, or a trap, aborts the test binary.
        return 'caught/abort', dt
    return 'caught', dt


def main():
    args = sys.argv[1:]
    show = '--show' in args
    args = [a for a in args if a != '--show']
    spec = json.load(open(args[0], encoding='utf-8'))
    only = {int(a) for a in args[1:]}
    defaults = {}
    if isinstance(spec, dict):
        defaults, spec = spec.get('defaults', {}), spec['mutants']
    bad = 0
    ran = 0
    for k, m in enumerate(spec):
        if only and k not in only:
            continue
        ran += 1
        m = {**defaults, **m}
        verdict, dt = run_one(m, show)
        bad += not verdict.startswith('caught')
        print(f'{k:3} {verdict:12} {dt:6.1f}s  {m.get("why", m["new"][:60])}', flush=True)
    print(f'{ran - bad}/{ran} caught')
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
