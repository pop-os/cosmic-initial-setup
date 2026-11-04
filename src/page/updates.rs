// Copyright 2026 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Length;
use cosmic::iced::alignment::Vertical;
use cosmic::{Apply, Element, surface, widget};
use pop_system_updater_zbus::client::ClientProxy;
use pop_system_updater_zbus::client::notification::ClientProxy as NotificationClientProxy;
use pop_system_updater_zbus::{Interval, Schedule, client};
use std::any::Any;
use std::sync::Arc;
use std::unreachable;

#[derive(Clone, Debug)]
pub enum Message {
    SetAutomaticUpdates(bool),
    SetSchedule(Schedule),
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
    pub frequency_labels: Vec<String>,
    pub interval_labels: Vec<String>,
    pub time_labels: Vec<String>,
    pub hour_label: String,
    pub minute_label: String,
    pub is_pm: usize,
    pub hour: u8,
    pub minute: u8,
}

impl Page {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, message: Message) -> cosmic::Task<crate::Message> {
        match message {
            Message::SetSchedule(schedule) => {
                self.apply_schedule(&schedule);
                self.config.schedule = Some(schedule.clone());

                if let Some((client, _)) = self.clients.as_ref() {
                    let mut client = client.clone();
                    tokio::task::spawn(async move {
                        _ = client.update_scheduling_set(schedule).await;
                    });
                }
            }
            Message::SetAutomaticUpdates(enable) => {
                self.config.auto_update = enable;

                if let Some((client, _)) = self.clients.as_ref() {
                    let mut client = client.clone();
                    tokio::task::spawn(async move {
                        _ = client.auto_update_set(enable).await;
                    });
                }
            }

            Message::SystemUpdateClient(client_proxy, notification_proxy) => {
                self.clients = Some((client_proxy, notification_proxy));
            }

            Message::SystemUpdateConfigs(config, notification_config) => {
                if let Some(ref schedule) = config.schedule {
                    self.apply_schedule(schedule);
                }
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

    #[inline(never)]
    fn apply_schedule(&mut self, schedule: &Schedule) {
        fn as_12(hour: u8) -> (u8, bool) {
            if hour == 0 {
                (12, false)
            } else if hour < 12 {
                (hour, false)
            } else if hour == 12 {
                (12, true)
            } else {
                (hour - 12, true)
            }
        }

        let (hour, is_pm) = match as_12(schedule.hour) {
            (hour, false) => (hour, 0),
            (hour, true) => (hour, 1),
        };

        self.hour_label = format!("{:02}", hour);
        self.hour = hour;
        self.minute_label = format!("{:02}", schedule.minute);
        self.minute = schedule.minute;
        self.is_pm = is_pm;
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

    fn open(&mut self) -> cosmic::Task<super::Message> {
        self.time_labels = vec![crate::fl!("time-am"), crate::fl!("time-pm")];

        self.frequency_labels = vec![
            crate::fl!("disabled"),
            crate::fl!("frequency-daily"),
            crate::fl!("frequency-weekly"),
            crate::fl!("frequency-monthly"),
        ];

        self.interval_labels = vec![
            crate::fl!("time-monday"),
            crate::fl!("time-tuesday"),
            crate::fl!("time-wednesday"),
            crate::fl!("time-thursday"),
            crate::fl!("time-friday"),
            crate::fl!("time-saturday"),
            crate::fl!("time-sunday"),
            crate::fl!("time-weekdays"),
        ];

        cosmic::Task::none()
    }

    fn view(&self) -> Element<'_, super::Message> {
        let theme = cosmic::theme::active();
        let spacing = &theme.cosmic().spacing;

        let automatic_updates_toggle =
            widget::settings::item::builder(crate::fl!("updates-page", "automatic-toggle"))
                .toggler(self.config.auto_update, |enable| {
                    Message::SetAutomaticUpdates(enable).into()
                });

        let update_schedule = self.config.auto_update.then_some({
            let schedule = self
                .config
                .schedule
                .clone()
                .unwrap_or_else(pop_system_updater_zbus::Config::default_schedule);

            let hour = self.hour;
            let minute = self.minute;
            let is_pm = self.is_pm;

            let interval_pos = match schedule.interval {
                Interval::Monday => 0,
                Interval::Tuesday => 1,
                Interval::Wednesday => 2,
                Interval::Thursday => 3,
                Interval::Friday => 4,
                Interval::Saturday => 5,
                Interval::Sunday => 6,
                Interval::Weekdays => 7,
            };

            let schedule = Arc::new(schedule);

            let interval = cosmic::widget::dropdown::popup_dropdown(
                &self.interval_labels,
                Some(interval_pos),
                {
                    let schedule = schedule.clone();
                    move |pos| {
                        let mut schedule = schedule.as_ref().clone();
                        schedule.interval = match pos {
                            0 => Interval::Monday,
                            1 => Interval::Tuesday,
                            2 => Interval::Wednesday,
                            3 => Interval::Thursday,
                            4 => Interval::Friday,
                            5 => Interval::Saturday,
                            6 => Interval::Sunday,
                            7 => Interval::Weekdays,
                            _ => unreachable!("unknown interval index"),
                        };

                        super::Message::from(Message::SetSchedule(schedule))
                    }
                },
                cosmic::iced::window::Id::RESERVED,
                |a| Message::Surface(a).into(),
                crate::Message::from,
            );

            let hour = cosmic::widget::spin_button::vertical(
                &self.hour_label,
                crate::fl!("time-hour"),
                hour,
                1,
                0,
                13,
                {
                    let schedule = schedule.clone();
                    move |mut v: u8| {
                        let mut schedule = schedule.as_ref().clone();
                        if v != hour {
                            if is_pm == 1 {
                                if v == 0 {
                                    v = 12;
                                } else if hour == 12 && v == 11 {
                                    v = 11
                                } else if hour == 11 && v == 12 {
                                    v = 0;
                                } else if v != 13 {
                                    v += 12;
                                }
                            } else if hour == 12 && v == 11 {
                                v = 23;
                            } else if v == 13 {
                                v = 1;
                            }

                            schedule.hour = v
                        }

                        Message::SetSchedule(schedule).into()
                    }
                },
            );

            let minute = cosmic::widget::spin_button::vertical(
                &self.minute_label,
                crate::fl!("time-minute"),
                minute,
                1,
                0,
                59,
                {
                    let schedule = schedule.clone();
                    move |v| {
                        let mut schedule = schedule.as_ref().clone();
                        schedule.minute = v;
                        Message::SetSchedule(schedule).into()
                    }
                },
            );

            let time_of_day = cosmic::widget::dropdown::popup_dropdown(
                &self.time_labels,
                Some(is_pm),
                {
                    let schedule = schedule.clone();
                    move |pos| {
                        let mut schedule = schedule.as_ref().clone();
                        if pos != is_pm {
                            if pos == 1 {
                                schedule.hour += 12
                            } else {
                                schedule.hour -= 12;
                            }
                        }

                        Message::SetSchedule(schedule).into()
                    }
                },
                cosmic::iced::window::Id::RESERVED,
                |a| Message::Surface(a).into(),
                crate::Message::from,
            );

            let control = widget::row([
                interval.into(),
                hour.into(),
                minute.into(),
                time_of_day.into(),
            ])
            .align_y(Vertical::Center);

            cosmic::widget::settings::item::builder(crate::fl!("updates-page", "schedule"))
                .control(control)
        });

        let settings = cosmic::widget::settings::section::section()
            .add(automatic_updates_toggle)
            .add_maybe(update_schedule);

        widget::column::with_capacity(2)
            .push(settings)
            .spacing(spacing.space_l)
            .width(Length::Fill)
            .apply(Element::from)
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
