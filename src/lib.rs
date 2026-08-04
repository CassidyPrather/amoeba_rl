//! Amoeba Roguelike Remastered: a Rust remaster of the 7DRL roguelike.
//!
//! The split the crate is organised around: [`sim`] is the whole game as a
//! pure, deterministic, dependency-light library, and the binary is a thin
//! macroquad shell that turns key presses into [`sim::Command`]s and a
//! [`sim::RenderView`] into glyphs. No macroquad types appear in here, so the
//! interesting half runs headless in `cargo test` and `cargo bench` at
//! whatever speed the CPU allows.

pub mod sim;
pub mod synth;

/// `git describe` version, embedded by `build.rs`.
pub const VERSION: &str = env!("GIT_DESCRIBE_VERSION");
