//! Test doubles: a stub for a function that is not compiled into the oracle
//! runs the double installed on the current thread, records the call, and
//! goes back to trapping once the guard is dropped (the trap itself is
//! checked in traps.rs).

use game::recomp::{reg::*, RecompContext};
use n64mem::Rdram;
use oracle::doubles::{self, clobber_caller_saved, install, restore_saved};

extern "C" {
    // rom_read: never compiled into the oracle (the PI chain below it has
    // indirect calls), so it is always a generated stub.
    fn func_80011CDC(rdram: *mut u8, ctx: *mut RecompContext);
}

fn call(rdram: &mut Rdram, ctx: &mut RecompContext) {
    ctx.fix_f_odd();
    unsafe { func_80011CDC(rdram.as_mut_ptr(), ctx) }
}

#[test]
fn installed_double_runs_and_is_traced() {
    let mut rdram = Rdram::new();
    let mut ctx = RecompContext::default();
    ctx.gpr[A0] = 0x1234;
    ctx.gpr[A1] = 0xFFFF_FFFF_8010_0000;
    {
        let _g = install("func_80011CDC", |mem, ctx| {
            mem.write_u32(ctx.gpr[A1] as u32, ctx.gpr[A0] as u32);
            ctx.gpr[V0] = 7;
        });
        doubles::start_trace();
        call(&mut rdram, &mut ctx);
        call(&mut rdram, &mut ctx);
        let trace = doubles::take_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].name, "func_80011CDC");
        assert_eq!(trace[0].gpr[A0], 0x1234);
        assert_eq!(trace[1].gpr[V0], 7, "second call sees the first one's v0");
    }
    assert_eq!(rdram.mem().read_u32(0x8010_0000), 0x1234);
    assert!(doubles::take_trace().is_empty());
}

#[test]
fn nested_installs_restore_the_outer_double() {
    let mut rdram = Rdram::new();
    let mut ctx = RecompContext::default();
    let _outer = install("func_80011CDC", |_, ctx| ctx.gpr[V0] = 1);
    {
        let _inner = install("func_80011CDC", |_, ctx| ctx.gpr[V0] = 2);
        call(&mut rdram, &mut ctx);
        assert_eq!(ctx.gpr[V0], 2);
    }
    call(&mut rdram, &mut ctx);
    assert_eq!(ctx.gpr[V0], 1);
}

#[test]
#[should_panic(expected = "compiled into the oracle")]
fn doubles_for_compiled_in_functions_are_refused() {
    let _g = install("func_80000554", |_, _| {});
}

#[test]
#[should_panic(expected = "not listed in crates/oracle/doubles.txt")]
fn doubles_must_be_listed() {
    // osPiStartDma: not compiled in, and not listed.
    let _g = install("func_80087D70", |_, _| {});
}

#[test]
fn listed_kinds() {
    assert_eq!(doubles::listed("func_80011CDC"), Some(doubles::Kind::Contract));
    assert_eq!(doubles::listed("func_800827C0"), Some(doubles::Kind::StandIn));
    assert_eq!(doubles::listed("func_80087D70"), None);
}

#[test]
fn abi_helpers() {
    let mut a = RecompContext::default();
    a.gpr[S0] = 0x1234_5678_8000_0000;
    a.gpr[SP] = 0x0000_0000_8000_F000;
    a.gpr[S4] = 0x5555_0000_0000_0001;
    restore_saved(&mut a, &[S0, SP]);
    assert_eq!(a.gpr[S0], 0xFFFF_FFFF_8000_0000);
    assert_eq!(a.gpr[SP], 0xFFFF_FFFF_8000_F000);
    assert_eq!(a.gpr[S4], 0x5555_0000_0000_0001, "not in the list: untouched");

    // Same position in the trace, same values; callee-saved registers and
    // `keep` are untouched.
    let mut b = a;
    doubles::start_trace();
    clobber_caller_saved(&mut a, &[V0]);
    clobber_caller_saved(&mut b, &[V0]);
    assert_eq!(a.gpr, b.gpr);
    assert_eq!(a.gpr[V0], 0);
    assert_eq!(a.gpr[S4], 0x5555_0000_0000_0001);
    assert_ne!(a.gpr[T0], 0);
    assert_eq!(a.gpr[T0], a.gpr[T0] as u32 as i32 as i64 as u64, "sign-extended");
}
