use crate::fl;
use bytes::Bytes;
use cosmic::{
    Apply, cosmic_theme,
    iced::{Alignment, Length},
    widget::{self, image},
};
use std::{any::Any, sync::LazyLock};

static SCREENSHOT: LazyLock<image::Handle> = LazyLock::new(|| {
    let embedded_bytes = include_bytes!("../../res/new-apps.png");
    image::Handle::from_bytes(Bytes::from_static(embedded_bytes))
});

#[derive(Default)]
pub struct Page;

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

    fn view(&self) -> cosmic::Element<super::Message> {
        let cosmic_theme::Spacing { space_s, .. } = cosmic::theme::active().cosmic().spacing;

        let description = widget::text::body(fl!("new-apps-page", "description"))
            .align_x(cosmic::iced::Alignment::Center)
            .width(Length::Fill);

        let screenshot = widget::image(&*SCREENSHOT).filter_method(image::FilterMethod::Linear);

        widget::column::with_capacity(2)
            .push(description)
            .push(screenshot)
            .align_x(Alignment::Center)
            .spacing(space_s)
            .padding(0)
            .into()
    }
}
