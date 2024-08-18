use simulator::{Action, ActionMask, Settings, SimulationState};
use solvers::MacroSolver;

use log::debug;

fn main() {
    debug!("SimulationState size: {} bytes", std::mem::size_of::<SimulationState>());
    debug!("SimulationState alignment: {} bytes", std::mem::align_of::<SimulationState>());

    // Ra'Kaznar Lapidary Hammer
    // 4462 Craftsmanship, 4391 Control
    let settings = Settings {
        max_cp: 569,
        max_durability: 80,
        max_progress: 6600,
        max_quality: 10000,
        base_progress: 237,
        base_quality: 245,
        job_level: 100,
        allowed_actions: ActionMask::from_level(100)
            .remove(Action::TrainedEye)
            .remove(Action::HeartAndSoul)
            .remove(Action::QuickInnovation),
        adversarial: false,
    };

    let state = SimulationState::new(&settings);
    let actions = MacroSolver::new(settings, Box::new(|_| {}), Box::new(|_| {}))
        .solve(state, false)
        .unwrap();

    let quality = SimulationState::from_macro(&settings, &actions)
        .unwrap()
        .quality;
    let steps = actions.len();
    let duration: i16 = actions.iter().map(|action| action.time_cost()).sum();
    debug!("Quality: {}, Steps: {}, Duration: {}", quality, steps, duration);
}
