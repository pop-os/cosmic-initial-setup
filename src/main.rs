// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    app::{message, Core, CosmicFlags, Settings, Task},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        event::{self, Event},
        futures::{self, SinkExt},
        keyboard::{Event as KeyEvent, Key, Modifiers},
        stream,
        widget::scrollable,
        window::{self, Event as WindowEvent},
        Alignment, Length, Limits, Size, Subscription,
    },
    theme, widget, Application, ApplicationExt, Element,
};
use std::process;

mod localize;

/// Runs application with these settings
#[rustfmt::skip]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    localize::localize();

    let mut settings = Settings::default();
    settings = settings.size_limits(Limits::NONE.width(900.0).height(650.0));

    cosmic::app::run::<App>(settings, ())?;

    Ok(())
}

trait Page {
    fn title(&self) -> String;
    fn width(&self) -> f32 {
        480.0
    }
    fn view(&self) -> Element<Message>;
}

struct WelcomePage;

impl Page for WelcomePage {
    fn title(&self) -> String {
        fl!("welcome")
    }

    fn view(&self) -> Element<Message> {
        let mut section = widget::settings::section();
        section = section.add(
            widget::settings::item::builder(fl!("screen-reader")).toggler(false, |_| Message::None),
        );
        section = section.add(
            widget::settings::item::builder(fl!("interface-size")).control(widget::dropdown(
                &[
                    "50%", "75%", "100%", "125%", "150%", "175%", "200%", "225%", "250%", "275%",
                    "300%",
                ],
                Some(2),
                |_| Message::None,
            )),
        );
        section.into()
    }
}

struct LocationPage;

impl Page for LocationPage {
    fn title(&self) -> String {
        fl!("timezone-and-location")
    }

    fn view(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_m, .. } = theme::active().cosmic().spacing;

        let mut section = widget::settings::section();
        for hour in 0..12 {
            section = section.add(
                widget::settings::item::builder(format!("Example {}", hour))
                    .control(widget::text::body(format!("+{}:00 UTC", hour))),
            );
        }
        widget::column::with_children(vec![
            widget::search_input(fl!("search-the-closest-major-city"), String::new()).into(),
            widget::Space::with_height(space_m).into(),
            widget::scrollable(section).into(),
        ])
        .into()
    }
}

struct AppearancePage;

impl Page for AppearancePage {
    fn title(&self) -> String {
        fl!("personalize-appearance")
    }

    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
    }
}

struct LayoutPage;

impl Page for LayoutPage {
    fn title(&self) -> String {
        fl!("layout-configuration")
    }

    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
    }
}

struct WorkflowPage;

impl Page for WorkflowPage {
    fn title(&self) -> String {
        fl!("your-workflow-your-way")
    }

    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
    }
}

struct LauncherPage;

impl Page for LauncherPage {
    fn title(&self) -> String {
        fl!("fast-and-efficient")
    }

    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
    }
}

struct WirelessPage;

impl Page for WirelessPage {
    fn title(&self) -> String {
        fl!("get-connected")
    }

    fn view(&self) -> Element<Message> {
        widget::text::body("TODO").into()
    }
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    None,
    Finish,
    Page(usize),
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    pages: Vec<Box<dyn Page>>,
    page_i: usize,
}

/// Implement [`Application`] to integrate with COSMIC.
impl Application for App {
    /// Multithreaded async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received
    type Flags = ();

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicInitialSetup";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        core.window.show_headerbar = false;
        core.window.show_close = false;
        core.window.show_maximize = false;
        core.window.show_minimize = false;
        (
            App {
                core,
                pages: vec![
                    Box::new(WelcomePage),
                    //TODO: If no user exists
                    // Box::new(LanguagePage),
                    // Box::new(KeyboardPage),
                    // Box::new(UserPage),
                    Box::new(LocationPage),
                    Box::new(AppearancePage),
                    Box::new(LayoutPage),
                    Box::new(WorkflowPage),
                    Box::new(LauncherPage),
                    Box::new(WirelessPage),
                ],
                page_i: 0,
            },
            Task::none(),
        )
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Message> {
        match message {
            Message::None => {}
            Message::Finish => {
                //TODO: save some config about finishing
                return cosmic::iced::exit();
            }
            Message::Page(page_i) => {
                if self.pages.get(page_i).is_some() {
                    self.page_i = page_i;
                }
            }
        }
        Task::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_m,
            space_xl,
            ..
        } = theme::active().cosmic().spacing;

        let page = &self.pages[self.page_i];
        let mut button_row = widget::row::with_capacity(3)
            .spacing(space_xxs)
            .push(widget::horizontal_space());
        if let Some(page_i) = self.page_i.checked_sub(1) {
            if self.pages.get(page_i).is_some() {
                button_row = button_row
                    .push(widget::button::standard(fl!("back")).on_press(Message::Page(page_i)));
            }
        }
        if let Some(page_i) = self.page_i.checked_add(1) {
            if self.pages.get(page_i).is_some() {
                button_row = button_row
                    .push(widget::button::suggested(fl!("next")).on_press(Message::Page(page_i)));
            } else {
                button_row = button_row
                    .push(widget::button::suggested(fl!("finish")).on_press(Message::Finish));
            }
        }

        widget::container(
            widget::column::with_children(vec![
                widget::Space::with_height(space_xl).into(),
                widget::text::title1(page.title())
                    .center()
                    .width(Length::Fill)
                    .into(),
                widget::Space::with_height(space_xl).into(),
                widget::container(page.view()).height(406.0).into(),
                widget::Space::with_height(space_m).into(),
                button_row.into(),
                widget::Space::with_height(space_xl).into(),
            ])
            .max_width(page.width()),
        )
        .center_x(Length::Fill)
        .into()
    }
}
