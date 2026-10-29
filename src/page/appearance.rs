mod preview;

use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet, CosmicConfigEntry};
use cosmic::cosmic_theme::{self, Theme as CosmicTheme, ThemeBuilder, ThemeMode};
use cosmic::iced::core::text::LineHeight;
use cosmic::iced::widget::{rich_text, span};
use cosmic::iced::{Alignment, Length, alignment, never};
use cosmic::theme::{self, ThemeType};
use cosmic::widget::ListColumn;
use cosmic::{Element, Task, widget};
use cosmic_theme_bundle_config::bundle::{self, Part};
use std::sync::Arc;

use crate::{fl, page};

struct Theme {
    name: String,
    builder: ThemeBuilder,
    theme: CosmicTheme,
    is_dark: bool,
    slug: Option<String>,
}

fn load_theme(bundle: bundle::Bundle) -> Option<Theme> {
    let theme_config = Config::system(
        &bundle::bundle_id(&bundle.slug, Part::Theme),
        Part::Theme.version(),
    )
    .ok()?;
    let builder_config = Config::system(
        &bundle::bundle_id(&bundle.slug, Part::Builder),
        Part::Builder.version(),
    )
    .ok()?;
    let theme = theme_config.get::<CosmicTheme>("base").ok()?;
    let builder = builder_config.get::<ThemeBuilder>("base").ok()?;
    let is_dark = theme.is_dark;
    Some(Theme {
        name: theme.name.clone(),
        builder,
        theme,
        is_dark,
        slug: Some(bundle.slug),
    })
}

#[derive(Clone, Debug)]
pub enum Message {
    Select(usize),
    ToggleFrosted(bool),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::Appearance(message)
    }
}

pub struct Page {
    theme_mode_config: Option<Config>,
    light_theme_config: Option<Config>,
    light_theme_builder_config: Option<Config>,
    dark_theme_config: Option<Config>,
    dark_theme_builder_config: Option<Config>,
    themes: Vec<Theme>,
    selected: usize,
    frosted_glass: bool,
}

impl Page {
    pub fn new() -> Self {
        let theme_mode_config = match ThemeMode::config() {
            Ok(config) => Some(config),
            Err(err) => {
                tracing::warn!(err = err.to_string(), "failed to get theme mode config");
                None
            }
        };

        let themes: Vec<_> = bundle::system_bundles()
            .unwrap_or_default()
            .into_iter()
            .filter_map(load_theme)
            .collect();

        let theme_mode = ThemeMode::default();
        let live_name = if theme_mode.is_dark {
            CosmicTheme::dark_config()
                .ok()
                .and_then(|config| config.get::<String>("name").ok())
        } else {
            CosmicTheme::light_config()
                .ok()
                .and_then(|config| config.get::<String>("name").ok())
        };
        let selected = live_name
            .and_then(|name| themes.iter().position(|theme| theme.name == name))
            .or_else(|| {
                themes
                    .iter()
                    .position(|theme| theme.is_dark == theme_mode.is_dark)
            })
            .unwrap_or(0);

        Self {
            theme_mode_config,
            dark_theme_config: CosmicTheme::dark_config().ok(),
            dark_theme_builder_config: ThemeBuilder::dark_config().ok(),
            light_theme_config: CosmicTheme::light_config().ok(),
            light_theme_builder_config: ThemeBuilder::light_config().ok(),
            themes,
            selected,
            frosted_glass: false,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<page::Message> {
        match message {
            Message::Select(index) => {
                let Some(selected_theme) = self.themes.get_mut(index) else {
                    return Task::none();
                };

                selected_theme.builder.frosted_applets = self.frosted_glass;
                selected_theme.builder.frosted_panel = self.frosted_glass;
                selected_theme.builder.frosted_system_interface = self.frosted_glass;
                selected_theme.builder.frosted_windows = self.frosted_glass;

                let theme = selected_theme.builder.clone().build();
                self.selected = index;

                let (builder_config, theme_config) = if selected_theme.is_dark {
                    (
                        self.dark_theme_builder_config.as_ref(),
                        self.dark_theme_config.as_ref(),
                    )
                } else {
                    (
                        self.light_theme_builder_config.as_ref(),
                        self.light_theme_config.as_ref(),
                    )
                };

                if let Some(builder_config) = builder_config {
                    _ = selected_theme.builder.write_entry(builder_config);
                }

                if let Some(theme_config) = theme_config {
                    _ = theme.write_entry(theme_config);
                }

                selected_theme.theme = theme.clone();

                if let Some(theme_mode_config) = &self.theme_mode_config
                    && let Err(why) = theme_mode_config.set("is_dark", selected_theme.is_dark)
                {
                    tracing::warn!(why = why.to_string(), "failed to set theme mode");
                }

                if let Some(slug) = &selected_theme.slug
                    && let Err(why) = bundle::apply_background(
                        slug,
                        bundle::Mode::from_is_dark(selected_theme.is_dark),
                    )
                {
                    tracing::warn!(why = why.to_string(), "failed to apply bundle background");
                }

                cosmic::Task::done(page::Message::SetTheme(cosmic::Theme {
                    theme_type: ThemeType::Custom(Arc::new(theme)),
                    ..cosmic::Theme::default()
                }))
            }
            Message::ToggleFrosted(enabled) => {
                self.frosted_glass = enabled;
                self.update(Message::Select(self.selected))
            }
        }
    }
}

impl page::Page for Page {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn title(&self) -> String {
        fl!("appearance-page")
    }

    fn skippable(&self) -> bool {
        true
    }

    fn view(&self) -> Element<'_, page::Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            space_xl,
            ..
        } = theme::spacing();

        let mut grid = widget::grid().column_spacing(space_m).row_spacing(space_m);
        for (i, theme) in self.themes.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                grid = grid.insert_row();
            }

            let thumbnail = preview::view(&theme.theme);

            let button = widget::button::custom_image_button(thumbnail, None)
                .class(theme::Button::Image)
                .description(&theme.name)
                .selected(i == self.selected)
                .on_press(Message::Select(i).into());

            let selection = widget::column::with_capacity(2)
                .push(button)
                .push(widget::text::body(&theme.name))
                .spacing(space_s)
                .align_x(Alignment::Center);

            grid = grid.push(selection);
        }

        let mut content = widget::column::with_capacity(2)
            .spacing(space_m)
            .align_x(alignment::Horizontal::Center);
        content = content.push(grid);
        let mut effects_column = widget::column::with_capacity(2)
            .spacing(space_xxs)
            .align_x(alignment::Horizontal::Center);
        effects_column = effects_column
            .push(widget::text::heading(fl!("appearance-page", "effects")).width(Length::Fill))
            .align_x(alignment::Horizontal::Center);
        let mut effects = widget::row::with_capacity(2).spacing(space_xxs);
        effects = effects.push(
            ListColumn::new()
                .list_item_padding([space_xxs + 1, space_m]) // attempts to match size of tip
                .add(
                    cosmic::widget::settings::item::builder(fl!(
                        "appearance-page",
                        "frosted-glass"
                    ))
                    .description(fl!("appearance-page", "frosted-description"))
                    .toggler(self.frosted_glass, |v| {
                        page::Message::Appearance(Message::ToggleFrosted(v))
                    }),
                ),
        );
        let t = theme::active();
        effects = effects.push(
            ListColumn::new().add(
                rich_text![
                    span(fl!("appearance-page", "tip"))
                        .color(t.cosmic().accent_text_color())
                        .size(14)
                        .line_height(LineHeight::Absolute(20.0.into()))
                        .font(cosmic::font::default()),
                    span(" ")
                        .size(14)
                        .line_height(LineHeight::Absolute(20.0.into()))
                        .font(cosmic::font::default()),
                    span(fl!("appearance-page", "effects-tip"))
                        .size(14)
                        .line_height(LineHeight::Absolute(20.0.into()))
                        .font(cosmic::font::default())
                ]
                .on_link_click(never),
            ),
        );
        effects_column = effects_column.push(effects);

        content = content.push(effects_column);

        widget::column::with_capacity(2)
            .push(widget::scrollable(content))
            .align_x(Alignment::Center)
            .spacing(space_xl)
            .into()
    }
}
