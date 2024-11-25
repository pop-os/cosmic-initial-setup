use cosmic::{widget, Element, Task};

use crate::fl;

pub mod location;

pub fn pages() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(WelcomePage),
        //TODO: If no user exists
        // Box::new(LanguagePage),
        // Box::new(KeyboardPage),
        // Box::new(UserPage),
        Box::new(location::LocationPage::new()),
        Box::new(AppearancePage),
        Box::new(LayoutPage),
        Box::new(WorkflowPage),
        Box::new(LauncherPage),
        Box::new(WirelessPage),
    ]
}

#[derive(Clone, Debug)]
pub enum Message {
    Open,
    Location(location::Message),
    Todo,
}

pub trait Page {
    fn title(&self) -> String;
    fn completed(&self) -> bool {
        true
    }
    fn width(&self) -> f32 {
        480.0
    }
    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
    }
    fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }
}

pub struct WelcomePage;

impl Page for WelcomePage {
    fn title(&self) -> String {
        fl!("welcome")
    }

    fn view(&self) -> Element<Message> {
        let mut section = widget::settings::section();
        section = section.add(
            widget::settings::item::builder(fl!("screen-reader")).toggler(false, |_| Message::Todo),
        );
        section = section.add(
            widget::settings::item::builder(fl!("interface-size")).control(widget::dropdown(
                &[
                    "50%", "75%", "100%", "125%", "150%", "175%", "200%", "225%", "250%", "275%",
                    "300%",
                ],
                Some(2),
                |_| Message::Todo,
            )),
        );
        section.into()
    }
}

pub struct AppearancePage;

impl Page for AppearancePage {
    fn title(&self) -> String {
        fl!("personalize-appearance")
    }
}

pub struct LayoutPage;

impl Page for LayoutPage {
    fn title(&self) -> String {
        fl!("layout-configuration")
    }
}

pub struct WorkflowPage;

impl Page for WorkflowPage {
    fn title(&self) -> String {
        fl!("your-workflow-your-way")
    }
}

pub struct LauncherPage;

impl Page for LauncherPage {
    fn title(&self) -> String {
        fl!("fast-and-efficient")
    }
}

pub struct WirelessPage;

impl Page for WirelessPage {
    fn title(&self) -> String {
        fl!("get-connected")
    }
}
