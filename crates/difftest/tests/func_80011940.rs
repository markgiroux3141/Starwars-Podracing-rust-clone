//! func_80011940 ("Comp"/"Wolf" LZSS decompressor): recompiled C vs
//! game::asset port, plus game::asset::lzss (the slice version) on the same
//! inputs.
//!
//! The real-data tests read the compressed models from baserom.z64 at test
//! time. Nothing ROM-derived is stored in the repository.

use difftest::{compare, Rng, State};
use game::asset::{func_80011940, lzss};
use game::recomp::reg::{A0, A1, AT, SP, V0};
use proptest::prelude::*;
use std::path::PathBuf;
use std::sync::OnceLock;

const NAME: &str = "func_80011940";
/// Input stream; the ring buffer is the 4 KB below it.
const SRC: u32 = 0x8060_0000;
const SRC_LEN: u32 = 0x8_0000;
/// Output area, well away from the input.
const DST: u32 = 0x8010_0000;
/// Stack: the function saves s0-s2 in the 16 bytes below it.
const STACK: u32 = 0x8000_F000;

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

#[derive(Clone, Debug)]
enum Token {
    Literal(u8),
    /// offset 1..=0xFFF, length field 0..=15 (copies field + 2 bytes)
    Ref(u16, u8),
}

/// Encode tokens as the game's format, then the terminator, which is placed
/// after `pad` more literals so that it can land on any flag bit.
fn encode(tokens: &[Token], pad: usize) -> Vec<u8> {
    encode_with_end(tokens, pad, 0)
}

/// As [`encode`], with the terminator's length nibble set to `end_len`: any
/// reference with offset 0 ends the stream, and the nibble is left in t1/t9.
fn encode_with_end(tokens: &[Token], pad: usize, end_len: u8) -> Vec<u8> {
    let mut all: Vec<Option<&Token>> = tokens.iter().map(Some).collect();
    let pads: Vec<Token> = (0..pad).map(|k| Token::Literal(k as u8)).collect();
    all.extend(pads.iter().map(Some));
    all.push(None); // terminator
    let mut out = Vec::new();
    for group in all.chunks(8) {
        let flag_at = out.len();
        out.push(0u8);
        for (bit, t) in group.iter().enumerate() {
            match t {
                Some(Token::Literal(b)) => {
                    out[flag_at] |= 1 << bit;
                    out.push(*b);
                }
                Some(Token::Ref(off, len)) => {
                    out.push((len << 4) | (off >> 8) as u8);
                    out.push(*off as u8);
                }
                None => out.extend([end_len << 4, 0]),
            }
        }
    }
    out
}

fn token() -> impl Strategy<Value = Token> {
    prop_oneof![
        any::<u8>().prop_map(Token::Literal),
        (1u16..=0xFFF, 0u8..=15).prop_map(|(o, l)| Token::Ref(o, l)),
        // Short distances back from the usual write positions, so references
        // hit bytes the stream itself wrote (and overlap the copy).
        (1u16..=24, 0u8..=15).prop_map(|(o, l)| Token::Ref(o, l)),
    ]
}

/// A state with random registers, a random window and input area, `stream`
/// at `src`, and the arguments set.
fn setup(seed: u64, src: u32, dst: u32, stream: &[u8]) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed.rotate_left(7), src - 0x1000, 0x1000 + SRC_LEN);
    s.rdram.mem().write_bytes(src, stream);
    s.ctx.gpr[A0] = sext(src);
    s.ctx.gpr[A1] = sext(dst);
    s.ctx.gpr[SP] = sext(STACK);
    s
}

fn run(s: &State) -> State {
    compare(NAME, func_80011940, s).unwrap_or_else(|d| panic!("{d}"))
}

fn read(s: &mut State, vaddr: u32, len: usize) -> Vec<u8> {
    let mut v = vec![0; len];
    s.rdram.mem().read_bytes(vaddr, &mut v);
    v
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Well-formed streams: the port matches the C, and the slice version
    /// produces the same bytes whenever it doesn't need the uninitialised window.
    #[test]
    fn token_streams(seed in any::<u64>(), tokens in prop::collection::vec(token(), 0..600), pad in 0usize..8, end_len in 0u8..16) {
        let stream = encode_with_end(&tokens, pad, end_len);
        let before = setup(seed, SRC, DST, &stream);
        let mut after = compare(NAME, func_80011940, &before).map_err(|d| TestCaseError::fail(d.to_string()))?;
        let end = after.ctx.gpr[V0] as u32;
        prop_assert!(end >= DST);
        match lzss::decompress(&stream) {
            Ok(d) => {
                prop_assert_eq!(d.consumed, stream.len());
                prop_assert_eq!(end - DST, d.data.len() as u32);
                prop_assert_eq!(read(&mut after, DST, d.data.len()), d.data);
            }
            Err(lzss::Error::UnwrittenWindow { .. }) => {}
            Err(e) => prop_assert!(false, "slice version: {e}"),
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Arbitrary bytes: the stream runs until a reference happens to have
    /// offset 0 (about one in 4096), reading the random window throughout.
    #[test]
    fn random_bytes(seed in any::<u64>()) {
        let mut rng = Rng::new(seed);
        let stream: Vec<u8> = (0..SRC_LEN - 0x100).map(|_| rng.next_u32() as u8).collect();
        let before = setup(seed, SRC, DST, &stream);
        compare(NAME, func_80011940, &before).map_err(|d| TestCaseError::fail(d.to_string()))?;
    }
}

#[test]
fn terminator_on_every_flag_bit() {
    for pad in 0..=16 {
        let s = setup(pad as u64, SRC, DST, &encode_with_end(&[], pad, pad as u8 % 16));
        let after = run(&s);
        assert_eq!(after.ctx.gpr[V0], sext(DST + pad as u32), "pad {pad}");
    }
}

#[test]
fn immediate_terminator_keeps_at_and_full_a1() {
    // No output: v0 is s0, which is a1 moved with a 64-bit `or`, and $at is
    // never written.
    let mut s = setup(1, SRC, DST, &[0x00, 0x00, 0x00]);
    s.ctx.gpr[AT] = 0x1234_5678_9ABC_DEF0;
    s.ctx.gpr[A1] = 0x0BAD_0000_8010_0000;
    let after = run(&s);
    assert_eq!(after.ctx.gpr[V0], 0x0BAD_0000_8010_0000);
    assert_eq!(after.ctx.gpr[AT], 0x1234_5678_9ABC_DEF0);
}

#[test]
fn upper_register_halves() {
    let stream = encode(&[Token::Literal(1), Token::Ref(1, 15), Token::Literal(2)], 3);
    for (k, (a0, a1, sp)) in [
        (0xFFFF_FFFF_8060_0000, 0xFFFF_FFFF_8010_0000, 0x0000_0000_8000_F000),
        // a0 and a1 are dereferenced as they are (lbu 0($a0), sb 0($s0)), so
        // they must be canonical: a non-canonical pointer is an address error
        // on hardware and outside the domain. sp only passes through ADD32
        // first, so its upper half is simply dropped.
        (0xFFFF_FFFF_8060_0000, 0xFFFF_FFFF_8010_0000, 0x3333_0000_8000_F000),
    ]
    .into_iter()
    .enumerate()
    {
        let mut s = setup(k as u64 + 10, SRC, DST, &stream);
        s.ctx.gpr[A0] = a0;
        s.ctx.gpr[A1] = a1;
        s.ctx.gpr[SP] = sp;
        s.ctx.gpr[16] = 0x4444_5555_6666_7777; // s0: restored sign-extended from its low word
        run(&s);
    }
}

#[test]
fn overlapping_layouts() {
    // Output over the window, over the input, and the stack inside the
    // window: C and Rust must agree access by access. Output that overtakes
    // unread input rewrites the stream; these layouts still terminate for
    // this fixed stream (a layout that ran away would do so on hardware too,
    // and is outside the domain).
    let mut rng = Rng::new(99);
    let tokens: Vec<Token> = (0..400)
        .map(|_| match rng.next_u32() % 3 {
            0 => Token::Literal(rng.next_u32() as u8),
            1 => Token::Ref(1 + (rng.next_u32() % 0xFFF) as u16, (rng.next_u32() % 16) as u8),
            _ => Token::Ref(1 + (rng.next_u32() % 20) as u16, (rng.next_u32() % 16) as u8),
        })
        .collect();
    let stream = encode(&tokens, 2);
    for (k, dst) in [SRC - 0x1000, SRC - 0x800, SRC - 0x10, SRC - 3, SRC + 0x40, SRC + 1].into_iter().enumerate() {
        run(&setup(k as u64, SRC, dst, &stream));
    }
    let mut s = setup(7, SRC, DST, &stream);
    s.ctx.gpr[SP] = sext(SRC - 0x100);
    run(&s);
}

/// baserom.z64 from the repo root. The oracle can't be built without it
/// (generated/ is ROM-derived), so its absence is an error, not a skip.
fn rom() -> &'static [u8] {
    static ROM: OnceLock<Vec<u8>> = OnceLock::new();
    ROM.get_or_init(|| {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baserom.z64");
        let r = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask verify-rom`", p.display()));
        assert_eq!(&r[..4], [0x80, 0x37, 0x12, 0x40], "baserom.z64 is not z64");
        assert_eq!(&r[0x20..0x33], b"STAR WARS EP1 RACER");
        r
    })
}

/// Every compressed model in the model block (ROM 0x0141E200, NOTES.md).
fn compressed_models() -> Vec<(usize, &'static [u8])> {
    const BLOCK: usize = 0x0141_E200;
    let rom = rom();
    let u32_at = |o: usize| u32::from_be_bytes(rom[o..o + 4].try_into().unwrap()) as usize;
    let count = u32_at(BLOCK);
    (0..count)
        .filter_map(|i| {
            let (model, next) = (u32_at(BLOCK + 8 + 8 * i), u32_at(BLOCK + 12 + 8 * i));
            let bytes = &rom[BLOCK + model..BLOCK + next];
            bytes.starts_with(b"Comp").then_some((i, bytes))
        })
        .collect()
}

#[test]
fn real_compressed_models() {
    let models = compressed_models();
    assert_eq!(models.len(), 92, "compressed models in the USA ROM");
    for (i, block) in models {
        let comp = lzss::parse_comp(block).unwrap();
        assert_eq!(&comp.tag, b"Wolf", "model {i}");
        let size = comp.size;

        // The game's layout: stream at the top of the heap, output at the cursor.
        let mut after = run(&setup(i as u64, SRC, DST, comp.stream));
        assert_eq!(after.ctx.gpr[V0], sext(DST + size), "model {i}: end of output");
        let c_out = read(&mut after, DST, size as usize);
        let slice = lzss::decompress(comp.stream).unwrap_or_else(|e| panic!("model {i}: {e}"));
        assert_eq!(slice.data, c_out, "model {i}: slice version vs C");
        assert!(
            [b"Modl", b"Trak", b"Podd", b"Part", b"Scen", b"MAlt", b"Pupp"].iter().any(|t| c_out.starts_with(*t)),
            "model {i}: tag {:?}",
            &c_out[..4]
        );

        // A tight heap, as when memory runs short: the output runs into the
        // window below the stream. Results may be garbage, but C and Rust
        // must produce the same garbage.
        let dst = (SRC - 0x1000).wrapping_sub(size) + 0x800;
        run(&setup(i as u64 ^ 0xFF, SRC, dst & !7, comp.stream));
    }
}
