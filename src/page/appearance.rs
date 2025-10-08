use cosmic::{
    Element, Task,
    cosmic_config::{Config, ConfigSet, CosmicConfigEntry},
    cosmic_theme::{self, ThemeBuilder, ThemeMode},
    iced::{Alignment, Length},
    theme::{self, ThemeType},
    widget,
};
use heck::ToTitleCase;
use std::{collections::BTreeSet, sync::Arc};
use std::{ffi::OsStr, io::Read};

use crate::{fl, page};

static COSMIC_DARK_PNG: &'static [u8] = include_bytes!("../../res/cosmic-dark.png");
static COSMIC_LIGHT_PNG: &'static [u8] = include_bytes!("../../res/cosmic-light.png");

fn dark_icon() -> widget::image::Handle {
    widget::image::Handle::from_bytes(COSMIC_DARK_PNG)
}

fn light_icon() -> widget::image::Handle {
    widget::image::Handle::from_bytes(COSMIC_LIGHT_PNG)
}

struct Theme {
    name: String,
    builder: ThemeBuilder,
    handle: widget::image::Handle,
    is_dark: bool,
}

impl PartialEq for Theme {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl PartialOrd for Theme {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Eq for Theme {}

impl Ord for Theme {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Select(usize),
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

        let mut themes = vec![
            Theme {
                name: "COSMIC Dark".to_string(),
                handle: dark_icon(),
                builder: ThemeBuilder::dark(),
                is_dark: true,
            },
            Theme {
                name: "COSMIC Light".to_string(),
                handle: light_icon(),
                builder: ThemeBuilder::light(),
                is_dark: false,
            },
        ];

        #[cfg(feature = "nixos")]
        let themes_dir_path = "/run/current-system/sw/share/cosmic-themes/";
        #[cfg(not(feature = "nixos"))]
        let themes_dir_path = "/usr/share/cosmic-themes/";
        if let Ok(directory) = std::fs::read_dir(themes_dir_path) {
            let mut extra_themes = BTreeSet::new();
            let mut buffer = Vec::with_capacity(8 * 1024);
            for entry in directory.filter_map(Result::ok) {
                let path = entry.path();

                if path.extension() != Some(OsStr::new("ron")) {
                    continue;
                }

                let file_stem = path.file_stem();

                let Some(name) = file_stem.and_then(|x| x.to_str()) else {
                    continue;
                };

                let Ok(mut file) = std::fs::File::open(&path) else {
                    continue;
                };

                buffer.clear();
                let Ok(read) = file.read_to_end(&mut buffer) else {
                    continue;
                };

                match ron::de::from_bytes::<ThemeBuilder>(&buffer[..read]) {
                    Ok(builder) => {
                        let is_dark = name.ends_with("dark");
                        extra_themes.insert(Theme {
                            name: name.replace('-', " ").to_title_case(),
                            handle: widget::image::Handle::from_path({
                                let mut icon_path = path.clone();
                                icon_path.set_extension("png");
                                icon_path
                            }),
                            builder,
                            is_dark,
                        });
                    }

                    Err(why) => {
                        tracing::error!(?why, "failed to parse theme");
                    }
                }
            }

            themes.extend(extra_themes.into_iter());
        }

        Self {
            theme_mode_config,
            dark_theme_config: cosmic::cosmic_theme::Theme::dark_config().ok(),
            dark_theme_builder_config: ThemeBuilder::dark_config().ok(),
            light_theme_config: cosmic::cosmic_theme::Theme::light_config().ok(),
            light_theme_builder_config: ThemeBuilder::light_config().ok(),
            themes,
            selected: if ThemeMode::default().is_dark { 0 } else { 1 },
        }
    }

    pub fn update(&mut self, message: Message) -> Task<page::Message> {
        match message {
            Message::Select(index) => {
                let Some(selected_theme) = self.themes.get(index) else {
                    return Task::none();
                };

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

                if let Some(theme_mode_config) = &self.theme_mode_config {
                    if let Err(why) = theme_mode_config.set("is_dark", selected_theme.is_dark) {
                        tracing::warn!(why = why.to_string(), "failed to set theme mode");
                    }
                }

                return cosmic::Task::done(page::Message::SetTheme(cosmic::Theme {
                    theme_type: ThemeType::Custom(Arc::new(theme)),
                    ..cosmic::Theme::default()
                }));
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
            space_s,
            space_m,
            space_xl,
            ..
        } = theme::active().cosmic().spacing;

        let mut grid = widget::grid().column_spacing(space_m).row_spacing(space_m);
        for (i, theme) in self.themes.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                grid = grid.insert_row();
            }

            let thumbnail = widget::image(theme.handle.clone()).width(144).height(81);

            let button = widget::button::custom_image_button(thumbnail, None)
                .class(theme::Button::Image)
                .selected(i == self.selected)
                .on_press(Message::Select(i).into());

            let selection = widget::column::with_capacity(2)
                .push(button)
                .push(widget::text::body(&theme.name))
                .spacing(space_s)
                .align_x(Alignment::Center);

            grid = grid.push(selection);
        }

        let description = widget::text::body(fl!("appearance-page", "description"))
            .align_x(cosmic::iced::Alignment::Center)
            .width(Length::Fill);

        widget::column::with_capacity(2)
            .push(widget::scrollable(grid))
            .push(description)
            .align_x(Alignment::Center)
            .spacing(space_xl)
            .into()
    }
}
