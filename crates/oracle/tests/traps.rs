//! The stub runtime's entry points must abort the process loudly, not return
//! or hang. Each case re-runs this test binary as a child that hits one stub.

use oracle::runtime::{do_break, get_function, oracle_unexpected_call, switch_error};
use std::process::Command;

const CHILD_ENV: &str = "ORACLE_TRAP_CHILD";

/// In the child, hit the stub named by the env var. Never returns there.
fn child() {
    let Ok(which) = std::env::var(CHILD_ENV) else { return };
    unsafe {
        match which.as_str() {
            "break" => do_break(0x8000_1234),
            "lookup" => {
                get_function(0x8001_0000u32 as i32);
            }
            "switch" => switch_error(c"func_80001000".as_ptr(), 0x8000_1010, 0x800A_0000),
            "call" => oracle_unexpected_call(c"func_80002000".as_ptr()),
            _ => unreachable!(),
        }
    }
    // Reaching this line means the stub returned instead of trapping.
    std::process::exit(0);
}

fn run_child(which: &str) -> (bool, String) {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "trap_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, which)
        .output()
        .unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn trap_child() {
    child();
}

#[test]
fn stubs_abort_loudly() {
    for (which, expect) in [
        ("break", "break instruction at 0x80001234"),
        ("lookup", "LOOKUP_FUNC(0x80010000)"),
        ("switch", "func_80001000: jump table at 0x800A0000"),
        ("call", "call to func_80002000, which is not compiled into the oracle"),
    ] {
        let (ok, stderr) = run_child(which);
        assert!(!ok, "{which}: child exited successfully; the stub did not trap");
        assert!(stderr.contains("oracle trap") && stderr.contains(expect), "{which}: unexpected stderr:\n{stderr}");
    }
}
