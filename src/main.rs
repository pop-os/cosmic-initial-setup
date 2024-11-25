// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    app::{self, Core, Settings, Task},
    cosmic_theme, executor,
    iced::{Length, Limits},
    theme, widget, Application, Element,
};

mod localize;

use self::page::Page;
mod page;

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

fn page_task(message: app::Message<page::Message>) -> app::Message<Message> {
    match message {
        app::Message::App(app) => app::Message::App(Message::PageMessage(app)),
        app::Message::Cosmic(cosmic) => app::Message::Cosmic(cosmic),
        app::Message::None => app::Message::None,
    }
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    None,
    Finish,
    PageMessage(page::Message),
    PageOpen(usize),
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
    fn init(mut core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        core.window.show_close = false;
        core.window.show_maximize = false;
        core.window.show_minimize = false;
        let mut app = App {
            core,
            pages: page::pages(),
            page_i: 0,
        };
        let task = app.update(Message::PageOpen(0));
        (app, task)
    }

    /// Handle application events here.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::None => {}
            Message::Finish => {
                //TODO: save some config about finishing
                return cosmic::iced::exit();
            }
            Message::PageMessage(page_message) => {
                if let Some(page) = self.pages.get_mut(self.page_i) {
                    return page.update(page_message).map(page_task);
                }
            }
            Message::PageOpen(page_i) => {
                if let Some(page) = self.pages.get_mut(page_i) {
                    self.page_i = page_i;
                    return page.update(page::Message::Open).map(page_task);
                }
            }
        }
        Task::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Message> {
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
                button_row = button_row.push(
                    widget::button::standard(fl!("back")).on_press(Message::PageOpen(page_i)),
                );
            }
        }
        if let Some(page_i) = self.page_i.checked_add(1) {
            if self.pages.get(page_i).is_some() {
                let mut next = widget::button::suggested(fl!("next"));
                if page.completed() {
                    next = next.on_press(Message::PageOpen(page_i));
                }
                button_row = button_row.push(next);
            } else {
                let mut finish = widget::button::suggested(fl!("finish"));
                if page.completed() {
                    finish = finish.on_press(Message::Finish);
                }
                button_row = button_row.push(finish);
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
                widget::container(page.view().map(Message::PageMessage))
                    .height(406.0)
                    .into(),
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
