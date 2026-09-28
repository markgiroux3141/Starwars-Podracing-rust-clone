#!/usr/bin/env python3
"""Find function boundaries in the main code segment and write the N64Recomp
symbol file (symbols/racer.syms.toml).

Everything here comes from our own analysis of the user's ROM (see NOTES.md,
"Decision: build our own symbol file"). Published facts from other projects
are used only as a cross-check, never as input.

Method
------
1. The boot routine tells us how big the loaded image is (ROM 0x1000 up to the
   end of the self-copy at 0x80000450).
2. Recursive descent from the entrypoint. Each function is traversed along its
   control flow: conditional branches, `b`/`j`, jump tables (recognised with
   the same lui/addiu/addu/lw pattern N64Recomp uses), `jal` targets (new
   functions), `j`/branches to another function's start (tail calls) and
   indirect `jr` (indirect tail calls).
3. Function-pointer candidates are then collected from lui/addiu (lui/ori)
   pairs in code and from words in the data part of the image. Candidates
   that are not inside an existing function become new functions.
4. Any remaining non-padding words between functions are traversed as
   functions only reachable through pointers we could not resolve.
5. Extents are recomputed with the final set of starts until nothing changes.

A function's size runs from its start to just after the last instruction
reachable from it (the delay slot of its last return or tail call). If control
falls through into another function's start, the first function's extent
covers the second as well (N64Recomp emits a C function per symbol, so a
function that falls off its end must contain its continuation). These overlaps
are reported.

Usage: .venv/Scripts/python tools/find_functions.py [--rom baserom.z64]
       [--out symbols/racer.syms.toml] [--csv symbols/functions.csv]
"""

from __future__ import annotations

import argparse
import csv
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path

import rabbitizer
from rabbitizer import InstrId

ROOT = Path(__file__).resolve().parent.parent

TEXT_ROM = 0x1000
TEXT_VRAM = 0x80000400
ENTRYPOINT = 0x80000400
# IPL3 copies 1 MB from ROM 0x1000 to the entrypoint; that is more than the image.
IPL3_LOAD_SIZE = 0x100000

# Instructions N64Recomp (pinned ffb39cd) cannot translate: they are absent from
# operations.cpp and recompilation.cpp, which then reject the function. Functions
# containing them must be supplied by the runtime.
UNSUPPORTED = {
    InstrId.cpu_cache, InstrId.cpu_eret, InstrId.cpu_tlbp, InstrId.cpu_tlbr,
    InstrId.cpu_tlbwi, InstrId.cpu_tlbwr, InstrId.cpu_dmfc0, InstrId.cpu_dmtc0,
    InstrId.cpu_sync,
    # 64-bit float->int conversions other than cvt.l.*
    InstrId.cpu_trunc_l_s, InstrId.cpu_trunc_l_d, InstrId.cpu_round_l_s, InstrId.cpu_round_l_d,
    InstrId.cpu_ceil_l_s, InstrId.cpu_ceil_l_d, InstrId.cpu_floor_l_s, InstrId.cpu_floor_l_d,
}
COP0_STATUS = 12
MIPS_COP2_OPCODES = {0x12, 0x32, 0x3A, 0x36, 0x3E}  # cop2, lwc2, swc2, ldc2, sdc2


def sext16(x: int) -> int:
    return x - 0x10000 if x & 0x8000 else x


@dataclass
class JumpTable:
    jr_vram: int
    table_vram: int
    entries: list[int]
    bound: int | None  # from `sltiu`, if found


@dataclass
class Func:
    start: int
    end: int = 0  # exclusive
    visited: set[int] = field(default_factory=set)
    calls: set[int] = field(default_factory=set)
    tail_calls: set[int] = field(default_factory=set)
    fallthrough_into: set[int] = field(default_factory=set)
    back_branch_seeds: set[int] = field(default_factory=set)
    jump_tables: list[JumpTable] = field(default_factory=list)
    indirect_jumps: list[int] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    unsupported: list[str] = field(default_factory=list)
    uses_float: bool = False
    has_calls: bool = False


class Image:
    def __init__(self, rom: bytes):
        self.rom = rom
        self.image_end = self._image_end_from_boot()
        self._cache: dict[int, rabbitizer.Instruction] = {}

    def word(self, vram: int) -> int:
        r = vram - TEXT_VRAM + TEXT_ROM
        return struct.unpack_from(">I", self.rom, r)[0]

    def ins(self, vram: int) -> rabbitizer.Instruction:
        i = self._cache.get(vram)
        if i is None:
            i = rabbitizer.Instruction(self.word(vram), vram=vram)
            self._cache[vram] = i
        return i

    def _image_end_from_boot(self) -> int:
        """The boot routine at 0x80000450 copies [a1, t7) of ROM to KSEG0 and
        clears the rest up to 4 MB. Decode its first four instructions."""
        seq = [self.word(0x80000450 + 4 * k) for k in range(4)]
        ops = [(w >> 26, (w >> 16) & 31, w & 0xFFFF) for w in seq]
        # lui a1; addiu a1,a1; lui t7; addiu t7,t7
        expect = [(0x0F, 5), (0x09, 5), (0x0F, 15), (0x09, 15)]
        if [(o, rt) for o, rt, _ in ops] != expect:
            sys.exit("boot routine at 0x80000450 does not have the expected shape; is this the right ROM?")
        a1 = ((ops[0][2] << 16) + sext16(ops[1][2])) & 0xFFFFFFFF
        t7 = ((ops[2][2] << 16) + sext16(ops[3][2])) & 0xFFFFFFFF
        self.boot_copy_rom = (a1, t7)
        end_rom = t7
        if end_rom - TEXT_ROM > IPL3_LOAD_SIZE:
            sys.exit(f"image end {end_rom:#x} exceeds what IPL3 loads")
        return end_rom - TEXT_ROM + TEXT_VRAM


class Analyzer:
    def __init__(self, img: Image):
        self.img = img
        # Upper bound until the call graph gives the real end (see run()).
        self.first_cop2 = self._first_non_cpu_word()
        self.text_end = self.first_cop2
        self.starts: dict[int, str] = {ENTRYPOINT: "entry"}
        self.funcs: dict[int, Func] = {}
        self.ptr_candidates_code: dict[int, set[int]] = {}  # target -> referencing vrams
        self.ptr_candidates_data: dict[int, set[int]] = {}
        self.notes: list[str] = []

    # ---- region helpers -------------------------------------------------

    def _first_non_cpu_word(self) -> int:
        """Upper bound for .text: the first COP2 / lwc2 / swc2 word. The VR4300
        has no COP2, so these only occur in RSP microcode stored as data."""
        v = TEXT_VRAM
        while v < self.img.image_end:
            if (self.img.word(v) >> 26) in MIPS_COP2_OPCODES:
                return v
            v += 4
        return self.img.image_end

    def in_text(self, v: int) -> bool:
        return TEXT_VRAM <= v < self.text_end and v % 4 == 0

    # ---- jump tables (mirrors N64Recomp's analysis.cpp register tracker) ---

    def find_jump_table(self, start: int, jr_vram: int) -> tuple[int, int | None] | None:
        """Linear scan from the function start to the `jr`, tracking lui/addiu/
        addu/lw the way N64Recomp does. Returns (table vram, sltiu bound)."""
        EMPTY = {}
        st: list[dict] = [dict() for _ in range(32)]
        stack: dict[int, dict] = {}
        bound_by_reg: dict[int, int] = {}
        last_bound = None
        v = start
        while v < jr_vram:
            w = self.img.word(v)
            i = self.img.ins(v)
            uid = i.uniqueId
            rs, rt, rd = (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31
            imm = w & 0xFFFF
            if uid == InstrId.cpu_lui:
                st[rt] = {"lui": (imm << 16) & 0xFFFFFFFF}
            elif uid == InstrId.cpu_addiu:
                s = dict(st[rs])
                if "addiu" not in s:
                    s["addiu"] = sext16(imm)
                else:
                    s = dict()
                st[rt] = s
            elif uid in (InstrId.cpu_addu, InstrId.cpu_add):
                a, b = st[rs], st[rt]
                if ("lui" in a) != ("lui" in b):
                    lui_reg, addend = (rs, rt) if "lui" in a else (rt, rs)
                    s = dict(st[lui_reg])
                    s["addend"] = addend
                elif rs == 0:
                    s = dict(st[rt])
                elif rt == 0:
                    s = dict(st[rs])
                else:
                    s = dict()
                if rd:
                    st[rd] = s
            elif uid in (InstrId.cpu_or, InstrId.cpu_daddu):
                s = dict(st[rt]) if rs == 0 else dict(st[rs]) if rt == 0 else dict()
                if rd:
                    st[rd] = s
            elif uid == InstrId.cpu_sw and rs == 29:
                stack[sext16(imm)] = dict(st[rt])
            elif uid == InstrId.cpu_lw:
                s = dict()
                if rs == 29:
                    s = dict(stack.get(sext16(imm), EMPTY))
                elif "lui" in st[rs] and "addend" in st[rs]:
                    b = st[rs]
                    if not (imm != 0 and "addiu" in b):
                        lo = sext16(imm) if imm != 0 else b.get("addiu", 0)
                        s = {"loaded": (b["lui"] + lo) & 0xFFFFFFFF}
                st[rt] = s
            elif uid == InstrId.cpu_sltiu:
                last_bound = sext16(imm) & 0xFFFFFFFF
                if rt:
                    st[rt] = {}
            else:
                if i.modifiesRd() and rd:
                    st[rd] = {}
                if i.modifiesRt() and rt:
                    st[rt] = {}
            v += 4
        w = self.img.word(jr_vram)
        rs = (w >> 21) & 31
        if "loaded" in st[rs]:
            return st[rs]["loaded"], last_bound
        return None

    def read_table(self, table: int, lo: int, hi: int, limit: int | None) -> list[int]:
        out = []
        a = table
        while TEXT_VRAM <= a < self.img.image_end and (limit is None or len(out) < limit):
            e = self.img.word(a)
            if not (lo <= e < hi and e % 4 == 0):
                break
            out.append(e)
            a += 4
        return out

    # ---- traversal --------------------------------------------------------

    def traverse(self, start: int) -> Func:
        f = Func(start)
        work = [start]
        img = self.img

        def visit(pc: int) -> rabbitizer.Instruction | None:
            if not self.in_text(pc):
                f.errors.append(f"control reaches {pc:#010x}, outside .text")
                return None
            i = img.ins(pc)
            f.visited.add(pc)
            if not i.isValid():
                f.errors.append(f"invalid instruction {img.word(pc):08X} at {pc:#010x}")
                return None
            uid = i.uniqueId
            w = img.word(pc)
            if uid in UNSUPPORTED:
                f.unsupported.append(f"{pc:#010x} {i.getOpcodeName()}")
            elif uid in (InstrId.cpu_mfc0, InstrId.cpu_mtc0) and ((w >> 11) & 31) != COP0_STATUS:
                f.unsupported.append(f"{pc:#010x} {i.getOpcodeName()} cop0 r{(w >> 11) & 31}")
            elif uid in (InstrId.cpu_cfc1, InstrId.cpu_ctc1) and ((w >> 11) & 31) != 31:
                f.unsupported.append(f"{pc:#010x} {i.getOpcodeName()} fcr{(w >> 11) & 31}")
            elif uid == InstrId.cpu_jalr and ((w >> 11) & 31) != 31:
                f.unsupported.append(f"{pc:#010x} jalr with rd != $ra")
            if i.isFloat() or (w >> 26) in (0x11, 0x31, 0x35, 0x39, 0x3D):
                f.uses_float = True
            return i

        while work:
            pc = work.pop()
            while True:
                if pc in f.visited:
                    break
                if pc != start and pc in self.starts:
                    # Sequential flow into another function's start.
                    f.fallthrough_into.add(pc)
                i = visit(pc)
                if i is None:
                    break
                uid = i.uniqueId
                w = img.word(pc)

                def delay():
                    d = visit(pc + 4)
                    if d is not None and d.hasDelaySlot():
                        f.errors.append(f"branch in delay slot at {pc + 4:#010x}")

                if uid == InstrId.cpu_eret:
                    break
                if uid == InstrId.cpu_jal:
                    t = i.getInstrIndexAsVram()
                    f.has_calls = True
                    delay()
                    if self.in_text(t):
                        f.calls.add(t)
                    else:
                        f.errors.append(f"jal to {t:#010x} outside .text at {pc:#010x}")
                    pc += 8
                    continue
                if uid == InstrId.cpu_jalr:
                    f.has_calls = True
                    delay()
                    pc += 8
                    continue
                if uid == InstrId.cpu_jr:
                    delay()
                    rs = (w >> 21) & 31
                    if rs == 31:
                        break
                    jt = self.find_jump_table(start, pc)
                    if jt is None:
                        f.indirect_jumps.append(pc)
                        break
                    table, bound = jt
                    # With an sltiu bound the count is known; without one, stop
                    # at the next known function start.
                    hi = self.text_end if bound is not None else self.next_start_after(start)
                    entries = self.read_table(table, start, hi, bound)
                    if bound is not None and len(entries) != bound:
                        f.errors.append(
                            f"jump table {table:#010x} at {pc:#010x}: sltiu bound {bound} but only {len(entries)} in-range entries")
                    if not entries:
                        f.errors.append(f"jump table {table:#010x} at {pc:#010x} has no entries")
                    f.jump_tables.append(JumpTable(pc, table, entries, bound))
                    work.extend(entries)
                    break
                if uid == InstrId.cpu_j:
                    t = i.getInstrIndexAsVram()
                    delay()
                    if t == start:
                        work.append(t)  # loop back to own start
                    elif t in self.starts or t < start:
                        f.tail_calls.add(t)
                    else:
                        work.append(t)
                    break
                if i.isBranch():
                    t = i.getBranchVramGeneric()
                    links = i.doesLink()
                    delay()
                    if links:
                        f.has_calls = True
                        f.calls.add(t)
                    elif t != start and t in self.starts:
                        f.tail_calls.add(t)
                    elif t < start:
                        # Branch backwards out of the function: the target must be
                        # a function start for N64Recomp to treat it as a tail call.
                        f.back_branch_seeds.add(t)
                        f.tail_calls.add(t)
                    else:
                        work.append(t)
                    if i.isUnconditionalBranch() and not links:
                        break
                    pc += 8
                    continue
                if uid == InstrId.cpu_syscall:
                    break
                pc += 4
        f.end = max(f.visited) + 4 if f.visited else start
        return f

    def next_start_after(self, v: int) -> int:
        later = [s for s in self.starts if s > v]
        return min(later) if later else self.text_end

    # ---- pointer candidates -------------------------------------------------

    def scan_code_pointers(self, f: Func):
        """lui/addiu and lui/ori pairs producing a .text address."""
        hi: dict[int, int] = {}
        for v in range(f.start, f.end, 4):
            if v not in f.visited:
                hi.clear()
                continue
            w = self.img.word(v)
            op, rs, rt, rd = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31
            imm = w & 0xFFFF
            i = self.img.ins(v)
            if op == 0x0F:  # lui
                hi[rt] = imm << 16
                continue
            if op in (0x09, 0x0D) and rs in hi:  # addiu / ori
                val = (hi[rs] + (sext16(imm) if op == 0x09 else imm)) & 0xFFFFFFFF
                if self.in_text(val):
                    self.ptr_candidates_code.setdefault(val, set()).add(v)
            if i.modifiesRt():
                hi.pop(rt, None)
            if i.modifiesRd():
                hi.pop(rd, None)

    def scan_data_pointers(self):
        for a in range(self.text_end, self.img.image_end, 4):
            w = self.img.word(a)
            if self.in_text(w):
                self.ptr_candidates_data.setdefault(w, set()).add(a)

    # ---- driver ---------------------------------------------------------------

    def covering(self, v: int) -> list[Func]:
        return [f for f in self.funcs.values() if f.start <= v < f.end and v in f.visited]

    def descend(self):
        """Traverse every known start, adding call/tail-call targets, until closed."""
        work = [s for s in self.starts if s not in self.funcs]
        while work:
            s = work.pop()
            if s in self.funcs:
                continue
            f = self.traverse(s)
            self.funcs[s] = f
            for t in f.calls:
                if t not in self.starts:
                    self.starts[t] = "jal"
                    work.append(t)
            for t in f.tail_calls:
                if t not in self.starts:
                    self.starts[t] = "tail-branch" if t in f.back_branch_seeds else "tail-j"
                    work.append(t)

    def retraverse_all(self):
        """Recompute every extent with the current start set until stable."""
        for _ in range(20):
            before = {s: (f.end, len(f.visited)) for s, f in self.funcs.items()}
            self.funcs = {}
            self.descend()
            after = {s: (f.end, len(f.visited)) for s, f in self.funcs.items()}
            if before == after:
                return
        sys.exit("extents did not converge")

    def is_padding(self, v: int) -> bool:
        return self.img.word(v) == 0

    def uncovered_runs(self) -> list[tuple[int, int]]:
        covered = set()
        for f in self.funcs.values():
            covered.update(range(f.start, f.end, 4))
        runs, v = [], TEXT_VRAM
        while v < self.text_end:
            if v in covered or self.is_padding(v):
                v += 4
                continue
            s = v
            while v < self.text_end and v not in covered and not self.is_padding(v):
                v += 4
            runs.append((s, v))
        return runs

    def plausible_start(self, v: int) -> bool:
        if not self.in_text(v) or self.is_padding(v):
            return False
        f = self.traverse(v)
        return not f.errors

    def add_start(self, t: int, src: str) -> bool:
        """Accept one candidate start and traverse it (and its callees) at once,
        so its own branch targets are known before the next candidate is judged."""
        if t in self.starts or self.covering(t) or not self.plausible_start(t):
            return False
        self.starts[t] = src
        self.descend()
        return True

    def code_pointer_round(self) -> bool:
        self.ptr_candidates_code.clear()
        for f in list(self.funcs.values()):
            self.scan_code_pointers(f)
        added = False
        for t in sorted(self.ptr_candidates_code):
            added |= self.add_start(t, "ptr-code")
        return added

    def coverage(self) -> bytearray:
        cov = bytearray((self.text_end - TEXT_VRAM) // 4)
        for f in self.funcs.values():
            a, b = (f.start - TEXT_VRAM) // 4, (f.end - TEXT_VRAM) // 4
            cov[a:b] = b"\x01" * (b - a)
        return cov

    def gap_round(self) -> bool:
        """Code nothing points at. Walk forward: the first non-padding word not
        inside any function starts one (functions are laid out back to back,
        objects padded to 16 bytes). Coverage is recomputed after each new
        function, since nops inside its body would otherwise look like gaps.
        A run whose first word does not begin valid code is data; skip all of it."""
        added = False
        cov = self.coverage()
        v = TEXT_VRAM
        while v < self.text_end:
            k = (v - TEXT_VRAM) // 4
            if cov[k] or self.is_padding(v):
                v += 4
                continue
            if self.add_start(v, "gap"):
                added = True
                cov = self.coverage()
                continue
            while v < self.text_end and not cov[(v - TEXT_VRAM) // 4] and not self.is_padding(v):
                v += 4
        return added

    def run(self):
        # 1. Direct calls and tail calls from the entrypoint, plus addresses
        #    built in code (the entrypoint reaches the boot routine via `jr $t2`).
        self.descend()
        while self.code_pointer_round():
            pass
        self.retraverse_all()
        # .text ends after the last function reached this way. What follows up
        # to the first COP2 word is RSP code (rspboot) that happens to decode as
        # CPU code, so it must not be offered to the gap search.
        self.text_end = (max(f.end for f in self.funcs.values()) + 15) & ~15
        # 2./3. Addresses built in code, then code nothing references directly.
        for _ in range(50):
            progressed = self.code_pointer_round()
            progressed |= self.gap_round()
            if not progressed:
                break
            self.retraverse_all()
        # 4. Words in data. By now almost every one is a known start or a
        #    jump-table target; the rest are judged individually.
        self.scan_data_pointers()
        in_tables = {a for f in self.funcs.values() for j in f.jump_tables
                     for a in range(j.table_vram, j.table_vram + 4 * len(j.entries), 4)}
        for t in sorted(self.ptr_candidates_data):
            if self.ptr_candidates_data[t] <= in_tables:
                continue
            self.add_start(t, "ptr-data")
        self.retraverse_all()
        # Whatever is still uncovered is data inside .text.
        self.notes = [f"uncovered words {s:#010x}-{e:#010x} ({(e - s) // 4} words), not valid code"
                      for s, e in self.uncovered_runs()]

        last_end = max(f.end for f in self.funcs.values())
        self.text_end_aligned = (last_end + 15) & ~15

    # ---- checks N64Recomp will make -------------------------------------------

    def recomp_jump_table_check(self) -> list[str]:
        """Re-derive every jump table with N64Recomp's rule (entries must lie in
        [start, end), stop at the next table of the same function)."""
        issues = []
        for f in self.funcs.values():
            tables = sorted(f.jump_tables, key=lambda j: j.table_vram)
            for k, jt in enumerate(tables):
                stop = tables[k + 1].table_vram if k + 1 < len(tables) else None
                out, a = [], jt.table_vram
                while stop is None or a < stop:
                    e = self.img.word(a)
                    if not (f.start <= e < f.end):
                        break
                    out.append(e)
                    a += 4
                if out[: len(jt.entries)] != jt.entries:
                    issues.append(f"func_{f.start:08X}: N64Recomp would read table {jt.table_vram:#010x} as {len(out)} entries, "
                                  f"not the {len(jt.entries)} we traversed")
                elif len(out) != len(jt.entries):
                    extra = out[len(jt.entries):]
                    issues.append(f"func_{f.start:08X}: table {jt.table_vram:#010x} bound {len(jt.entries)}, N64Recomp reads "
                                  f"{len(extra)} extra in-range word(s) {', '.join(f'{x:#010x}' for x in extra)} (harmless extra case labels)")
        return issues


def write_syms(path: Path, a: Analyzer, sha1: str):
    funcs = sorted(a.funcs.values(), key=lambda f: f.start)
    size = a.text_end_aligned - TEXT_VRAM
    lines = [
        "# N64Recomp symbol file for Star Wars Episode I: Racer (N64, USA).",
        "# Generated by tools/find_functions.py from our own analysis of the ROM; do not edit by hand.",
        f"# Functions: {len(funcs)}",
        "",
        "[[section]]",
        'name = ".text"',
        f"rom = 0x{TEXT_ROM:X}",
        f"vram = 0x{TEXT_VRAM:08X}",
        f"size = 0x{size:X}",
        "",
        "functions = [",
    ]
    for f in funcs:
        lines.append(f'    {{ name = "func_{f.start:08X}", vram = 0x{f.start:08X}, size = 0x{f.end - f.start:X} }},')
    lines.append("]")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def write_csv(path: Path, a: Analyzer):
    """SPEC §7 table. Keeps name/status/etc. of rows that already exist."""
    cols = ["vram", "name", "name_source", "depth", "subsystem", "status", "test_kind", "confidence", "notes"]
    old: dict[str, dict] = {}
    if path.exists():
        with path.open(newline="", encoding="utf-8") as fh:
            for row in csv.DictReader(fh):
                old[row["vram"]] = row
    funcs = sorted(a.funcs.values(), key=lambda f: f.start)
    depth = compute_depths(a)
    with path.open("w", newline="", encoding="utf-8") as fh:
        wr = csv.DictWriter(fh, fieldnames=cols, lineterminator="\n")
        wr.writeheader()
        for f in funcs:
            key = f"0x{f.start:08X}"
            row = old.get(key) or {
                "vram": key, "name": f"func_{f.start:08X}", "name_source": "ours", "depth": "",
                "subsystem": "", "status": "recomp", "test_kind": "", "confidence": "", "notes": "",
            }
            row["depth"] = "" if depth.get(f.start) is None else str(depth[f.start])
            if not old.get(key):
                notes = []
                if f.start >= LIBULTRA_GUESS:
                    notes.append("libultra range?")
                if f.unsupported:
                    notes.append("not recompilable; runtime-provided")
                if f.fallthrough_into:
                    notes.append("falls through into " + " ".join(f"func_{t:08X}" for t in sorted(f.fallthrough_into)))
                if f.indirect_jumps:
                    notes.append("indirect jr")
                row["notes"] = "; ".join(notes)
            wr.writerow({c: row.get(c, "") for c in cols})


# Cross-check fact (not input): libultra reportedly starts around here.
LIBULTRA_GUESS = 0x8008C000


def compute_depths(a: Analyzer) -> dict[int, int | None]:
    """Direct-call depth from leaves (calls + tail calls). Functions with
    indirect calls or on a cycle get None."""
    memo: dict[int, int | None] = {}
    onstack: set[int] = set()

    def d(s: int) -> int | None:
        if s in memo:
            return memo[s]
        if s in onstack:
            return None
        f = a.funcs.get(s)
        if f is None:
            return None
        onstack.add(s)
        callees = (f.calls | f.tail_calls | f.fallthrough_into) - {s}
        has_indirect = bool(f.indirect_jumps) or any(
            a.img.ins(v).uniqueId == InstrId.cpu_jalr for v in f.visited)
        res: int | None = 0
        if has_indirect:
            res = None
        for c in callees:
            cd = d(c)
            if cd is None or res is None:
                res = None
            else:
                res = max(res, cd + 1)
        onstack.discard(s)
        memo[s] = res
        return res

    sys.setrecursionlimit(10000)
    for s in a.funcs:
        d(s)
    return memo


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rom", default=ROOT / "baserom.z64", type=Path)
    ap.add_argument("--out", default=ROOT / "symbols" / "racer.syms.toml", type=Path)
    ap.add_argument("--csv", default=ROOT / "symbols" / "functions.csv", type=Path)
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args()

    rom = args.rom.read_bytes()
    if rom[:4] != b"\x80\x37\x12\x40":
        sys.exit(f"{args.rom} is not a z64 ROM; run `cargo xtask verify-rom` first")
    import hashlib
    sha1 = hashlib.sha1(rom).hexdigest()
    expected = ROOT / "rom" / "EXPECTED.sha1"
    if expected.exists() and expected.read_text().strip() != sha1:
        sys.exit("ROM SHA-1 differs from rom/EXPECTED.sha1")

    img = Image(rom)
    a = Analyzer(img)
    a.run()

    funcs = sorted(a.funcs.values(), key=lambda f: f.start)
    print(f"image:            ROM {TEXT_ROM:#x}-{img.image_end - TEXT_VRAM + TEXT_ROM:#x}, "
          f"VRAM {TEXT_VRAM:#010x}-{img.image_end:#010x} (boot copies ROM {img.boot_copy_rom[0]:#x}-{img.boot_copy_rom[1]:#x})")
    print(f"first COP2 word:  {a.first_cop2:#010x} (ROM {a.first_cop2 - TEXT_VRAM + TEXT_ROM:#x}); "
          f"{a.first_cop2 - a.text_end_aligned:#x} bytes of RSP boot code before it")
    print(f".text end:        {a.text_end_aligned:#010x} (ROM {a.text_end_aligned - TEXT_VRAM + TEXT_ROM:#x}), "
          f"size {a.text_end_aligned - TEXT_VRAM:#x}")
    print(f"functions:        {len(funcs)}")
    by_src: dict[str, int] = {}
    for s in a.funcs:
        by_src[a.starts[s]] = by_src.get(a.starts[s], 0) + 1
    print("  by discovery:   " + ", ".join(f"{k} {v}" for k, v in sorted(by_src.items())))
    lib = sum(1 for f in funcs if f.start >= LIBULTRA_GUESS)
    print(f"  at/after {LIBULTRA_GUESS:#x}: {lib}")
    leaves = [f for f in funcs if not f.has_calls and not f.tail_calls and not f.indirect_jumps and not f.fallthrough_into]
    print(f"  leaves (no calls/tail calls/indirect jumps): {len(leaves)}, integer-only: "
          f"{sum(1 for f in leaves if not f.uses_float)}")

    errs = [(f, e) for f in funcs for e in f.errors]
    print(f"\nerrors: {len(errs)}")
    for f, e in errs:
        print(f"  func_{f.start:08X}: {e}")

    overl = [(f, t) for f in funcs for t in sorted(f.fallthrough_into)]
    print(f"\nfallthroughs (extent covers the next function's start): {len(overl)}")
    for f, t in overl:
        print(f"  func_{f.start:08X} [{f.start:#010x}-{f.end:#010x}) falls into func_{t:08X}")

    overlaps = []
    for f in funcs:
        for g in funcs:
            if g.start > f.start and g.start < f.end and g.start not in f.fallthrough_into:
                overlaps.append((f, g))
    print(f"\nother overlaps (start inside another function's extent): {len(overlaps)}")
    for f, g in overlaps:
        how = a.starts[g.start]
        print(f"  func_{g.start:08X} ({how}) lies inside func_{f.start:08X} [{f.start:#010x}-{f.end:#010x})")

    bb = [(f, t) for f in funcs for t in sorted(f.back_branch_seeds)]
    print(f"\nbranches out of a function to a non-start (made into starts): {len(bb)}")
    for f, t in bb:
        print(f"  func_{f.start:08X} -> {t:#010x}")

    unsup = [f for f in funcs if f.unsupported]
    print(f"\nfunctions N64Recomp cannot translate (privileged or unsupported instructions): {len(unsup)}")
    for f in unsup:
        print(f"  func_{f.start:08X}: {', '.join(f.unsupported[:4])}{' ...' if len(f.unsupported) > 4 else ''}")

    jts = [j for f in funcs for j in f.jump_tables]
    print(f"\njump tables: {len(jts)} ({sum(1 for j in jts if j.bound is None)} without an sltiu bound)")
    for msg in a.recomp_jump_table_check():
        print("  " + msg)

    ind = [(f, v) for f in funcs for v in f.indirect_jumps]
    print(f"\nindirect jr (not a jump table, treated as tail call): {len(ind)}")
    for f, v in ind:
        print(f"  func_{f.start:08X} at {v:#010x}")

    amb = []
    for src, cands in (("code", a.ptr_candidates_code), ("data", a.ptr_candidates_data)):
        for t, refs in sorted(cands.items()):
            if t in a.starts:
                continue
            jt_entry = any(t in j.entries for f in funcs for j in f.jump_tables)
            if jt_entry:
                continue
            amb.append((src, t, refs))
    print(f"\npointer candidates into .text that are neither a start nor a jump-table target: {len(amb)}")
    for src, t, refs in amb[: 60 if not args.verbose else None]:
        inside = ", ".join(f"func_{f.start:08X}" for f in a.covering(t)) or "no function"
        print(f"  {t:#010x} from {src} at {', '.join(f'{r:#010x}' for r in sorted(refs)[:3])} (inside {inside})")
    if len(amb) > 60 and not args.verbose:
        print(f"  ... {len(amb) - 60} more (-v)")

    print(f"\ndata inside .text / unreached words: {len(a.notes)}")
    for n in a.notes:
        print("  " + n)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    write_syms(args.out, a, sha1)
    print(f"\nwrote {args.out}")
    if args.csv:
        write_csv(args.csv, a)
        print(f"wrote {args.csv}")
    return 1 if errs else 0


if __name__ == "__main__":
    sys.exit(main())
