use cosmic::{cosmic_theme, iced::Alignment, theme, widget, Element, Task};

use crate::{fl, page};

static CITIES: &'static [u8] = include_bytes!("../../res/cities.bitcode-v0-6");

#[derive(Clone, Debug)]
pub enum Message {
    Search(String),
    Select(usize),
}

pub struct LocationPage {
    cities: Vec<geonames::City>,
    selected_opt: Option<usize>,
    search_id: widget::Id,
    search: String,
    regex_opt: Option<regex::Regex>,
}

impl LocationPage {
    pub fn new() -> Self {
        let cities = match geonames::bitcode::decode(CITIES) {
            Ok(ok) => ok,
            Err(err) => {
                log::warn!("failed to decode cities: {}", err);
                Vec::new()
            }
        };
        Self {
            cities,
            selected_opt: None,
            search_id: widget::Id::unique(),
            search: String::new(),
            regex_opt: None,
        }
    }
}

impl page::Page for LocationPage {
    fn title(&self) -> String {
        fl!("timezone-and-location")
    }

    fn completed(&self) -> bool {
        self.selected_opt.is_some()
    }

    fn view(&self) -> Element<page::Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            ..
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
            let mut item = widget::settings::item::builder(name);
            if let Some(desc) = desc_opt {
                item = item.description(desc);
            }
            let selected = Some(i) == self.selected_opt;
            section = section.add(
                //TODO: properly style this
                widget::button::custom(
                    item.control(
                        widget::row::with_children(vec![
                            widget::text::body(timezone).into(),
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
        let mut search_input =
            widget::search_input(fl!("search-the-closest-major-city"), &self.search)
                .id(self.search_id.clone())
                .on_input(Message::Search);
        if self.selected_opt.is_some() {
            // Go to next page if an item is selected
            //TODO: search_input = search_input.on_submit(Message::Next);
        } else if let Some(first) = first_opt {
            if self.regex_opt.is_some() {
                // Select first item if no item is selected and there is a search
                search_input = search_input.on_submit(Message::Select(first));
            }
        }
        let element: Element<_> = widget::column::with_children(vec![
            search_input.into(),
            widget::Space::with_height(space_s).into(),
            //TODO: fix layout issues and move below list
            widget::text::caption(fl!("geonames-attribution")).into(),
            widget::Space::with_height(space_m).into(),
            widget::scrollable(section).into(),
        ])
        .into();
        element.map(page::Message::Location)
    }

    fn update(&mut self, page_message: page::Message) -> Task<page::Message> {
        let message = match page_message {
            page::Message::Open => {
                return widget::text_input::focus(self.search_id.clone());
            }
            page::Message::Location(message) => message,
            _ => return Task::none(),
        };
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
                            log::warn!("failed to parse regex {:?}: {}", pattern, err);
                        }
                    };
                }
            }
            Message::Select(selected) => self.selected_opt = Some(selected),
        }
        Task::none()
    }
}
