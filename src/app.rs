use std::borrow::Cow;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use egui::{Align, CursorIcon, Id, Layout, TextStyle, Visuals};
use game_data::{action_name, get_initial_quality, get_job_name, Consumable, Locale};

use simulator::{Action, ActionImpl, HeartAndSoul, Manipulation, QuickInnovation, Settings};

use crate::config::{CrafterConfig, QualitySource, QualityTarget, RecipeConfiguration};
use crate::widgets::*;
use crate::worker::BridgeType;

fn load<T: DeserializeOwned>(cc: &eframe::CreationContext<'_>, key: &'static str, default: T) -> T {
    match cc.storage {
        Some(storage) => eframe::get_value(storage, key).unwrap_or(default),
        None => default,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SolverInput {
    Start(Settings, SolverConfig),
    Cancel,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SolverEvent {
    Progress(usize),
    IntermediateSolution(Vec<Action>),
    FinalSolution(Vec<Action>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolverConfig {
    pub quality_target: QualityTarget,
    pub backload_progress: bool,
    pub adversarial: bool,
    pub minimize_steps: bool,
}

pub struct SolverUpdates {
    pub progress_update: Cell<Option<SolverEvent>>,
    pub solution_update: Cell<Option<SolverEvent>>,
}

pub struct MacroSolverApp {
    locale: Locale,
    recipe_config: RecipeConfiguration,
    selected_food: Option<Consumable>,
    selected_potion: Option<Consumable>,
    crafter_config: CrafterConfig,
    solver_config: SolverConfig,
    macro_view_config: MacroViewConfig,

    stats_edit_window_open: bool,
    actions: Vec<Action>,
    solver_pending: bool,
    solver_interrupt_pending: bool,
    solver_progress: usize,
    start_time: Option<Instant>,
    duration: Option<Duration>,
    data_update: Rc<SolverUpdates>,
    bridge: BridgeType,
}

impl MacroSolverApp {
    #[cfg(target_arch = "wasm32")]
    fn initialize_bridge(
        cc: &eframe::CreationContext<'_>,
        data_update: &Rc<SolverUpdates>,
    ) -> BridgeType {
        let ctx = cc.egui_ctx.clone();
        let sender = data_update.clone();

        <crate::worker::Worker as gloo_worker::Spawnable>::spawner()
            .callback(move |response| {
                match response {
                    SolverEvent::Progress(_) => sender.progress_update.set(Some(response)),
                    SolverEvent::IntermediateSolution(_) | SolverEvent::FinalSolution(_) => {
                        sender.solution_update.set(Some(response))
                    }
                }
                ctx.request_repaint();
            })
            .spawn(concat!("./webworker", env!("RANDOM_SUFFIX"), ".js"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn initialize_bridge(
        _cc: &eframe::CreationContext<'_>,
        _data_cell: &Rc<SolverUpdates>,
    ) -> BridgeType {
        BridgeType::new()
    }

    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let data_update = Rc::new(SolverUpdates {
            progress_update: Cell::new(None),
            solution_update: Cell::new(None),
        });
        let bridge = Self::initialize_bridge(cc, &data_update);

        let dark_mode = cc
            .egui_ctx
            .data_mut(|data| *data.get_persisted_mut_or(Id::new("DARK_MODE"), true));
        if dark_mode {
            cc.egui_ctx.set_visuals(Visuals::dark());
        } else {
            cc.egui_ctx.set_visuals(Visuals::light());
        }

        cc.egui_ctx.style_mut(|style| {
            style.visuals.interact_cursor = Some(CursorIcon::PointingHand);
            style.url_in_tooltip = true;
            style.always_scroll_the_only_direction = true;
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        });

        load_fonts(&cc.egui_ctx);

        let default_recipe_config = RecipeConfiguration {
            recipe: *game_data::RECIPES.last().unwrap(),
            quality_source: QualitySource::HqMaterialList([0; 6]),
        };

        Self {
            locale: load(cc, "LOCALE", Locale::EN),
            recipe_config: load(cc, "RECIPE_CONFIG", default_recipe_config),
            selected_food: load(cc, "SELECTED_FOOD", None),
            selected_potion: load(cc, "SELECTED_POTION", None),
            crafter_config: load(cc, "CRAFTER_CONFIG", Default::default()),
            solver_config: load(cc, "SOLVER_CONFIG", Default::default()),
            macro_view_config: load(cc, "MACRO_VIEW_CONFIG", Default::default()),

            stats_edit_window_open: false,
            actions: Vec::new(),
            solver_pending: false,
            solver_interrupt_pending: false,
            solver_progress: 0,
            start_time: None,
            duration: None,
            data_update,
            bridge,
        }
    }
}

impl eframe::App for MacroSolverApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_arch = "wasm32")]
        self.load_fonts_dyn(ctx);

        self.solver_update();

        match self.locale {
            Locale::EN => rust_i18n::set_locale("en"),
            Locale::DE => rust_i18n::set_locale("de"),
            Locale::FR => rust_i18n::set_locale("fr"),
            Locale::JP => rust_i18n::set_locale("jp"),
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.label(egui::RichText::new("Raphael  |  FFXIV Crafting Solver").strong());
                        ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));

                        egui::ComboBox::from_id_salt("LOCALE")
                            .selected_text(format!("{}", self.locale))
                            .width(0.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.locale,
                                    Locale::EN,
                                    format!("{}", Locale::EN),
                                );
                                ui.selectable_value(
                                    &mut self.locale,
                                    Locale::DE,
                                    format!("{}", Locale::DE),
                                );
                                ui.selectable_value(
                                    &mut self.locale,
                                    Locale::FR,
                                    format!("{}", Locale::FR),
                                );
                                ui.selectable_value(
                                    &mut self.locale,
                                    Locale::JP,
                                    format!("{}", Locale::JP),
                                );
                            });

                        let mut visuals = ctx.style().visuals.clone();
                        ui.selectable_value(&mut visuals, Visuals::light(), t!("header.light"));
                        ui.selectable_value(&mut visuals, Visuals::dark(), t!("header.dark"));
                        ctx.data_mut(|data| {
                            *data.get_persisted_mut_or_default(Id::new("DARK_MODE")) =
                                visuals.dark_mode;
                        });
                        ctx.set_visuals(visuals);

                        ui.add(
                            egui::Hyperlink::from_label_and_url(
                                t!("header.github"),
                                "https://github.com/KonaeAkira/raphael-rs",
                            )
                            .open_in_new_tab(true),
                        );
                        ui.label("/");
                        ui.add(
                            egui::Hyperlink::from_label_and_url(
                                t!("header.discord"),
                                "https://discord.com/invite/m2aCy3y8he",
                            )
                            .open_in_new_tab(true),
                        );
                        ui.label("/");
                        ui.add(
                            egui::Hyperlink::from_label_and_url(
                                t!("header.ko_fi"),
                                "https://ko-fi.com/konaeakira",
                            )
                            .open_in_new_tab(true),
                        );
                        ui.with_layout(
                            Layout::right_to_left(Align::Center),
                            egui::warn_if_debug_build,
                        );
                    });
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                self.draw_simulator_widget(ui);
                ui.with_layout(
                    Layout::left_to_right(Align::TOP).with_main_wrap(true),
                    |ui| {
                        let select_min_width: f32 = 612.0;
                        let config_min_width: f32 = 300.0;
                        let macro_min_width: f32 = 290.0;

                        let select_width;
                        let config_width;
                        let macro_width;

                        let row_width = ui.available_width();
                        if row_width >= select_min_width + config_min_width + macro_min_width {
                            select_width = row_width
                                - config_min_width
                                - macro_min_width
                                - 2.0 * ui.spacing().item_spacing.x;
                            config_width = config_min_width;
                            macro_width = macro_min_width;
                        } else if row_width >= select_min_width + config_min_width {
                            select_width =
                                row_width - config_min_width - ui.spacing().item_spacing.x;
                            config_width = config_min_width;
                            macro_width = row_width;
                        } else if row_width >= config_min_width + macro_min_width {
                            select_width = row_width;
                            config_width = config_min_width;
                            macro_width =
                                row_width - config_min_width - ui.spacing().item_spacing.x;
                        } else {
                            select_width = row_width;
                            config_width = row_width;
                            macro_width = row_width;
                        }

                        let response = ui
                            .allocate_ui(egui::vec2(select_width, 0.0), |ui| {
                                self.draw_list_select_widgets(ui);
                            })
                            .response;

                        let config_min_height = match ui.available_size_before_wrap().x {
                            x if x < config_width => 0.0,
                            _ => response.rect.height(),
                        };
                        let response = ui
                            .allocate_ui(egui::vec2(config_width, config_min_height), |ui| {
                                self.draw_config_and_results_widget(ui);
                            })
                            .response;

                        let macro_min_height = match ui.available_size_before_wrap().x {
                            x if x < macro_width => 0.0,
                            _ => response.rect.height(),
                        };
                        ui.allocate_ui(egui::vec2(macro_width, macro_min_height), |ui| {
                            self.draw_macro_output_widget(ui);
                        });
                    },
                );
            });
        });

        egui::Window::new(
            egui::RichText::new(t!("stats_window.title"))
                .strong()
                .text_style(TextStyle::Body),
        )
        .open(&mut self.stats_edit_window_open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add(StatsEdit::new(self.locale, &mut self.crafter_config));
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "LOCALE", &self.locale);
        eframe::set_value(storage, "RECIPE_CONFIG", &self.recipe_config);
        eframe::set_value(storage, "SELECTED_FOOD", &self.selected_food);
        eframe::set_value(storage, "SELECTED_POTION", &self.selected_potion);
        eframe::set_value(storage, "CRAFTER_CONFIG", &self.crafter_config);
        eframe::set_value(storage, "SOLVER_CONFIG", &self.solver_config);
        eframe::set_value(storage, "MACRO_VIEW_CONFIG", &self.macro_view_config);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }
}

impl MacroSolverApp {
    fn solver_update(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(update) = self.bridge.rx.try_recv() {
            match update {
                SolverEvent::Progress(_) => self.data_update.progress_update.set(Some(update)),
                SolverEvent::IntermediateSolution(_) | SolverEvent::FinalSolution(_) => {
                    self.data_update.solution_update.set(Some(update))
                }
            }
        }

        if let Some(update) = self.data_update.progress_update.take() {
            match update {
                SolverEvent::Progress(progress) => {
                    self.solver_progress = progress;
                }
                SolverEvent::IntermediateSolution(_) | SolverEvent::FinalSolution(_) => {
                    log::debug!("Unexpected progress update: {:?}", update);
                }
            }
        }

        if let Some(update) = self.data_update.solution_update.take() {
            match update {
                SolverEvent::Progress(_) => {
                    log::debug!("Unexpected solution update: {:?}", update);
                }
                SolverEvent::IntermediateSolution(actions) => {
                    self.actions = actions;
                }
                SolverEvent::FinalSolution(actions) => {
                    self.actions = actions;
                    self.duration = Some(Instant::now() - self.start_time.unwrap());
                    self.solver_pending = false;
                }
            }
        }
    }

    fn draw_simulator_widget(&mut self, ui: &mut egui::Ui) {
        let game_settings = game_data::get_game_settings(
            self.recipe_config.recipe,
            *self.crafter_config.active_stats(),
            self.selected_food,
            self.selected_potion,
            self.solver_config.adversarial,
        );
        let initial_quality = match self.recipe_config.quality_source {
            QualitySource::HqMaterialList(hq_materials) => {
                game_data::get_initial_quality(self.recipe_config.recipe, hq_materials)
            }
            QualitySource::Value(quality) => quality,
        };
        ui.add(Simulator::new(
            &game_settings,
            initial_quality,
            self.solver_config,
            &self.crafter_config,
            &self.actions,
            game_data::ITEMS
                .get(&self.recipe_config.recipe.item_id)
                .unwrap(),
            self.locale,
        ));
    }

    fn draw_list_select_widgets(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.add_enabled(
                !self.solver_pending,
                RecipeSelect::new(
                    &mut self.crafter_config,
                    &mut self.recipe_config,
                    self.selected_food,
                    self.selected_potion,
                    self.locale,
                ),
            );
            ui.add_enabled(
                !self.solver_pending,
                FoodSelect::new(
                    self.crafter_config.crafter_stats[self.crafter_config.selected_job as usize],
                    &mut self.selected_food,
                    self.locale,
                ),
            );
            ui.add_enabled(
                !self.solver_pending,
                PotionSelect::new(
                    self.crafter_config.crafter_stats[self.crafter_config.selected_job as usize],
                    &mut self.selected_potion,
                    self.locale,
                ),
            );
        });
    }

    fn draw_config_and_results_widget(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            // ui.set_height(560.0);
            ui.vertical(|ui| {
                ui.add_enabled_ui(!self.solver_pending, |ui| {
                    self.draw_configuration_widget(ui);
                });
                self.draw_results_widget(ui);
                // fill the remaining space
                ui.with_layout(Layout::bottom_up(Align::LEFT), |_| {});
            });
        });
    }

    fn draw_configuration_widget(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Configuration").strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.style_mut().spacing.item_spacing = [4.0, 4.0].into();
                if ui.button("✏").clicked() {
                    self.stats_edit_window_open = true;
                }
                egui::ComboBox::from_id_salt("SELECTED_JOB")
                    .width(20.0)
                    .selected_text(get_job_name(self.crafter_config.selected_job, self.locale))
                    .show_ui(ui, |ui| {
                        for i in 0..8 {
                            ui.selectable_value(
                                &mut self.crafter_config.selected_job,
                                i,
                                get_job_name(i, self.locale),
                            );
                        }
                    });
            });
        });
        ui.separator();

        ui.label(egui::RichText::new(t!("label.crafter_stats")).strong());
        ui.horizontal(|ui| {
            ui.label(format!("{}:", t!("craftsmanship")));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled(
                    false,
                    egui::DragValue::new(&mut game_data::craftsmanship_bonus(
                        self.crafter_config.active_stats().craftsmanship,
                        &[self.selected_food, self.selected_potion],
                    )),
                );
                ui.monospace("+");
                ui.add(
                    egui::DragValue::new(&mut self.crafter_config.active_stats_mut().craftsmanship)
                        .range(0..=9999),
                );
            });
        });
        ui.horizontal(|ui| {
            ui.label(format!("{}:", t!("control")));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled(
                    false,
                    egui::DragValue::new(&mut game_data::control_bonus(
                        self.crafter_config.active_stats().control,
                        &[self.selected_food, self.selected_potion],
                    )),
                );
                ui.monospace("+");
                ui.add(
                    egui::DragValue::new(&mut self.crafter_config.active_stats_mut().control)
                        .range(0..=9999),
                );
            });
        });
        ui.horizontal(|ui| {
            ui.label(format!("{}:", t!("cp")));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled(
                    false,
                    egui::DragValue::new(&mut game_data::cp_bonus(
                        self.crafter_config.active_stats().cp,
                        &[self.selected_food, self.selected_potion],
                    )),
                );
                ui.monospace("+");
                ui.add(
                    egui::DragValue::new(&mut self.crafter_config.active_stats_mut().cp)
                        .range(0..=999),
                );
            });
        });
        ui.horizontal(|ui| {
            ui.label(format!("{}:", t!("job_level")));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.crafter_config.active_stats_mut().level)
                        .range(1..=100),
                );
            });
        });
        ui.separator();

        ui.label(egui::RichText::new(t!("label.hq_materials")).strong());
        let mut has_hq_ingredient = false;
        let recipe_ingredients = self.recipe_config.recipe.ingredients;
        if let QualitySource::HqMaterialList(provided_ingredients) =
            &mut self.recipe_config.quality_source
        {
            for (index, ingredient) in recipe_ingredients.into_iter().enumerate() {
                if let Some(item) = game_data::ITEMS.get(&ingredient.item_id) {
                    if item.can_be_hq {
                        has_hq_ingredient = true;
                        ui.horizontal(|ui| {
                            ui.add(ItemNameLabel::new(ingredient.item_id, false, self.locale));
                            ui.with_layout(
                                Layout::right_to_left(Align::Center),
                                |ui: &mut egui::Ui| {
                                    let mut max_placeholder = ingredient.amount;
                                    ui.add_enabled(
                                        false,
                                        egui::DragValue::new(&mut max_placeholder),
                                    );
                                    ui.monospace("/");
                                    ui.add(
                                        egui::DragValue::new(&mut provided_ingredients[index])
                                            .range(0..=ingredient.amount),
                                    );
                                },
                            );
                        });
                    }
                }
            }
        }
        if !has_hq_ingredient {
            ui.label(t!("label.none"));
        }
        ui.separator();

        ui.label(egui::RichText::new(t!("label.actions")).strong());
        if self.crafter_config.active_stats().level >= Manipulation::LEVEL_REQUIREMENT {
            ui.add(egui::Checkbox::new(
                &mut self.crafter_config.active_stats_mut().manipulation,
                action_name(Action::Manipulation, self.locale),
            ));
        } else {
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut false, action_name(Action::Manipulation, self.locale)),
            );
        }
        if self.crafter_config.active_stats().level >= HeartAndSoul::LEVEL_REQUIREMENT {
            ui.add(egui::Checkbox::new(
                &mut self.crafter_config.active_stats_mut().heart_and_soul,
                action_name(Action::HeartAndSoul, self.locale),
            ));
        } else {
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut false, action_name(Action::HeartAndSoul, self.locale)),
            );
        }
        if self.crafter_config.active_stats().level >= QuickInnovation::LEVEL_REQUIREMENT {
            ui.add(egui::Checkbox::new(
                &mut self.crafter_config.active_stats_mut().quick_innovation,
                action_name(Action::QuickInnovation, self.locale),
            ));
        } else {
            ui.add_enabled(
                false,
                egui::Checkbox::new(
                    &mut false,
                    action_name(Action::QuickInnovation, self.locale),
                ),
            );
        }
        ui.separator();

        ui.label(egui::RichText::new(t!("label.solver_settings")).strong());
        ui.horizontal(|ui| {
            ui.label(t!("label.target_quality"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.style_mut().spacing.item_spacing = [4.0, 4.0].into();
                let game_settings = game_data::get_game_settings(
                    self.recipe_config.recipe,
                    self.crafter_config.crafter_stats[self.crafter_config.selected_job as usize],
                    self.selected_food,
                    self.selected_potion,
                    self.solver_config.adversarial,
                );
                let mut current_value = self
                    .solver_config
                    .quality_target
                    .get_target(game_settings.max_quality);
                match &mut self.solver_config.quality_target {
                    QualityTarget::Custom(value) => {
                        ui.add(egui::DragValue::new(value));
                    }
                    _ => {
                        ui.add_enabled(false, egui::DragValue::new(&mut current_value));
                    }
                };
                egui::ComboBox::from_id_salt("TARGET_QUALITY")
                    .selected_text(format!("{}", self.solver_config.quality_target))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.solver_config.quality_target,
                            QualityTarget::Zero,
                            format!("{}", QualityTarget::Zero),
                        );
                        ui.selectable_value(
                            &mut self.solver_config.quality_target,
                            QualityTarget::CollectableT1,
                            format!("{}", QualityTarget::CollectableT1),
                        );
                        ui.selectable_value(
                            &mut self.solver_config.quality_target,
                            QualityTarget::CollectableT2,
                            format!("{}", QualityTarget::CollectableT2),
                        );
                        ui.selectable_value(
                            &mut self.solver_config.quality_target,
                            QualityTarget::CollectableT3,
                            format!("{}", QualityTarget::CollectableT3),
                        );
                        ui.selectable_value(
                            &mut self.solver_config.quality_target,
                            QualityTarget::Full,
                            format!("{}", QualityTarget::Full),
                        );
                        ui.selectable_value(
                            &mut self.solver_config.quality_target,
                            QualityTarget::Custom(current_value),
                            format!("{}", QualityTarget::Custom(0)),
                        )
                    });
            });
        });

        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.solver_config.backload_progress,
                t!("label.backload_progress"),
            );
            ui.add(HelpText::new(t!("info.backload_progress")));
        });

        if self.recipe_config.recipe.is_expert {
            self.solver_config.adversarial = false;
        }
        ui.horizontal(|ui| {
            ui.add_enabled(
                !self.recipe_config.recipe.is_expert,
                egui::Checkbox::new(&mut self.solver_config.adversarial, t!("label.adversarial")),
            );
            ui.add(HelpText::new(t!("info.adversarial")));
        });
        if self.solver_config.adversarial {
            ui.label(
                egui::RichText::new(Self::experimental_warning_text())
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }

        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.solver_config.minimize_steps,
                t!("label.min_steps"),
            );
            ui.add(HelpText::new(t!("info.min_steps")));
        });
        if self.solver_config.minimize_steps {
            ui.label(
                egui::RichText::new(Self::experimental_warning_text())
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }
    }

    fn draw_results_widget(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.solver_pending {
                    if ui.button(t!("label.solve")).clicked() {
                        self.actions = Vec::new();
                        self.solver_pending = true;
                        self.solver_interrupt_pending = false;
                        self.solver_progress = 0;
                        self.start_time = Some(Instant::now());
                        let mut game_settings = game_data::get_game_settings(
                            self.recipe_config.recipe,
                            self.crafter_config.crafter_stats
                                [self.crafter_config.selected_job as usize],
                            self.selected_food,
                            self.selected_potion,
                            self.solver_config.adversarial,
                        );
                        let target_quality = self
                            .solver_config
                            .quality_target
                            .get_target(game_settings.max_quality);
                        let initial_quality = match self.recipe_config.quality_source {
                            QualitySource::HqMaterialList(hq_materials) => {
                                get_initial_quality(self.recipe_config.recipe, hq_materials)
                            }
                            QualitySource::Value(quality) => quality,
                        };

                        ui.ctx().data_mut(|data| {
                            data.insert_temp(
                                Id::new("LAST_SOLVE_PARAMS"),
                                (game_settings, initial_quality, self.solver_config),
                            );
                        });

                        game_settings.max_quality = target_quality.saturating_sub(initial_quality);
                        self.bridge
                            .send(SolverInput::Start(game_settings, self.solver_config));
                        log::debug!("{game_settings:?}");
                    }
                    if let Some(duration) = self.duration {
                        ui.label(format!("Time: {:.3}s", duration.as_secs_f64()));
                    }
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    ui.add_enabled_ui(
                        !self.solver_interrupt_pending && self.solver_pending,
                        |ui| {
                            if ui.button(t!("label.cancel")).clicked() {
                                self.bridge.send(SolverInput::Cancel);
                                self.solver_interrupt_pending = true;
                            }
                        },
                    );

                    ui.spinner();
                    if self.solver_progress == 0 {
                        ui.label("Populating DP tables");
                    } else {
                        // format with thousands separator
                        let num = self
                            .solver_progress
                            .to_string()
                            .as_bytes()
                            .rchunks(3)
                            .rev()
                            .map(std::str::from_utf8)
                            .collect::<Result<Vec<&str>, _>>()
                            .unwrap()
                            .join(",");
                        ui.label(format!("{} nodes visited", num));
                    }
                }
            });
        });
    }

    fn draw_macro_output_widget(&mut self, ui: &mut egui::Ui) {
        ui.add(MacroView::new(
            &mut self.actions,
            &mut self.macro_view_config,
            self.locale,
        ));
    }

    fn experimental_warning_text() -> Cow<'static, str> {
        #[cfg(not(target_arch = "wasm32"))]
        return t!("warning.experimental_feature.native");
        #[cfg(target_arch = "wasm32")]
        return t!("warning.experimental_feature.wasm");
    }

    #[cfg(target_arch = "wasm32")]
    fn load_fonts_dyn(&self, ctx: &egui::Context) {
        if self.locale == Locale::JP {
            let uri = concat!(
                env!("BASE_URL"),
                "/fonts/M_PLUS_1_Code/static/MPLUS1Code-Regular.ttf"
            );
            load_font_dyn(ctx, "MPLUS1Code-Regular", uri);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_font_dyn(ctx: &egui::Context, font_name: &str, uri: &str) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    let id = egui::Id::new(format!("{} loaded", uri));
    if ctx.data(|data| data.get_temp(id).unwrap_or(false)) {
        return;
    }
    if let Ok(egui::load::BytesPoll::Ready { bytes, .. }) = ctx.try_load_bytes(uri) {
        ctx.add_font(FontInsert::new(
            font_name,
            egui::FontData::from_owned(bytes.to_vec()),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: FontPriority::Lowest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: FontPriority::Lowest,
                },
            ],
        ));
        ctx.data_mut(|data| *data.get_temp_mut_or_default(id) = true);
        log::debug!("Font loaded: {}", font_name);
    };
}

fn load_fonts(ctx: &egui::Context) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    ctx.add_font(FontInsert::new(
        "XIV_Icon_Recreations",
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/XIV_Icon_Recreations/XIV_Icon_Recreations.ttf"
        )),
        vec![
            InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: FontPriority::Lowest,
            },
            InsertFontFamily {
                family: egui::FontFamily::Monospace,
                priority: FontPriority::Lowest,
            },
        ],
    ));
    #[cfg(not(target_arch = "wasm32"))]
    ctx.add_font(FontInsert::new(
        "MPLUS1Code-Regular",
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/M_PLUS_1_Code/static/MPLUS1Code-Regular.ttf"
        )),
        vec![
            InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: FontPriority::Lowest,
            },
            InsertFontFamily {
                family: egui::FontFamily::Monospace,
                priority: FontPriority::Lowest,
            },
        ],
    ));
}
