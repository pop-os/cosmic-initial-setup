use cosmic::{cosmic_theme, iced::Alignment, theme, widget, Element, Task};

use crate::{fl, page};

static COSMIC_DARK_SVG: &'static [u8] = include_bytes!("../../res/cosmic-dark.svg");
static COSMIC_LIGHT_SVG: &'static [u8] = include_bytes!("../../res/cosmic-light.svg");

struct Theme {
    name: String,
    handle: widget::svg::Handle,
}

#[derive(Clone, Debug)]
pub enum Message {
    Select(usize),
}

pub struct AppearancePage {
    themes: Vec<Theme>,
    selected: usize,
}

impl AppearancePage {
    pub fn new() -> Self {
        Self {
            themes: vec![
                Theme {
                    name: "COSMIC dark".to_string(),
                    handle: widget::svg::Handle::from_memory(COSMIC_DARK_SVG),
                },
                Theme {
                    name: "COSMIC light".to_string(),
                    handle: widget::svg::Handle::from_memory(COSMIC_LIGHT_SVG),
                },
            ],
            selected: 0,
        }
    }
}

impl page::Page for AppearancePage {
    fn title(&self) -> String {
        fl!("personalize-appearance")
    }

    fn update(&mut self, page_message: page::Message) -> Task<page::Message> {
        let message = match page_message {
            page::Message::Appearance(message) => message,
            _ => return Task::none(),
        };
        match message {
            Message::Select(index) => {
                self.selected = index;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<page::Message> {
        let cosmic_theme::Spacing {
            space_xxs, space_m, ..
        } = theme::active().cosmic().spacing;

        let mut grid = widget::grid().column_spacing(space_m).row_spacing(space_m);
        for (i, theme) in self.themes.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                grid = grid.insert_row();
            }
            grid = grid.push(
                widget::column::with_children(vec![
                    widget::button::custom(widget::svg(theme.handle.clone()).width(144).height(81))
                        .class(theme::Button::Image)
                        .selected(i == self.selected)
                        .on_press(Message::Select(i))
                        .into(),
                    widget::text::body(&theme.name).into(),
                ])
                .spacing(space_xxs)
                .align_x(Alignment::Center),
            );
        }

        let element: Element<_> = grid.into();
        element.map(page::Message::Appearance)
    }
}
