use game_data::{Item, Locale};
use simulator::{Action, Settings, SimulationState};

use crate::{
    app::SolverConfig,
    config::{CrafterConfig, QualityTarget},
};

use super::{HelpText, util};

pub struct Simulator<'a> {
    settings: &'a Settings,
    initial_quality: u16,
    solver_config: SolverConfig,
    crafter_config: &'a CrafterConfig,
    actions: &'a [Action],
    item: &'a Item,
    locale: Locale,
}

impl<'a> Simulator<'a> {
    pub fn new(
        settings: &'a Settings,
        initial_quality: u16,
        solver_config: SolverConfig,
        crafter_config: &'a CrafterConfig,
        actions: &'a [Action],
        item: &'a Item,
        locale: Locale,
    ) -> Self {
        Self {
            settings,
            initial_quality,
            solver_config,
            crafter_config,
            actions,
            item,
            locale,
        }
    }
}

impl Simulator<'_> {
    fn config_changed(&self, ctx: &egui::Context) -> bool {
        ctx.data(|data| {
            match data.get_temp::<(Settings, u16, SolverConfig)>(egui::Id::new("LAST_SOLVE_PARAMS"))
            {
                Some((settings, initial_quality, solver_config)) => {
                    settings != *self.settings
                        || initial_quality != self.initial_quality
                        || solver_config != self.solver_config
                }
                None => false,
            }
        })
    }

    fn draw_simulation(&self, ui: &mut egui::Ui, state: &SimulationState) {
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Simulation").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_visible(
                            !self.actions.is_empty() && self.config_changed(ui.ctx()),
                            egui::Label::new(
                                egui::RichText::new(
                                    "⚠ Some parameters have changed since last solve.",
                                )
                                .small()
                                .color(ui.visuals().warn_fg_color),
                            ),
                        );
                    });
                });

                ui.separator();

                let progress_text_width = text_width(ui, "Progress");
                let quality_text_width = text_width(ui, "Quality");
                let durability_text_width = text_width(ui, "Durability");
                let cp_text_width = text_width(ui, "CP");

                let max_text_width = progress_text_width
                    .max(quality_text_width)
                    .max(durability_text_width)
                    .max(cp_text_width);

                let text_size = egui::vec2(max_text_width, ui.spacing().interact_size.y);
                let text_layout = egui::Layout::right_to_left(egui::Align::Center);

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("Progress");
                    });
                    ui.add(
                        egui::ProgressBar::new(
                            state.progress as f32 / self.settings.max_progress as f32,
                        )
                        .text(progress_bar_text(
                            state.progress,
                            self.settings.max_progress,
                        ))
                        .corner_radius(0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("Quality");
                    });
                    let quality = self.initial_quality + state.quality;
                    ui.add(
                        egui::ProgressBar::new(quality as f32 / self.settings.max_quality as f32)
                            .text(progress_bar_text(quality, self.settings.max_quality))
                            .corner_radius(0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("Durability");
                    });
                    ui.add(
                        egui::ProgressBar::new(
                            state.durability as f32 / self.settings.max_durability as f32,
                        )
                        .text(progress_bar_text(
                            state.durability,
                            self.settings.max_durability,
                        ))
                        .corner_radius(0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("CP");
                    });
                    ui.add(
                        egui::ProgressBar::new(state.cp as f32 / self.settings.max_cp as f32)
                            .text(progress_bar_text(state.cp, self.settings.max_cp))
                            .corner_radius(0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.with_layout(text_layout, |ui| {
                        ui.set_height(ui.style().spacing.interact_size.y);
                        ui.add(HelpText::new(match self.settings.adversarial {
                            true => "Calculated assuming worst possible sequence of conditions",
                            false => "Calculated assuming Normal conditon on every step",
                        }));
                        if !state.is_final(self.settings) {
                            // do nothing
                        } else if state.progress < self.settings.max_progress {
                            ui.label("Synthesis failed");
                        } else if self.item.always_collectable {
                            let (t1, t2, t3) = (
                                QualityTarget::CollectableT1.get_target(self.settings.max_quality),
                                QualityTarget::CollectableT2.get_target(self.settings.max_quality),
                                QualityTarget::CollectableT3.get_target(self.settings.max_quality),
                            );
                            let tier = match self.initial_quality + state.quality {
                                quality if quality >= t3 => 3,
                                quality if quality >= t2 => 2,
                                quality if quality >= t1 => 1,
                                _ => 0,
                            };
                            ui.label(format!("Tier {} collectable", tier));
                        } else {
                            let hq = game_data::hq_percentage(
                                self.initial_quality + state.quality,
                                self.settings.max_quality,
                            );
                            ui.label(format!("{}% HQ", hq));
                        }
                    });
                });
            });
        });
    }

    fn draw_actions(&self, ui: &mut egui::Ui, errors: &[Result<(), &str>]) {
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.set_height(30.0);
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    for (action, error) in self.actions.iter().zip(errors.iter()) {
                        let image =
                            util::get_action_icon(*action, self.crafter_config.selected_job)
                                .fit_to_exact_size(egui::Vec2::new(30.0, 30.0))
                                .corner_radius(4.0)
                                .tint(match error {
                                    Ok(_) => egui::Color32::WHITE,
                                    Err(_) => egui::Color32::DARK_GRAY,
                                });
                        let response = ui
                            .add(image)
                            .on_hover_text(game_data::action_name(*action, self.locale));
                        if error.is_err() {
                            egui::Image::new(egui::include_image!(
                                "../../assets/action-icons/disabled.webp"
                            ))
                            .tint(egui::Color32::GRAY)
                            .paint_at(ui, response.rect);
                        }
                    }
                });
            });
        });
    }
}

impl egui::Widget for Simulator<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (state, errors) =
            SimulationState::from_macro_continue_on_error(self.settings, self.actions);
        ui.vertical(|ui| {
            self.draw_simulation(ui, &state);
            self.draw_actions(ui, &errors);
        })
        .response
    }
}

fn text_width(ui: &mut egui::Ui, text: impl Into<String>) -> f32 {
    ui.fonts(|fonts| {
        let galley = fonts.layout_no_wrap(
            text.into(),
            egui::FontId::default(),
            egui::Color32::default(),
        );
        galley.rect.width()
    })
}

fn progress_bar_text<T: Copy + std::cmp::Ord + std::ops::Sub<Output = T> + std::fmt::Display>(
    value: T,
    maximum: T,
) -> String {
    if value > maximum {
        let overflow = value - maximum;
        format!("{: >5} / {}  (+{} overflow)", value, maximum, overflow)
    } else {
        format!("{: >5} / {}", value, maximum)
    }
}
