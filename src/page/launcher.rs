use cosmic::{
    Element, cosmic_theme,
    iced::{Alignment, Length},
    theme, widget,
};

use crate::{fl, page};

static LAUNCHER_SVG: &[u8] = include_bytes!("../../res/launcher.svg");

pub struct Page {
    handle: widget::svg::Handle,
}

impl Page {
    pub fn new() -> Self {
        Self {
            handle: widget::svg::Handle::from_memory(LAUNCHER_SVG),
        }
    }
}

impl page::Page for Page {
    fn title(&self) -> String {
        fl!("launcher-page")
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn skippable(&self) -> bool {
        true
    }

    fn view(&self) -> Element<'_, page::Message> {
        let cosmic_theme::Spacing { space_s, .. } = theme::active().cosmic().spacing;

        widget::column::with_children(vec![
            widget::text::body(fl!("launcher-page", "description"))
                .align_x(cosmic::iced::Alignment::Center)
                .width(Length::Fill)
                .into(),
            widget::svg(self.handle.clone()).width(Length::Fill).into(),
        ])
        .align_x(Alignment::Center)
        .spacing(space_s)
        .into()
    }
}
