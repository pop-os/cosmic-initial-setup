use crate::fl;
use cosmic::{
    cosmic_theme,
    iced::{Alignment, Length},
    widget,
};
use std::any::Any;

static SCREENSHOT: &'static [u8] = include_bytes!("../../res/new-apps.svg");

pub struct Page {
    handle: widget::svg::Handle,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            handle: widget::svg::Handle::from_memory(SCREENSHOT),
        }
    }
}

impl super::Page for Page {
    fn title(&self) -> String {
        fl!("new-apps-page")
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn skippable(&self) -> bool {
        true
    }

    fn view(&self) -> cosmic::Element<'_, super::Message> {
        let cosmic_theme::Spacing { space_s, .. } = cosmic::theme::active().cosmic().spacing;

        let description = widget::text::body(fl!("new-apps-page", "description"))
            .align_x(cosmic::iced::Alignment::Center)
            .width(Length::Fill);

        widget::column::with_capacity(2)
            .push(description)
            .push(widget::svg(self.handle.clone()).width(Length::Fill))
            .align_x(Alignment::Center)
            .spacing(space_s)
            .padding(0)
            .into()
    }
}
