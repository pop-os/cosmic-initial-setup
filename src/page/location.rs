use crate::fl;
use crate::page;
use cosmic::{
    Element, Task, cosmic_theme, iced::Alignment, theme, widget,
    cosmic_config::{self, Config, ConfigSet, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
};
use serde::{Deserialize, Serialize};

static CITIES: &'static [u8] = include_bytes!("../../res/cities.bitcode-v0-6");

const CONFIG_NAME: &str = "com.system76.CosmicInitialSetup";

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, CosmicConfigEntry)]
pub struct LocationState {
    pub city_name: String,
    pub timezone: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl LocationState {
    pub fn version() -> u64 {
        1
    }

    pub fn state() -> Result<Config, cosmic_config::Error> {
        Config::new_state(CONFIG_NAME, Self::version())
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Search(String),
    Select(usize),
}

pub struct Page {
    cities: Vec<geonames::City>,
    selected_opt: Option<usize>,
    search_id: widget::Id,
    search: String,
    regex_opt: Option<regex::Regex>,
    config: Option<Config>,
}

impl Page {
    pub fn new() -> Self {
        let cities = match geonames::bitcode::decode(CITIES) {
            Ok(ok) => ok,
            Err(err) => {
                tracing::warn!(err = err.to_string(), "failed to decode cities");
                Vec::new()
            }
        };
        let config = LocationState::state().ok();
        Self {
            cities,
            selected_opt: None,
            search_id: widget::Id::unique(),
            search: String::new(),
            regex_opt: None,
            config,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<page::Message> {
        match message {
            Message::Search(search) => {
                self.selected_opt = None;
                self.search = search;
                self.regex_opt = None;
                if !self.search.is_empty() {
                    let pattern = regex::escape(&self.search);
                    match regex::RegexBuilder::new(&pattern)
                        .case_insensitive(true)
                        .build()
                    {
                        Ok(regex) => self.regex_opt = Some(regex),
                        Err(err) => {
                            tracing::warn!(
                                err = err.to_string(),
                                "failed to parse regex {:?}",
                                pattern
                            );
                        }
                    };
                }
            }
            Message::Select(selected) => {
                self.selected_opt = Some(selected);

                if let Some(city) = self.cities.get(selected) {
                    let timezone = city.timezone.clone();

                    if let Some(ref config) = self.config {
                        let location_state = LocationState {
                            city_name: city.name.to_string(),
                            timezone: city.timezone.to_string(),
                            latitude: city.latitude,
                            longitude: city.longitude,
                        };

                        if let Err(err) = config.set("selected_location", &location_state) {
                            tracing::warn!(err = err.to_string(), "failed to save location state");
                        }
                    }

                    tokio::spawn(async move {
                        _ = tokio::process::Command::new("timedatectl")
                            .args(&["set-timezone", &timezone])
                            .status()
                            .await;
                    });
                }
            }
        }
        Task::none()
    }
}

impl page::Page for Page {
    fn title(&self) -> String {
        fl!("timezone-and-location-page")
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn open(&mut self) -> cosmic::Task<page::Message> {
        return widget::text_input::focus(self.search_id.clone());
    }

    fn completed(&self) -> bool {
        self.selected_opt.is_some()
    }

    fn view(&self) -> Element<'_, page::Message> {
        let cosmic_theme::Spacing {
            space_xxs, space_m, ..
        } = theme::active().cosmic().spacing;

        let mut section = widget::settings::section();
        //TODO: run search outside of view!
        let mut first_opt = None;
        for (i, name, desc_opt, timezone) in self
            .cities
            .iter()
            .enumerate()
            .filter_map(|(i, city)| {
                let Some(regex) = &self.regex_opt else {
                    return Some((i, &city.name, None, &city.timezone));
                };
                //TODO: better search method (fuzzy search?), show alternate names
                if regex.is_match(&city.name) {
                    return Some((i, &city.name, None, &city.timezone));
                }
                for alternate_name in &city.alternate_names {
                    if regex.is_match(alternate_name) {
                        return Some((i, alternate_name, Some(&city.name), &city.timezone));
                    }
                }
                None
            })
            .take(100)
        {
            let mut item = widget::settings::item::builder(&**name);
            if let Some(desc) = desc_opt {
                item = item.description(&**desc);
            }
            let selected = Some(i) == self.selected_opt;
            section = section.add(
                //TODO: properly style this
                widget::button::custom(
                    item.control(
                        widget::row::with_children(vec![
                            widget::text::body(&**timezone).into(),
                            if selected {
                                widget::icon::from_name("object-select-symbolic")
                                    .size(16)
                                    .into()
                            } else {
                                widget::Space::with_width(16).into()
                            },
                        ])
                        .align_y(Alignment::Center)
                        .spacing(space_xxs),
                    ),
                )
                .on_press(Message::Select(i))
                .class(if selected {
                    theme::Button::Link
                } else {
                    theme::Button::MenuRoot
                }),
            );
            if first_opt.is_none() {
                first_opt = Some(i);
            }
        }
        let mut search_input = widget::search_input(
            fl!(
                "timezone-and-location-page",
                "search-the-closest-major-city"
            ),
            &self.search,
        )
        .id(self.search_id.clone())
        .on_input(Message::Search);
        if self.selected_opt.is_some() {
            // Go to next page if an item is selected
            //TODO: search_input = search_input.on_submit(Message::Next);
        } else if let Some(first) = first_opt {
            if self.regex_opt.is_some() {
                // Select first item if no item is selected and there is a search
                search_input = search_input.on_submit(move |_| Message::Select(first));
            }
        }
        let element: Element<_> = widget::column::with_children(vec![
            search_input.into(),
            widget::Space::with_height(space_m).into(),
            //TODO: manual height used due to layout issues
            widget::scrollable(section).height(286).into(),
            widget::Space::with_height(space_m).into(),
            widget::text::body(fl!("timezone-and-location-page", "geonames-attribution")).into(),
        ])
        .into();
        element.map(page::Message::Location)
    }
}
