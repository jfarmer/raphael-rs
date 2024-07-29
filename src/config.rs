use game_data::{CrafterStats, Recipe};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QualitySource {
    HqMaterialList([u8; 6]),
    Value(u16),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RecipeConfiguration {
    pub recipe: Recipe,
    pub quality_source: QualitySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct CrafterConfig {
    pub selected_job: u8,
    pub crafter_stats: [CrafterStats; 8],
}

impl CrafterConfig {
    pub fn active_stats(&self) -> &CrafterStats {
        &self.crafter_stats[self.selected_job as usize]
    }

    pub fn active_stats_mut(&mut self) -> &mut CrafterStats {
        &mut self.crafter_stats[self.selected_job as usize]
    }
}

impl Default for CrafterConfig {
    fn default() -> Self {
        Self {
            selected_job: 1,
            crafter_stats: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityTarget {
    Zero,
    CollectableT1,
    CollectableT2,
    CollectableT3,
    Full,
    Custom(u16),
}

impl QualityTarget {
    pub fn get_target(self, max_quality: u16) -> u16 {
        match self {
            Self::Zero => 0,
            Self::CollectableT1 => (max_quality as f64 * 0.55).ceil() as u16,
            Self::CollectableT2 => (max_quality as f64 * 0.75).ceil() as u16,
            Self::CollectableT3 => (max_quality as f64 * 0.95).ceil() as u16,
            Self::Full => max_quality,
            Self::Custom(quality) => quality,
        }
    }
}

impl Default for QualityTarget {
    fn default() -> Self {
        Self::Full
    }
}

impl std::fmt::Display for QualityTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Zero => "0% quality",
                Self::CollectableT1 => "55% quality",
                Self::CollectableT2 => "75% quality",
                Self::CollectableT3 => "95% quality",
                Self::Full => "100% quality",
                Self::Custom(_) => "Custom",
            }
        )
    }
}
