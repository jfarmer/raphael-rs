use crate::game::units::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Settings {
    pub max_cp: CP,
    pub max_durability: Durability,
    pub max_progress: Progress,
    pub max_quality: Quality,
    pub allowed_actions: ActionMask,
}

use crate::game::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionMask {
    mask: u64,
}

impl ActionMask {
    pub const fn none() -> Self {
        Self { mask: 0 }
    }

    pub fn from_level(level: u32, manipulation: bool) -> Self {
        let mut result = Self::none();
        for action in ALL_ACTIONS {
            if action.level_requirement() <= level {
                result = result.add(*action);
            }
        }
        if !manipulation {
            result = result.remove(Action::Manipulation);
        }
        result
    }

    pub const fn has(self, action: Action) -> bool {
        (self.mask & (1 << action as u64)) != 0
    }

    pub const fn add(self, action: Action) -> Self {
        let bit = 1 << (action as u64);
        Self {
            mask: self.mask | bit,
        }
    }

    pub const fn remove(self, action: Action) -> Self {
        let bit = 1 << (action as u64);
        Self {
            mask: (self.mask | bit) ^ bit,
        }
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            mask: self.mask | other.mask,
        }
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self {
            mask: self.mask & other.mask,
        }
    }

    pub fn actions(self) -> Box<[Action]> {
        ALL_ACTIONS
            .iter()
            .copied()
            .filter(|action| ((self.mask >> *action as u64) & 1) != 0)
            .collect()
    }

    pub fn actions_iter(self) -> impl Iterator<Item = Action> {
        ALL_ACTIONS
            .iter()
            .copied()
            .filter(move |action| ((self.mask >> *action as u64) & 1) != 0)
    }
}

#[macro_export]
macro_rules! action_mask {
    ( $( $x:expr ),* ) => {
        {
            let mut action_mask = ActionMask::none();
            $(
                action_mask = action_mask.add($x);
            )*
            action_mask
        }
    };
}

const ALL_ACTIONS: &[Action] = &[
    Action::BasicSynthesis,
    Action::BasicTouch,
    Action::MasterMend,
    Action::Observe,
    Action::TricksOfTheTrade,
    Action::WasteNot,
    Action::Veneration,
    Action::StandardTouch,
    Action::ComboStandardTouch,
    Action::GreatStrides,
    Action::Innovation,
    Action::WasteNot2,
    Action::ByregotsBlessing,
    Action::PreciseTouch,
    Action::MuscleMemory,
    Action::CarefulSynthesis,
    Action::Manipulation,
    Action::PrudentTouch,
    Action::FocusedSynthesis,
    Action::FocusedTouch,
    Action::Reflect,
    Action::PreparatoryTouch,
    Action::Groundwork,
    Action::DelicateSynthesis,
    Action::IntensiveSynthesis,
    Action::AdvancedTouch,
    Action::ComboAdvancedTouch,
    Action::PrudentSynthesis,
    Action::TrainedFinesse,
];
