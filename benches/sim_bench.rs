//! Criterion benchmark over a whole playout.
//!
//! The sim is turn-based, so the unit worth measuring is a run rather than a
//! tick: generate a map, walk the amoeba back and forth, and resolve a view
//! each turn the way a frontend would.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use amoeba_rl::sim::grid::Dir;
use amoeba_rl::sim::{Command, Difficulty, Sim};

/// Turns per playout iteration.
const TURNS: usize = 200;

fn playout(seed: u64) -> Sim {
    let mut sim = Sim::new(seed, Difficulty::Normal);
    sim.advance(Some(Command::Start(Difficulty::Normal)));
    for turn in 0..TURNS {
        let dir = if turn % 2 == 0 { Dir::Left } else { Dir::Right };
        sim.advance(Some(Command::Move(dir)));
    }
    sim
}

fn bench_playout(c: &mut Criterion) {
    c.bench_function("sim_playout", |b| {
        b.iter(|| black_box(playout(black_box(0x5EED)).mass()));
    });
}

fn bench_mapgen(c: &mut Criterion) {
    let mut seed = 0_u64;
    c.bench_function("sim_mapgen", |b| {
        b.iter(|| {
            seed = seed.wrapping_add(1);
            let mut sim = Sim::new(black_box(seed), Difficulty::Normal);
            sim.advance(Some(Command::Start(Difficulty::Normal)));
            black_box(sim.mass())
        });
    });
}

fn bench_view(c: &mut Criterion) {
    let sim = playout(7);
    c.bench_function("sim_view", |b| {
        b.iter(|| black_box(sim.view(black_box(0))));
    });
}

criterion_group!(benches, bench_playout, bench_mapgen, bench_view);
criterion_main!(benches);
