use std::time::Instant;

use raphael_rs::{
    config::Settings,
    game::{
        actions::{PROG_DENOM, QUAL_DENOM},
        state::State,
    }, solvers::macro_solver::MacroSolver,
};

fn main() {
    let timer = Instant::now();

    let settings = Settings {
        max_cp: 120,
        max_durability: 60,
        max_progress: (20.00 * PROG_DENOM) as i32,
        max_quality: (400.00 * QUAL_DENOM) as i32,
    };
    let state = State::new(&settings);
    let solver = MacroSolver::new(settings);
    solver.solve(state);

    println!("time elapsed: {}s", timer.elapsed().as_secs_f32());
}
