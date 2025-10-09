use std::cmp;

use cosmic::{
    Element, Task,
    cosmic_config::{self, ConfigGet, ConfigSet},
    cosmic_theme,
    iced::Alignment,
    theme, widget,
};
use cosmic_comp_config::{KeyboardConfig, XkbConfig};
use slotmap::{DefaultKey, SlotMap};

use crate::{fl, page};

pub type Locale = String;
pub type Variant = String;
pub type Description = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutSource {
    Base,
    Extra,
}

#[derive(Clone, Debug)]
pub enum Message {
    Search(String),
    Select(DefaultKey),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::Keyboard(message).into()
    }
}

pub struct Page {
    search_id: widget::Id,
    search: String,
    regex_opt: Option<regex::Regex>,
    selected_opt: Option<DefaultKey>,
    config: cosmic_config::Config,
    xkb: XkbConfig,
    keyboard_config: KeyboardConfig,
    keyboard_layouts: SlotMap<DefaultKey, (Locale, Variant, Description, LayoutSource)>,
    active_layouts: Vec<DefaultKey>,
}

impl Page {
    pub fn new() -> Self {
        let config = cosmic_config::Config::new("com.system76.CosmicComp", 1).unwrap();

        Self {
            search_id: widget::Id::unique(),
            search: String::new(),
            regex_opt: None,
            selected_opt: None,
            keyboard_layouts: SlotMap::new(),
            active_layouts: Vec::new(),
            xkb: XkbConfig::default(),
            keyboard_config: KeyboardConfig::default(),
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
                self.active_layouts = vec![selected];
                self.update_xkb_config();
            }
        }
        Task::none()
    }

    fn update_xkb_config(&mut self) {
        fn update_xkb_config(
            config: &cosmic_config::Config,
            xkb: &mut XkbConfig,
            active_layouts: &mut dyn Iterator<Item = (&str, &str)>,
        ) -> Result<(), cosmic_config::Error> {
            let mut new_layout = String::new();
            let mut new_variant = String::new();

            for (locale, variant) in active_layouts {
                new_layout.push_str(locale);
                new_layout.push(',');
                new_variant.push_str(variant);
                new_variant.push(',');
            }

            let _excess_comma = new_layout.pop();
            let _excess_comma = new_variant.pop();

            xkb.layout = new_layout;
            xkb.variant = new_variant;

            config.set("xkb_config", xkb)
        }

        let result = update_xkb_config(
            &self.config,
            &mut self.xkb,
            &mut self
                .active_layouts
                .iter()
                .filter_map(|id| self.keyboard_layouts.get(*id))
                .map(|(locale, variant, _description, _source)| {
                    (locale.as_str(), variant.as_str())
                }),
        );

        if let Err(why) = result {
            tracing::error!(?why, "Failed to set config 'xkb_config'");
        }
    }
}

impl page::Page for Page {
    fn title(&self) -> String {
        fl!("keyboard-layout-page")
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self) -> Task<page::Message> {
        self.xkb = self.config.get("xkb_config").unwrap_or_else(|why| {
            if why.is_err() {
                tracing::error!(?why, "Failed to read xkb_config");
            }

            XkbConfig::default()
        });

        self.keyboard_config = self.config.get("keyboard_config").unwrap_or_else(|why| {
            if why.is_err() {
                tracing::error!(?why, "Failed to read keyboard_config");
            }

            KeyboardConfig::default()
        });

        match (
            xkb_data::keyboard_layouts(),
            xkb_data::extra_keyboard_layouts(),
        ) {
            (Ok(base_layouts), Ok(extra_layouts)) => {
                self.active_layouts.clear();
                self.keyboard_layouts.clear();

                let mut sorted_layouts = base_layouts
                    .layouts()
                    .iter()
                    .map(|layout| (layout, LayoutSource::Base))
                    .chain(
                        extra_layouts
                            .layouts()
                            .iter()
                            .map(|layout| (layout, LayoutSource::Extra)),
                    )
                    .collect::<Vec<_>>();

                sorted_layouts.sort_unstable_by(|(a, _), (b, _)| {
                    match (a.name(), b.name()) {
                        // Place US at the top of the list as it's the default
                        ("us", _) => cmp::Ordering::Less,
                        (_, "us") => cmp::Ordering::Greater,
                        // Place custom at the bottom
                        ("custom", _) => cmp::Ordering::Greater,
                        (_, "custom") => cmp::Ordering::Less,
                        // Compare everything else by description because it looks nicer (e.g. all
                        // English grouped together)
                        _ => a
                            .description()
                            .partial_cmp(b.description())
                            .expect("`str` is always comparable"),
                    }
                });

                for (layout, source) in sorted_layouts {
                    self.keyboard_layouts.insert((
                        layout.name().to_owned(),
                        String::new(),
                        gettextrs::dgettext("xkeyboard-config", layout.description()),
                        source.clone(),
                    ));

                    if let Some(variants) = layout.variants().map(|variants| {
                        variants.iter().map(|variant| {
                            (
                                layout.name().to_owned(),
                                variant.name().to_owned(),
                                gettextrs::dgettext("xkeyboard-config", variant.description()),
                                source.clone(),
                            )
                        })
                    }) {
                        let mut variants: Vec<_> = variants.collect();
                        variants.sort_unstable_by(|(_, _, desc_a, _), (_, _, desc_b, _)| {
                            desc_a
                                .partial_cmp(desc_b)
                                .expect("`str` is always comparable")
                        });

                        for (layout_name, name, description, source) in variants {
                            self.keyboard_layouts
                                .insert((layout_name, name, description, source));
                        }
                    }
                }

                // Xkb layouts currently enabled.
                let layouts = if self.xkb.layout.is_empty() {
                    "us"
                } else {
                    &self.xkb.layout
                }
                .split_terminator(',');

                // Xkb variants for each layout. Repeat empty strings in case there's more layouts than variants.
                let variants = self
                    .xkb
                    .variant
                    .split_terminator(',')
                    .chain(std::iter::repeat(""));

                for (layout, variant) in layouts.zip(variants) {
                    for (id, (xkb_layout, xkb_variant, _desc, _source)) in &self.keyboard_layouts {
                        if layout == xkb_layout && variant == xkb_variant {
                            self.active_layouts.push(id);
                            break;
                        }
                    }
                }

                self.selected_opt = self.active_layouts.first().cloned();
            }
            (Err(why), _) | (_, Err(why)) => {
                tracing::error!(?why, "failed to get keyboard layouts");
            }
        }

        Task::none()
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

        let mut list = widget::list_column();

        for (id, (_locale, variant, description, _source)) in &self.keyboard_layouts {
            if self
                .regex_opt
                .as_ref()
                .is_none_or(|re| re.is_match(description))
            {
                let selected = Some(id) == self.selected_opt;
                let item = widget::settings::item::builder(description).control(
                    widget::row::with_children(vec![
                        widget::text::body(variant).into(),
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
                );

                //TODO: properly style this
                let input_source = widget::button::custom(item)
                    .on_press(Message::Select(id))
                    .class(if selected {
                        theme::Button::Link
                    } else {
                        theme::Button::MenuRoot
                    });

                list = list.add(input_source);
            }
        }

        let search_input = widget::search_input(fl!("type-to-search"), &self.search)
            .id(self.search_id.clone())
            .on_input(Message::Search);

        let element: Element<_> = widget::column::with_children(vec![
            search_input.into(),
            widget::Space::with_height(space_m).into(),
            widget::scrollable(list).into(),
        ])
        .into();
        element.map(page::Message::Keyboard)
    }
}
