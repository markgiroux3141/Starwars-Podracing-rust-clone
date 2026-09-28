//! Star Wars Episode I: Racer (N64, USA), reimplemented in Rust.
//!
//! Phase 1 layout: each ported function keeps N64Recomp's signature and works
//! on the same RDRAM layout (through `n64mem`) and register file
//! ([`recomp::RecompContext`]), so it can stand in for the recompiled C and be
//! tested against it (`crates/difftest`).

pub mod recomp;
