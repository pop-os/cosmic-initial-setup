use cosmic::{cosmic_theme, theme, widget, Element};

use crate::{fl, page};

static LAUNCHER_SVG: &'static [u8] = include_bytes!("../../res/launcher.svg");

pub struct LauncherPage {
    handle: widget::svg::Handle,
}

impl LauncherPage {
    pub fn new() -> Self {
        Self {
            handle: widget::svg::Handle::from_memory(LAUNCHER_SVG),
        }
    }
}

impl page::Page for LauncherPage {
    fn title(&self) -> String {
        fl!("fast-and-efficient")
    }

    fn view(&self) -> Element<page::Message> {
        let cosmic_theme::Spacing { space_s, .. } = theme::active().cosmic().spacing;

        widget::column::with_children(vec![
            widget::text::body(fl!("launcher-description")).into(),
            widget::svg(self.handle.clone()).into(),
        ])
        .spacing(space_s)
        .into()
    }
}
