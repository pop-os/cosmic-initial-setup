use cosmic::{Element, cosmic_theme, theme, widget};

use crate::{fl, page};

static LAUNCHER_SVG: &'static [u8] = include_bytes!("../../res/launcher.svg");

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

    fn view(&self) -> Element<page::Message> {
        let cosmic_theme::Spacing { space_s, .. } = theme::active().cosmic().spacing;

        widget::column::with_children(vec![
            widget::text::body(fl!("launcher-page", "description")).into(),
            widget::svg(self.handle.clone()).into(),
        ])
        .spacing(space_s)
        .into()
    }
}
