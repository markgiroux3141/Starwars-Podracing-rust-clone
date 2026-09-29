"""Stage one group of ports when the working tree holds several (session 10).

    .venv/Scripts/python tools/stage_ports.py func_X...     (from the repo root)

For lib.rs (PORTED), crates/oracle/functions.txt and imports.rs, stages
the committed (index) version plus exactly the lines naming the given
functions (and, for imports, the callees they call). Other files are
staged by the caller (`git add`, or `git hash-object -w` plus `git
update-index --cacheinfo` for a module holding later groups too). Text
editing only.
"""
import re
import subprocess
import sys

names = set(sys.argv[1:])


def git(*a, inp=None):
    return subprocess.run(['git', *a], input=inp, capture_output=True, text=True, encoding='utf-8', check=True).stdout


def stage(path, text):
    h = git('hash-object', '-w', '--stdin', '--path', path, inp=text).strip()
    git('update-index', '--cacheinfo', f'100644,{h},{path}')


def merge_lines(path, pick, key):
    """The index version plus the working tree's lines picked by `pick`, kept sorted by `key`."""
    old = git('show', f':{path}')
    new = open(path, encoding='utf-8').read()
    old_lines = set(old.split('\n'))
    add = [l for l in new.split('\n') if l not in old_lines and pick(l)]
    return old, add


# lib.rs: PORTED entries (and the module line if collide is among them).
p = 'crates/game/src/lib.rs'
old, add = merge_lines(p, lambda l: any(f'"{n}"' in l for n in names) or l == 'pub mod collide;', None)
s = old
for l in add:
    if l == 'pub mod collide;':
        s = s.replace('pub mod channels;\n', 'pub mod channels;\npub mod collide;\n')
start = s.index('pub const PORTED: &[Ported] = &[\n') + len('pub const PORTED: &[Ported] = &[\n')
end = s.index('];', start)
lines = [l for l in s[start:end].splitlines() if l.strip()] + [l for l in add if 'Ported {' in l]
lines.sort(key=lambda l: int(re.search(r'vram: 0x([0-9A-F_]+)', l).group(1).replace('_', ''), 16))
stage(p, s[:start] + '\n'.join(lines) + '\n' + s[end:])

# functions.txt
p = 'crates/oracle/functions.txt'
old, add = merge_lines(p, lambda l: l.split(' ')[0] in names, None)
head, body = old.split('\n\n', 1)
entries = [l for l in body.splitlines() if l.strip()] + add
entries.sort(key=lambda l: int(l.split()[0][5:], 16))
stage(p, head + '\n\n' + '\n'.join(entries) + '\n')

# imports.rs: the callees the named ports call.
callees = set()
import glob
for f in glob.glob('crates/game/src/*.rs'):
    src = open(f, encoding='utf-8').read()
    for n in names:
        m = re.search(rf'pub unsafe extern "C" fn {n}\(.*?\n}}\n', src, re.S)
        if m:
            callees |= set(re.findall(r'imports::(func_[0-9A-F]{8})', m.group(0)))
p = 'crates/game/src/imports.rs'
old = git('show', f':{p}')
new = open(p, encoding='utf-8').read()
start = old.index('recomp_imports! {\n') + len('recomp_imports! {\n')
end = old.index('}\n', start)
have = [l for l in old[start:end].splitlines() if l.strip()]
have_names = {l.strip().split(',')[0] for l in have}
for l in new.splitlines():
    n = l.strip().split(',')[0]
    if n in callees and n not in have_names:
        have.append(l)
have.sort(key=lambda l: int(l.strip().split(',')[0][5:], 16))
stage(p, old[:start] + '\n'.join(have) + '\n' + old[end:])
print('staged; callees:', ' '.join(sorted(callees)))
