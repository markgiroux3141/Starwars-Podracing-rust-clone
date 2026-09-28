#!/usr/bin/env python3
"""Query the code segment: disassembly, callers/callees, address references.

Reads baserom.z64, symbols/racer.syms.toml, symbols/callgraph.csv and
symbols/functions.csv (names). Nothing is written; output is for reading.

  dis FUNC               disassemble a function (callees named, lui/addiu
                         pairs resolved to addresses)
  callers FUNC [-r N]    call sites of FUNC (recursively, N levels up)
  callees FUNC           what FUNC calls
  refs ADDR [ADDR2]      code that builds an address in [ADDR, ADDR2] with
                         lui + addiu/ori or lui + load/store offset
  pi                     code touching the PI registers (0xA4600000..)

FUNC is func_XXXXXXXX, a CSV name, or a hex address inside a function.
Usage: .venv/Scripts/python tools/xref.py <command> ...
"""

from __future__ import annotations

import csv
import re
import struct
import sys
from pathlib import Path

import rabbitizer
from rabbitizer import InstrId

ROOT = Path(__file__).resolve().parent.parent
TEXT_ROM, TEXT_VRAM = 0x1000, 0x80000400

rabbitizer.config.regNames_gprAbiNames = rabbitizer.Abi.O32
rabbitizer.config.misc_omit0XOnSmallImm = False


def sext16(x: int) -> int:
    return x - 0x10000 if x & 0x8000 else x


class Code:
    def __init__(self):
        self.rom = (ROOT / "baserom.z64").read_bytes()
        self.funcs: list[tuple[int, int]] = []
        for m in re.finditer(r"vram = 0x([0-9A-Fa-f]+), size = 0x([0-9A-Fa-f]+)",
                             (ROOT / "symbols/racer.syms.toml").read_text()):
            v, s = int(m[1], 16), int(m[2], 16)
            self.funcs.append((v, v + s))
        self.funcs.sort()
        self.names = {}
        with (ROOT / "symbols/functions.csv").open(newline="") as fh:
            for r in csv.DictReader(fh):
                self.names[int(r["vram"], 16)] = r["name"]
        self.edges = []
        with (ROOT / "symbols/callgraph.csv").open(newline="") as fh:
            for r in csv.DictReader(fh):
                self.edges.append((int(r["caller"], 16), int(r["site"], 16),
                                   int(r["callee"], 16) if r["callee"] else None, r["kind"]))

    def word(self, v: int) -> int:
        return struct.unpack_from(">I", self.rom, v - TEXT_VRAM + TEXT_ROM)[0]

    def name(self, v: int) -> str:
        return self.names.get(v, f"func_{v:08X}")

    def resolve(self, s: str) -> int:
        for v, n in self.names.items():
            if n == s:
                return v
        v = int(s.removeprefix("func_"), 16)
        for a, b in self.funcs:
            if a <= v < b:
                return a
        sys.exit(f"{s}: not in any function")

    def extent(self, start: int) -> tuple[int, int]:
        for a, b in self.funcs:
            if a == start:
                return a, b
        sys.exit(f"{start:#x}: not a function start")

    def containing(self, v: int) -> int | None:
        for a, b in self.funcs:
            if a <= v < b:
                return a
        return None

    def pairs(self, a: int, b: int):
        """Yield (vram, value, kind) for lui-based address constants in [a, b).
        Tracks one lui per register, forgetting it when the register is written."""
        hi: dict[int, int] = {}
        for v in range(a, b, 4):
            w = self.word(v)
            op, rs, rt, rd = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31
            imm = w & 0xFFFF
            if op == 0x0F:
                hi[rt] = imm << 16
                continue
            if rs in hi:
                if op == 0x09:
                    yield v, (hi[rs] + sext16(imm)) & 0xFFFFFFFF, "addiu"
                elif op == 0x0D:
                    yield v, (hi[rs] | imm) & 0xFFFFFFFF, "ori"
                elif op in (0x20, 0x21, 0x23, 0x24, 0x25, 0x27, 0x31, 0x35, 0x37):
                    yield v, (hi[rs] + sext16(imm)) & 0xFFFFFFFF, "load"
                elif op in (0x28, 0x29, 0x2B, 0x39, 0x3D, 0x3F):
                    yield v, (hi[rs] + sext16(imm)) & 0xFFFFFFFF, "store"
            ins = rabbitizer.Instruction(w, vram=v)
            if ins.modifiesRt():
                hi.pop(rt, None)
            if ins.modifiesRd():
                hi.pop(rd, None)


def cmd_dis(c: Code, args):
    start = c.resolve(args[0])
    a, b = c.extent(start)
    consts = {v: (val, k) for v, val, k in c.pairs(a, b)}
    print(f"{c.name(a)}: {a:#010x}-{b:#010x} ({b - a:#x} bytes)")
    for v in range(a, b, 4):
        w = c.word(v)
        ins = rabbitizer.Instruction(w, vram=v)
        text = ins.disassemble()
        note = ""
        if ins.uniqueId == InstrId.cpu_jal:
            note = c.name(ins.getInstrIndexAsVram())
        elif ins.uniqueId == InstrId.cpu_j:
            t = ins.getInstrIndexAsVram()
            note = c.name(t) if not (a <= t < b) else ""
        elif v in consts:
            val, k = consts[v]
            note = f"= {val:#010x}" if k in ("addiu", "ori") else f"[{val:#010x}]"
        print(f"  {v:08x}: {w:08x}  {text:<40} {('; ' + note) if note else ''}")


def cmd_callers(c: Code, args):
    start = c.resolve(args[0])
    depth = int(args[args.index("-r") + 1]) if "-r" in args else 1

    def up(t: int, level: int, seen: set[int]):
        for caller, site, callee, kind in c.edges:
            if callee == t:
                print(f"{'  ' * level}{c.name(caller)} at {site:#010x} ({kind})")
                if level < depth and caller not in seen:
                    up(caller, level + 1, seen | {caller})

    print(c.name(start))
    up(start, 1, {start})


def cmd_callees(c: Code, args):
    start = c.resolve(args[0])
    for caller, site, callee, kind in c.edges:
        if caller == start:
            print(f"  {site:#010x} {kind:<11} {c.name(callee) if callee is not None else '?'}")


def cmd_refs(c: Code, args):
    lo = int(args[0], 16)
    hi = int(args[1], 16) if len(args) > 1 else lo
    for a, b in c.funcs:
        for v, val, k in c.pairs(a, b):
            if lo <= val <= hi:
                print(f"  {v:#010x} in {c.name(a)}: {k} {val:#010x}")


def cmd_pi(c: Code, args):
    cmd_refs(c, ["0xA4600000", "0xA46FFFFF"])


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmds = {"dis": cmd_dis, "callers": cmd_callers, "callees": cmd_callees, "refs": cmd_refs, "pi": cmd_pi}
    f = cmds.get(sys.argv[1]) or sys.exit(__doc__)
    f(Code(), sys.argv[2:])


if __name__ == "__main__":
    main()
