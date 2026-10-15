use std::hint::black_box;
use std::time::Instant;
use tinderbox_sim::level::Preset;
use tinderbox_sim::{DEFAULT_RULES, Rules};

const WIDTH: usize = 384;
const HEIGHT: usize = 240;
const WORK_PER_FRAME: usize = 12_000;
const FRAMES: usize = 100;
const MIN_CELLS_PER_SECOND: f64 = 7_000_000.0;

fn main() {
    let rules = Rules::load(DEFAULT_RULES).expect("default rules are valid");
    let level = Preset::DryForest.level(&rules, WIDTH, HEIGHT);
    let mut world = level.generate(rules).expect("dry forest preset generates");
    let started = Instant::now();
    let completed = (0..FRAMES)
        .map(|_| black_box(world.step(WORK_PER_FRAME)))
        .sum::<usize>();
    let elapsed = started.elapsed().as_secs_f64();
    let cells_per_second = completed as f64 / elapsed;
    println!(
        "dry_forest={}x{} frames={} completed_cell_updates={} elapsed_s={elapsed:.3} cells_per_second={cells_per_second:.0}",
        WIDTH, HEIGHT, FRAMES, completed
    );
    assert!(
        cells_per_second >= MIN_CELLS_PER_SECOND,
        "simulation throughput {cells_per_second:.0} cells/s is below the generous floor of {MIN_CELLS_PER_SECOND:.0}"
    );
}
