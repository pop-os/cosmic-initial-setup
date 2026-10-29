// Copyright 2026 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::{Length, alignment};
use cosmic::{Apply, Element, surface, widget};
use pop_system_updater_zbus::client::ClientProxy;
use pop_system_updater_zbus::client::notification::ClientProxy as NotificationClientProxy;
use pop_system_updater_zbus::{Frequency, client};
use std::any::Any;

#[derive(Clone, Debug)]
pub enum Message {
    SetAutomaticUpdates(bool),
    SetNotificationFrequency(Option<Frequency>),
    /// Handling of internal messages
    Surface(surface::Action<crate::Message>),
    SystemUpdateClient(ClientProxy<'static>, NotificationClientProxy<'static>),
    SystemUpdateConfigs(
        pop_system_updater_zbus::Config,
        pop_system_updater_zbus::Local,
    ),
    Error(String),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::Updates(message)
    }
}

impl From<Message> for crate::Message {
    fn from(message: Message) -> Self {
        crate::Message::PageMessage(super::Message::Updates(message))
    }
}

#[derive(Default)]
pub struct Page {
    pub clients: Option<(
        client::ClientProxy<'static>,
        client::notification::ClientProxy<'static>,
    )>,
    pub notification_config: pop_system_updater_zbus::Local,
    pub config: pop_system_updater_zbus::Config,
}

impl Page {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, message: Message) -> cosmic::Task<crate::Message> {
        match message {
            Message::SetAutomaticUpdates(enable) => {
                self.config.auto_update = enable;

                if let Some((client, _)) = self.clients.as_ref() {
                    let mut client = client.clone();
                    tokio::task::spawn(async move {
                        _ = client.auto_update_set(enable).await;
                    });
                }
            }

            Message::SetNotificationFrequency(frequency) => {
                match frequency {
                    Some(frequency) => {
                        self.notification_config.enabled = true;
                        self.notification_config.notification_frequency = frequency;
                    }
                    None => self.notification_config.enabled = false,
                }

                if let Some((_, client)) = self.clients.as_ref() {
                    let mut client = client.clone();
                    tokio::task::spawn(async move {
                        eprintln!("setting notification frequency: {frequency:?}");
                        match frequency {
                            Some(frequency) => {
                                _ = client.notifications_enabled(true).await;
                                _ = dbg!(client.set_notification_frequency(frequency).await);
                            }
                            None => {
                                _ = client.notifications_enabled(false).await;
                            }
                        }
                    });
                }
            }

            Message::SystemUpdateClient(client_proxy, notification_proxy) => {
                self.clients = Some((client_proxy, notification_proxy));
            }

            Message::SystemUpdateConfigs(config, notification_config) => {
                self.config = config;
                self.notification_config = notification_config;
            }

            Message::Error(why) => {
                tracing::error!(?why, "updates page");
            }

            Message::Surface(a) => {
                return cosmic::task::message(crate::Message::Surface(a));
            }
        }

        cosmic::Task::none()
    }
}

impl super::Page for Page {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn title(&self) -> String {
        crate::fl!("updates-page")
    }

    fn skippable(&self) -> bool {
        true
    }

    fn init(&mut self) -> cosmic::Task<super::Message> {
        cosmic::Task::batch([
            cosmic::Task::future(async { load_clients().await }),
            cosmic::Task::future(async { load_configs().await }),
        ])
    }

    fn view(&self) -> Element<'_, super::Message> {
        let theme = cosmic::theme::active();
        let spacing = &theme.cosmic().spacing;

        let notification_dropdown = widget::dropdown::popup_dropdown(
            vec![
                crate::fl!("disabled"),
                crate::fl!("frequency-daily"),
                crate::fl!("frequency-weekly"),
                crate::fl!("frequency-monthly"),
            ],
            Some(if !self.notification_config.enabled {
                0
            } else {
                match self.notification_config.notification_frequency {
                    Frequency::Daily => 1,
                    Frequency::Weekly => 2,
                    Frequency::Monthly => 3,
                }
            }),
            |selected| {
                Message::SetNotificationFrequency(match selected {
                    1 => Some(Frequency::Daily),
                    2 => Some(Frequency::Weekly),
                    3 => Some(Frequency::Monthly),
                    _ => None,
                })
            },
            cosmic::iced::window::Id::RESERVED,
            Message::Surface,
            crate::Message::from,
        );

        let automatic_updates_toggle =
            widget::settings::item::builder(crate::fl!("updates-page", "automatic-toggle"))
                .toggler(self.config.auto_update, |enable| {
                    Message::SetAutomaticUpdates(enable)
                });

        let update_notification_frequency =
            widget::settings::item::builder(crate::fl!("updates-page", "notifications-toggle"))
                .control(notification_dropdown);

        let description = widget::text::body(crate::fl!("updates-page", "explain"))
            .align_x(alignment::Horizontal::Center)
            .width(Length::Fill);

        let view = widget::column::with_capacity(2)
            .push(widget::container(description))
            .push(
                widget::list_column()
                    .add(automatic_updates_toggle)
                    .add(update_notification_frequency),
            );

        view.spacing(spacing.space_l)
            .width(Length::Fill)
            .apply(Element::from)
            .map(super::Message::Updates)
    }
}

pub async fn load_clients() -> super::Message {
    let system_client = client::ClientProxy::new(crate::zbus_system_connection());
    let session_client = client::notification::ClientProxy::new(crate::zbus_session_connection());

    match futures::try_join!(system_client, session_client) {
        Ok((system_client, session_client)) => {
            Message::SystemUpdateClient(system_client, session_client).into()
        }
        Err(why) => Message::Error(format!("DBus proxy connection failed: {why}")).into(),
    }
}

pub async fn load_configs() -> super::Message {
    let (config, notification_config) = futures::join!(
        pop_system_updater_zbus::config::load_system(),
        pop_system_updater_zbus::config::load_session(),
    );
    Message::SystemUpdateConfigs(config, notification_config).into()
}
