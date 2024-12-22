use egui::{
    util::cache::{ComputerMut, FrameCache},
    Align, Id, Layout, Widget,
};
use egui_extras::Column;
use game_data::{
    find_recipes, get_game_settings, get_job_name, Consumable, Ingredient, Locale, RLVLS,
};

use crate::config::{CrafterConfig, QualitySource, RecipeConfiguration};

use super::ItemNameLabel;

#[derive(Default)]
struct RecipeFinder {}

impl ComputerMut<(&str, Locale), Vec<usize>> for RecipeFinder {
    fn compute(&mut self, (text, locale): (&str, Locale)) -> Vec<usize> {
        find_recipes(text, locale)
    }
}

type SearchCache<'a> = FrameCache<Vec<usize>, RecipeFinder>;

pub struct RecipeSelect<'a> {
    crafter_config: &'a mut CrafterConfig,
    recipe_config: &'a mut RecipeConfiguration,
    selected_food: Option<Consumable>, // used for base prog/qual display
    selected_potion: Option<Consumable>, // used for base prog/qual display
    locale: Locale,
}

impl<'a> RecipeSelect<'a> {
    pub fn new(
        crafter_config: &'a mut CrafterConfig,
        recipe_config: &'a mut RecipeConfiguration,
        selected_food: Option<Consumable>,
        selected_potion: Option<Consumable>,
        locale: Locale,
    ) -> Self {
        Self {
            crafter_config,
            recipe_config,
            selected_food,
            selected_potion,
            locale,
        }
    }

    fn draw_normal_recipe_select(self, ui: &mut egui::Ui) {
        let mut search_text = String::new();
        ui.ctx().data_mut(|data| {
            if let Some(text) = data.get_persisted::<String>(Id::new("RECIPE_SEARCH_TEXT")) {
                search_text = text;
            }
        });

        if egui::TextEdit::singleline(&mut search_text)
            .desired_width(f32::INFINITY)
            .ui(ui)
            .changed()
        {
            search_text = search_text.replace("\0", "");
        };
        ui.separator();

        let mut search_result = Vec::new();
        ui.ctx().memory_mut(|mem| {
            let search_cache = mem.caches.cache::<SearchCache<'_>>();
            search_result = search_cache.get((&search_text, self.locale));
        });

        ui.ctx().data_mut(|data| {
            data.insert_persisted(Id::new("RECIPE_SEARCH_TEXT"), search_text);
        });

        let line_height = ui.spacing().interact_size.y;
        let line_spacing = ui.spacing().item_spacing.y;
        let table_height = 6.3 * line_height + 6.0 * line_spacing;

        let table = egui_extras::TableBuilder::new(ui)
            .id_salt("RECIPE_SELECT_TABLE")
            .auto_shrink(false)
            .striped(true)
            .column(Column::exact(42.0))
            .column(Column::exact(28.0))
            .column(Column::remainder().clip(true))
            .min_scrolled_height(table_height)
            .max_scroll_height(table_height);
        table.body(|body| {
            body.rows(line_height, search_result.len(), |mut row| {
                let recipe = game_data::RECIPES[search_result[row.index()]];
                row.col(|ui| {
                    if ui.button(t!("label.select")).clicked() {
                        self.crafter_config.selected_job = recipe.job_id;
                        *self.recipe_config = RecipeConfiguration {
                            recipe,
                            quality_source: QualitySource::HqMaterialList([0; 6]),
                        }
                    };
                });
                row.col(|ui| {
                    ui.label(get_job_name(recipe.job_id, self.locale));
                });
                row.col(|ui| {
                    ui.add(ItemNameLabel::new(recipe.item_id, false, self.locale));
                });
            });
        });
    }

    fn draw_custom_recipe_select(self, ui: &mut egui::Ui) {
        self.recipe_config.recipe.item_id = 0;
        self.recipe_config.recipe.material_quality_factor = 0;
        self.recipe_config.recipe.ingredients = [Ingredient {
            item_id: 0,
            amount: 0,
        }; 6];

        let game_settings = get_game_settings(
            self.recipe_config.recipe,
            *self.crafter_config.active_stats(),
            self.selected_food,
            self.selected_potion,
            false,
        );

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Level:");
                    ui.add(
                        egui::DragValue::new(&mut self.recipe_config.recipe.level).range(1..=100),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Recipe Level:");
                    ui.add(
                        egui::DragValue::new(&mut self.recipe_config.recipe.recipe_level)
                            .range(1..=RLVLS.len() - 1),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Progress:");
                    ui.add(egui::DragValue::new(
                        &mut self.recipe_config.recipe.progress,
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("Quality:");
                    ui.add(egui::DragValue::new(&mut self.recipe_config.recipe.quality));
                });
                if let QualitySource::Value(initial_quality) =
                    &mut self.recipe_config.quality_source
                {
                    ui.horizontal(|ui| {
                        ui.label("Initial Quality:");
                        ui.add(
                            egui::DragValue::new(initial_quality)
                                .range(0..=self.recipe_config.recipe.quality),
                        );
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Durability:");
                    ui.add(
                        egui::DragValue::new(&mut self.recipe_config.recipe.durability)
                            .range(10..=100),
                    );
                });
                ui.checkbox(&mut self.recipe_config.recipe.is_expert, "Expert recipe");
            });
            ui.separator();
            ui.vertical(|ui| {
                let mut rlvl = RLVLS[self.recipe_config.recipe.recipe_level as usize];
                ui.horizontal(|ui| {
                    ui.label("Progress divider");
                    ui.add_enabled(false, egui::DragValue::new(&mut rlvl.progress_div));
                });
                ui.horizontal(|ui| {
                    ui.label("Quality divider");
                    ui.add_enabled(false, egui::DragValue::new(&mut rlvl.quality_div));
                });
                ui.horizontal(|ui| {
                    ui.label("Progress modifier");
                    ui.add_enabled(false, egui::DragValue::new(&mut rlvl.progress_mod));
                });
                ui.horizontal(|ui| {
                    ui.label("Quality modifier");
                    ui.add_enabled(false, egui::DragValue::new(&mut rlvl.quality_mod));
                });
                ui.horizontal(|ui| {
                    ui.label("Progress per 100% efficiency:");
                    ui.label(egui::RichText::new(game_settings.base_progress.to_string()).strong());
                });
                ui.horizontal(|ui| {
                    ui.label("Quality per 100% efficiency:");
                    ui.label(egui::RichText::new(game_settings.base_quality.to_string()).strong());
                });
            });
        });
    }
}

impl<'a> Widget for RecipeSelect<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            ui.vertical(|ui| {
                let mut custom_recipe = false;
                ui.ctx().data_mut(|data| {
                    if let Some(value) = data.get_persisted::<bool>(Id::new("CUSTOM_RECIPE")) {
                        custom_recipe = value;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!("label.recipe")).strong());
                    ui.add(ItemNameLabel::new(
                        self.recipe_config.recipe.item_id,
                        false,
                        self.locale,
                    ));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .checkbox(&mut custom_recipe, t!("label.custom_recipe"))
                            .changed()
                        {
                            self.recipe_config.quality_source = match custom_recipe {
                                true => QualitySource::Value(0),
                                false => QualitySource::HqMaterialList([0; 6]),
                            }
                        };
                    });
                });
                ui.separator();
                if custom_recipe {
                    self.draw_custom_recipe_select(ui);
                } else {
                    self.draw_normal_recipe_select(ui);
                }

                ui.ctx().data_mut(|data| {
                    data.insert_persisted(Id::new("CUSTOM_RECIPE"), custom_recipe);
                });
            });
        })
        .response
    }
}
