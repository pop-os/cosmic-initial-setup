use cosmic::{widget, Element};

use crate::{fl, page};

pub struct WelcomePage;

impl page::Page for WelcomePage {
    fn title(&self) -> String {
        fl!("welcome")
    }

    fn view(&self) -> Element<page::Message> {
        let mut section = widget::settings::section();
        section = section.add(
            widget::settings::item::builder(fl!("screen-reader"))
                .toggler(false, |_| page::Message::Todo),
        );
        section = section.add(
            widget::settings::item::builder(fl!("interface-size")).control(widget::dropdown(
                &[
                    "50%", "75%", "100%", "125%", "150%", "175%", "200%", "225%", "250%", "275%",
                    "300%",
                ],
                Some(2),
                |_| page::Message::Todo,
            )),
        );
        section.into()
    }
}
