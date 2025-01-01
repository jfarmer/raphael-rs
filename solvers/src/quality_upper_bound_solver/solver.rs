use crate::{
    actions::{SolverAction, FULL_SEARCH_ACTIONS, PROGRESS_ONLY_SEARCH_ACTIONS},
    utils::{AtomicFlag, ParetoFrontBuilder, ParetoFrontId, ParetoValue},
};
use simulator::*;

use rustc_hash::FxHashMap as HashMap;

use super::state::ReducedState;

pub struct SolverSettings {
    pub durability_cost: i16, // how much CP does it cost to restore 5 durability?
    pub backload_progress: bool,
    pub unsound_branch_pruning: bool,
}

pub struct QualityUpperBoundSolver {
    simulator_settings: Settings,
    solver_settings: SolverSettings,
    solved_states: HashMap<ReducedState, ParetoFrontId>,
    pareto_front_builder: ParetoFrontBuilder<u16, u16>,
    interrupt_signal: AtomicFlag,
    // pre-computed branch pruning values
    waste_not_1_min_cp: i16,
    waste_not_2_min_cp: i16,
}

impl QualityUpperBoundSolver {
    pub fn new(
        settings: Settings,
        backload_progress: bool,
        unsound_branch_pruning: bool,
        interrupt_signal: AtomicFlag,
    ) -> Self {
        log::trace!(
            "ReducedState (QualityUpperBoundSolver) - size: {}, align: {}",
            std::mem::size_of::<ReducedState>(),
            std::mem::align_of::<ReducedState>()
        );

        let settings = Settings {
            max_cp: i16::MAX,
            ..settings
        };

        let initial_state = SimulationState::new(&settings);
        let mut durability_cost = 100;
        if settings.is_action_allowed::<MasterMend>() {
            let master_mend_cost = MasterMend::base_cp_cost(&initial_state, &settings);
            durability_cost = std::cmp::min(durability_cost, master_mend_cost / 6);
        }
        if settings.is_action_allowed::<Manipulation>() {
            let manipulation_cost = Manipulation::base_cp_cost(&initial_state, &settings);
            durability_cost = std::cmp::min(durability_cost, manipulation_cost / 8);
        }
        if settings.is_action_allowed::<ImmaculateMend>() {
            let immaculate_mend_cost = ImmaculateMend::base_cp_cost(&initial_state, &settings);
            let max_restored = settings.max_durability as i16 / 5 - 1;
            durability_cost = std::cmp::min(durability_cost, immaculate_mend_cost / max_restored);
        }

        Self {
            simulator_settings: settings,
            solver_settings: SolverSettings {
                durability_cost,
                backload_progress,
                unsound_branch_pruning,
            },
            solved_states: HashMap::default(),
            pareto_front_builder: ParetoFrontBuilder::new(
                settings.max_progress,
                settings.max_quality,
            ),
            interrupt_signal,
            waste_not_1_min_cp: waste_not_min_cp(56, 4, durability_cost),
            waste_not_2_min_cp: waste_not_min_cp(98, 8, durability_cost),
        }
    }

    /// Returns an upper-bound on the maximum Quality achievable from this state while also maxing out Progress.
    /// There is no guarantee on the tightness of the upper-bound.
    pub fn quality_upper_bound(&mut self, state: SimulationState) -> Option<u16> {
        if self.interrupt_signal.is_set() {
            return None;
        }

        let current_quality = state.quality;
        let missing_progress = self
            .simulator_settings
            .max_progress
            .saturating_sub(state.progress);

        let reduced_state = ReducedState::from_simulation_state(
            state,
            &self.simulator_settings,
            &self.solver_settings,
        );
        let pareto_front = match self.solved_states.get(&reduced_state) {
            Some(id) => self.pareto_front_builder.retrieve(*id),
            None => {
                self.pareto_front_builder.clear();
                self.solve_state(reduced_state);
                self.pareto_front_builder.peek().unwrap()
            }
        };

        match pareto_front.last() {
            Some(element) => {
                if element.first < missing_progress {
                    return Some(0);
                }
            }
            None => return Some(0),
        }

        let index = match pareto_front.binary_search_by_key(&missing_progress, |value| value.first)
        {
            Ok(i) => i,
            Err(i) => i,
        };

        Some(std::cmp::min(
            self.simulator_settings.max_quality,
            pareto_front[index].second.saturating_add(current_quality),
        ))
    }

    fn solve_state(&mut self, state: ReducedState) -> Option<()> {
        if self.interrupt_signal.is_set() {
            return None;
        }

        if state.data.combo() == Combo::None {
            self.solve_normal_state(state)
        } else {
            self.solve_combo_state(state)
        }
    }

    fn solve_normal_state(&mut self, state: ReducedState) -> Option<()> {
        self.pareto_front_builder.push_empty();
        let search_actions = match state.data.progress_only() {
            true => PROGRESS_ONLY_SEARCH_ACTIONS,
            false => FULL_SEARCH_ACTIONS,
        };
        for action in search_actions.iter() {
            if !self.should_use_action(state, *action) {
                continue;
            }
            self.build_child_front(state, *action)?;
            if self.pareto_front_builder.is_max() {
                // stop early if both Progress and Quality are maxed out
                // this optimization would work even better with better action ordering
                // (i.e. if better actions are visited first)
                break;
            }
        }
        let id = self.pareto_front_builder.save().unwrap();
        self.solved_states.insert(state, id);

        Some(())
    }

    fn solve_combo_state(&mut self, state: ReducedState) -> Option<()> {
        match self.solved_states.get(&state.drop_combo()) {
            Some(id) => self.pareto_front_builder.push_from_id(*id),
            None => self.solve_normal_state(state.drop_combo())?,
        }
        match state.data.combo() {
            Combo::None => unreachable!(),
            Combo::SynthesisBegin => {
                self.build_child_front(state, SolverAction::Single(Action::MuscleMemory))?;
                self.build_child_front(state, SolverAction::Single(Action::Reflect))?;
                self.build_child_front(state, SolverAction::Single(Action::TrainedEye))?;
            }
            Combo::BasicTouch => {
                self.build_child_front(state, SolverAction::Single(Action::RefinedTouch))?;
                self.build_child_front(state, SolverAction::Single(Action::StandardTouch))?;
            }
            Combo::StandardTouch => {
                self.build_child_front(state, SolverAction::Single(Action::AdvancedTouch))?;
            }
        }

        Some(())
    }

    fn build_child_front(&mut self, state: ReducedState, action: SolverAction) -> Option<()> {
        if self.interrupt_signal.is_set() {
            return None;
        }

        if let Ok((new_state, action_progress, action_quality)) =
            state.use_action(action, &self.simulator_settings, &self.solver_settings)
        {
            if new_state.data.cp() >= self.solver_settings.durability_cost {
                match self.solved_states.get(&new_state) {
                    Some(id) => self.pareto_front_builder.push_from_id(*id),
                    None => self.solve_state(new_state)?,
                }
                self.pareto_front_builder.map(move |value| {
                    value.first = value.first.saturating_add(action_progress);
                    value.second = value.second.saturating_add(action_quality);
                });
                self.pareto_front_builder.merge();
            } else if new_state.data.cp() >= -self.solver_settings.durability_cost
                && action_progress != 0
            {
                // "durability" must not go lower than -5
                // last action must be a progress increase
                self.pareto_front_builder
                    .push_from_slice(&[ParetoValue::new(action_progress, action_quality)]);
                self.pareto_front_builder.merge();
            }
        }

        Some(())
    }

    fn should_use_action(&self, state: ReducedState, action: SolverAction) -> bool {
        match action {
            SolverAction::Single(Action::WasteNot) => state.data.cp() >= self.waste_not_1_min_cp,
            SolverAction::Single(Action::WasteNot2) => state.data.cp() >= self.waste_not_2_min_cp,
            _ => true,
        }
    }
}

/// Calculates the minimum CP a state must have so that using WasteNot is not worse than just restoring durability via CP
fn waste_not_min_cp(
    waste_not_action_cp_cost: i16,
    effect_duration: i16,
    durability_cost: i16,
) -> i16 {
    const BASIC_SYNTH_CP: i16 = 0;
    const GROUNDWORK_CP: i16 = 18;
    // how many units of 5-durability does WasteNot have to save to be worth using over magically restoring durability?
    let min_durability_save = (waste_not_action_cp_cost - 1) / durability_cost + 1;
    if min_durability_save > effect_duration * 2 {
        return i16::MAX;
    }
    // how many 20-durability actions and how many 10-durability actions are needed?
    let double_dur_count = min_durability_save.saturating_sub(effect_duration);
    let single_dur_count = min_durability_save.abs_diff(effect_duration) as i16;
    // minimum CP required to execute those actions
    let double_dur_cost = double_dur_count * (GROUNDWORK_CP + durability_cost * 2);
    let single_dur_cost = single_dur_count * (BASIC_SYNTH_CP + durability_cost);
    waste_not_action_cp_cost + double_dur_cost + single_dur_cost - durability_cost
}
