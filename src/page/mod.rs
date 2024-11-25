use cosmic::{widget, Element, Task};

use crate::fl;

pub mod appearance;
pub mod launcher;
pub mod location;
pub mod welcome;

pub fn pages() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(welcome::WelcomePage),
        //TODO: If no user exists
        // Box::new(LanguagePage),
        // Box::new(KeyboardPage),
        // Box::new(UserPage),
        Box::new(location::LocationPage::new()),
        Box::new(appearance::AppearancePage::new()),
        Box::new(LayoutPage),
        Box::new(WorkflowPage),
        Box::new(launcher::LauncherPage::new()),
        Box::new(WirelessPage),
    ]
}

#[derive(Clone, Debug)]
pub enum Message {
    Open,
    Appearance(appearance::Message),
    Location(location::Message),
    Todo,
}

pub trait Page {
    fn title(&self) -> String;

    fn width(&self) -> f32 {
        480.0
    }

    fn completed(&self) -> bool {
        true
    }

    fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
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

pub struct WirelessPage;

impl Page for WirelessPage {
    fn title(&self) -> String {
        fl!("get-connected")
    }
}
