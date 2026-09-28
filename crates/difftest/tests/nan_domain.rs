//! Where float ports' domains end: N64Recomp's generated C asserts that
//! operands of arithmetic are not NaN (`NAN_CHECK`, live in the oracle), so a
//! NaN operand stops the C instead of producing a result to compare with.
//! This pins that boundary in a child process, for one port (vec2_length),
//! so "NaN operands are outside the domain" is a checked fact, not a guess
//! (NOTES.md, "Floats").

use difftest::State;
use game::math;
use game::recomp::reg::*;
use std::process::Command;

const CHILD_ENV: &str = "NAN_DOMAIN_CHILD";
const VEC: u32 = 0x8030_0000;

fn nan_vector() -> State {
    let mut s = State::new();
    let mut m = s.rdram.mem();
    m.write_u32(VEC, 0x7FC0_1234); // a quiet NaN with a payload
    m.write_u32(VEC + 4, 1.0f32.to_bits());
    s.ctx.gpr[A0] = VEC as i32 as i64 as u64;
    s
}

/// In the child: run one side on a NaN component.
#[test]
fn nan_child() {
    let Ok(side) = std::env::var(CHILD_ENV) else { return };
    let mut s = nan_vector();
    let f = match side.as_str() {
        "c" => oracle::recomp::by_name("func_800151C0").unwrap(),
        _ => difftest::port_under_test("func_800151C0", math::func_800151C0),
    };
    s.run(f);
    println!("result {:#010x}", s.ctx.fpr[0].u32l());
    std::process::exit(0);
}

fn child(side: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "nan_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, side)
        .output()
        .unwrap()
}

/// The C stops on the NaN operand (the assert fails); the port computes on
/// (IEEE: the NaN propagates), so there is no reference to compare it with.
#[test]
fn nan_operand_stops_the_c() {
    let c = child("c");
    let stderr = String::from_utf8_lossy(&c.stderr);
    assert!(!c.status.success(), "the C returned on a NaN operand");
    assert!(stderr.contains("Assertion failed"), "unexpected stderr:\n{stderr}");

    let rust = child("rust");
    assert!(rust.status.success(), "{}", String::from_utf8_lossy(&rust.stderr));
    let stdout = String::from_utf8_lossy(&rust.stdout);
    let bits = stdout.split("result ").nth(1).and_then(|r| r.split_whitespace().next()).expect("no result line");
    let v = f32::from_bits(u32::from_str_radix(bits.trim_start_matches("0x"), 16).unwrap());
    assert!(v.is_nan(), "{bits}");
}
