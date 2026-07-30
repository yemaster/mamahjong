use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use mahjong_riichi::{RiichiVariant, TileSet, Wall, WallSeed};

const DEFAULT_ITERATIONS: u64 = 100_000;

fn main() -> Result<(), Box<dyn Error>> {
    let iterations = match std::env::args().nth(1) {
        Some(value) => value.parse::<u64>()?,
        None => DEFAULT_ITERATIONS,
    };
    if iterations == 0 {
        return Err("iteration count must be greater than zero".into());
    }

    let started = Instant::now();
    let mut checksum = 0_u64;
    for iteration in 0..iterations {
        let mut seed = [0_u8; 32];
        seed[..8].copy_from_slice(&iteration.to_le_bytes());
        let mut wall = Wall::new(
            TileSet::standard(RiichiVariant::Yonma),
            &WallSeed::from_bytes(seed),
        );
        let first_tile = wall
            .draw_live()
            .expect("a newly created wall contains live tiles");
        checksum ^= u64::from(first_tile.id().value());
        black_box(wall);
    }
    let elapsed = started.elapsed();
    let walls_per_second = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "generated {iterations} yonma walls in {elapsed:.3?} ({walls_per_second:.0} walls/s, checksum={checksum})"
    );
    Ok(())
}
