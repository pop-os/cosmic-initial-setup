use crate::accessibility::{AccessibilityEvent, AccessibilityRequest};
use crate::{fl, page};
use cosmic::Task;
use cosmic::{Element, widget};
use cosmic_settings_subscriptions::accessibility::{DBusRequest, DBusUpdate};
use futures_util::{FutureExt, SinkExt};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// Create static DPI_SCALE variables.
#[crabtime::function]
fn gen_dpi_scale_variables(components: Vec<String>) {
    let values = components.join(", ");

    let scales = components
        .iter()
        .map(|scale| format!("\"{scale}%\""))
        .collect::<Vec<String>>()
        .join(", ");

    crabtime::output! {
        static DPI_SCALES: &[u32] = &[ {{values}} ];
        static DPI_SCALE_LABELS: &[&str] = &[ {{scales}} ];
    }
}

gen_dpi_scale_variables!([50, 75, 100, 125, 150, 175, 200, 225, 250, 275, 300]);

pub struct Page {
    list: cosmic_randr_shell::List,
    magnifier_enabled: bool,
    reader_enabled: bool,
    interface_scale: usize,
    interface_adjusted_scale: u32,
    a11y_wayland_thread: Option<crate::accessibility::Sender>,
    screen_reader_dbus_sender: Option<UnboundedSender<DBusRequest>>,
}

impl page::Page for Page {
    fn title(&self) -> String {
        fl!("welcome-page")
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self) -> cosmic::Task<page::Message> {
        let mut tasks = Vec::new();

        // Intialize the a11y wayland thread, and fetch displays.
        if self.a11y_wayland_thread.is_none() {
            match crate::accessibility::spawn_wayland_connection() {
                Ok((tx, mut rx)) => {
                    self.a11y_wayland_thread = Some(tx);

                    let task = cosmic::Task::stream(cosmic::iced_futures::stream::channel(
                        1,
                        |mut sender| async move {
                            while let Some(event) = rx.recv().await {
                                let _ = sender
                                    .send(page::Message::Welcome(Message::A11yEvent(event)))
                                    .await;
                            }
                        },
                    ));

                    tasks.push(task);
                }
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        "Failed to spawn wayland connection for accessibility page"
                    );
                }
            }
        }

        tasks.push(cosmic::task::future(async {
            let list = cosmic_randr_shell::list().await;
            page::Message::from(Message::UpdateDisplayList(Arc::new(list)))
        }));

        cosmic::task::batch(tasks)
    }

    fn view(&self) -> Element<'_, page::Message> {
        let screen_reader = widget::settings::item::builder(fl!("welcome-page", "screen-reader"))
            .toggler(self.reader_enabled, |enable| {
                Message::ScreenReaderEnabled(enable).into()
            });

        let interface_size = widget::settings::item::builder(fl!("welcome-page", "interface-size"))
            .control(widget::dropdown(
                DPI_SCALE_LABELS,
                Some(self.interface_scale),
                |option| Message::Scale(option).into(),
            ));

        let scale_options = widget::settings::item::builder(fl!("welcome-page", "scale-options"))
            .control(widget::spin_button(
                format!("{}%", self.interface_adjusted_scale),
                self.interface_adjusted_scale,
                5,
                0,
                20,
                |value| Message::ScaleAdjust(value).into(),
            ));

        let magnifier = widget::settings::item::builder(fl!("welcome-page", "magnifier"))
            .toggler(self.magnifier_enabled, |enable| {
                Message::MagnifierEnabled(enable).into()
            });

        widget::settings::section()
            .add(screen_reader)
            .add(interface_size)
            .add(scale_options)
            .add(magnifier)
            .into()
    }
}

impl Page {
    pub fn new() -> Self {
        Self {
            list: cosmic_randr_shell::List::default(),
            magnifier_enabled: false,
            reader_enabled: false,
            interface_scale: 2,
            interface_adjusted_scale: 0,
            a11y_wayland_thread: None,
            screen_reader_dbus_sender: None,
        }
    }

    pub fn update(&mut self, message: Message) -> cosmic::Task<page::Message> {
        match message {
            Message::A11yEvent(AccessibilityEvent::Bound(version)) => {
                // self.wayland_available = Some(version);
            }

            Message::A11yEvent(AccessibilityEvent::Magnifier(value)) => {
                self.magnifier_enabled = value;
            }

            Message::A11yEvent(AccessibilityEvent::ScreenFilter { inverted, filter }) => {
                //     self.screen_inverted = inverted;
                //     self.screen_filter_active = filter.is_some();
                //     if let Some(filter) = filter {
                //         self.screen_filter_selection = filter;
                //     }
            }

            Message::A11yEvent(AccessibilityEvent::Closed) => {
                // self.wayland_available = None;
                // self.screen_filter_active = false;
            }

            Message::MagnifierEnabled(enabled) => {
                if let Some(sender) = self.a11y_wayland_thread.as_ref() {
                    tracing::debug!("toggling magnifier to {enabled}");
                    let _ = sender.send(AccessibilityRequest::Magnifier(enabled));
                }
            }

            Message::Scale(option) => return self.set_scale(option, 0),

            Message::ScaleAdjust(scale_adjust) => {
                return self.set_scale(self.interface_scale, scale_adjust);
            }

            Message::ScaleAdjustResult(ScaleAdjustResult::Success) => {}

            Message::ScaleAdjustResult(ScaleAdjustResult::FailureCode(code)) => {}

            Message::ScaleAdjustResult(ScaleAdjustResult::SpawnFailure(why)) => {}

            Message::ScreenReaderEnabled(enabled) => {
                if let Some(tx) = &self.screen_reader_dbus_sender {
                    self.reader_enabled = enabled;
                    let _ = tx.send(DBusRequest::Status(enabled));
                } else {
                    self.reader_enabled = false;
                }
            }

            Message::ScreenReaderDbus(update) => match update {
                DBusUpdate::Error(err) => {
                    tracing::error!(?err, "screen reader dbus error");
                    let _ = self.screen_reader_dbus_sender.take();
                    self.reader_enabled = false;
                }
                DBusUpdate::Status(enabled) => {
                    self.reader_enabled = enabled;
                }
                DBusUpdate::Init(enabled, tx) => {
                    self.reader_enabled = enabled;
                    self.screen_reader_dbus_sender = Some(tx);
                }
            },

            Message::UpdateDisplayList(result) => match Arc::into_inner(result) {
                Some(Ok(outputs)) => {
                    tracing::debug!("updating outputs");
                    self.list = outputs;
                    if let Some((_, first_output)) = self.list.outputs.iter().next() {
                        let scale_u32 = ((first_output.scale * 100.0) as u32).min(300);
                        self.interface_scale =
                            (scale_u32 / 25).checked_sub(2).unwrap_or(2) as usize;
                        self.interface_adjusted_scale = (scale_u32 % 25).min(20);
                    }
                }

                Some(Err(why)) => {
                    tracing::debug!("error fetching displays: {}", why);
                }

                None => (),
            },
        }

        Task::none()
    }

    /// Set the scale of each display.
    fn set_scale(&mut self, option: usize, scale_adjust: u32) -> Task<page::Message> {
        tracing::debug!("setting scale {option} with {scale_adjust}");
        self.interface_scale = option;
        self.interface_adjusted_scale = scale_adjust;

        let tasks = self
            .list
            .outputs
            .iter()
            .filter_map(|(_, output)| {
                let current = &self.list.modes[output.current?];
                let scale = (option * 25 + 50) as u32 + self.interface_adjusted_scale.min(20);

                let mut command = tokio::process::Command::new("cosmic-randr");

                command
                    .arg("mode")
                    .arg("--scale")
                    .arg(format!("{}.{:02}", scale / 100, scale % 100))
                    .arg("--refresh")
                    .arg(format!(
                        "{}.{:03}",
                        current.refresh_rate / 1000,
                        current.refresh_rate % 1000
                    ))
                    .arg(output.name.as_str())
                    .arg(itoa::Buffer::new().format(current.size.0))
                    .arg(itoa::Buffer::new().format(current.size.1));

                tracing::debug!("running command: {command:?}");

                let command_fut = command
                    .status()
                    .map(|result| {
                        Message::ScaleAdjustResult(match result {
                            Ok(status) if status.success() => ScaleAdjustResult::Success,
                            Ok(status) => ScaleAdjustResult::FailureCode(status.code()),
                            Err(why) => ScaleAdjustResult::SpawnFailure(Arc::new(why)),
                        })
                    })
                    .map_into::<page::Message>();

                Some(cosmic::task::future(command_fut))
            })
            .collect::<Vec<_>>();

        Task::batch(tasks)
    }
}

#[derive(Clone, Debug)]
pub enum ScaleAdjustResult {
    Success,
    FailureCode(Option<i32>),
    SpawnFailure(Arc<std::io::Error>),
}

#[derive(Clone, Debug)]
pub enum Message {
    /// Handle an a11y event.
    A11yEvent(crate::accessibility::AccessibilityEvent),
    /// Toggle the screen magnifier.
    MagnifierEnabled(bool),
    /// Set the preferred scale for a display.
    Scale(usize),
    /// Adjust the display scale.
    ScaleAdjust(u32),
    /// Status of scale adjust command.
    ScaleAdjustResult(ScaleAdjustResult),
    /// Screen reader DBus events.
    ScreenReaderDbus(DBusUpdate),
    /// Enable the screen reader.
    ScreenReaderEnabled(bool),
    /// Update the display list
    UpdateDisplayList(Arc<Result<cosmic_randr_shell::List, cosmic_randr_shell::Error>>),
}

impl From<Message> for page::Message {
    fn from(message: Message) -> Self {
        page::Message::Welcome(message)
    }
}
