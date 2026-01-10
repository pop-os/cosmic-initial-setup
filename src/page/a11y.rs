use crate::{fl, page};
use cosmic::Task;
use cosmic::iced::{Alignment, Length, alignment};
use cosmic::iced_core::text::Wrapping;
use cosmic::widget::{segmented_button, text};
use cosmic::{Element, widget};
use cosmic_randr_shell::OutputKey;
use cosmic_settings_a11y_manager_subscription::{
    self as cosmic_a11y_manager, AccessibilityEvent, AccessibilityRequest,
};
use cosmic_settings_accessibility_subscription as a11y_bus;
use futures_util::{FutureExt, SinkExt};
use std::collections::BTreeMap;
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
    displays: segmented_button::SingleSelectModel,
    magnifier_enabled: bool,
    reader_enabled: bool,
    scale: usize,
    interface_adjusted_scale: u32,
    a11y_wayland_thread: Option<cosmic_a11y_manager::Sender>,
    screen_reader_dbus_sender: Option<UnboundedSender<a11y_bus::Request>>,
}

impl page::Page for Page {
    fn title(&self) -> String {
        fl!("accessibility-page")
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self) -> cosmic::Task<page::Message> {
        let mut tasks = Vec::new();

        // Intialize the a11y wayland thread.
        if self.a11y_wayland_thread.is_none() {
            match cosmic_a11y_manager::spawn_wayland_connection(1) {
                Ok((tx, mut rx)) => {
                    self.a11y_wayland_thread = Some(tx);

                    let task = cosmic::Task::stream(cosmic::iced_futures::stream::channel(
                        1,
                        |mut sender| async move {
                            while let Some(event) = rx.recv().await {
                                let _ = sender
                                    .send(page::Message::A11y(Message::A11yEvent(event)))
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
                    self.a11y_wayland_thread = None;
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
        let spacing = cosmic::theme::spacing();

        let screen_reader = {
            let text = widget::column::with_capacity(2)
                .spacing(2)
                .push(
                    text::body(fl!("accessibility-page", "screen-reader")).wrapping(Wrapping::Word),
                )
                .push(text::caption("Super + Alt + S").wrapping(Wrapping::Word))
                .width(Length::Shrink);

            let icon = self.reader_enabled.then(|| {
                widget::icon::from_name("audio-speakers-symbolic")
                    .icon()
                    .size(24)
                    .class(cosmic::style::Svg::custom(|theme| {
                        cosmic::iced::widget::svg::Style {
                            color: Some(theme.cosmic().success_text_color().into()),
                        }
                    }))
            });

            widget::settings::item_row(vec![
                widget::row::with_capacity(2)
                    .spacing(spacing.space_xs)
                    .push(text)
                    .push_maybe(icon)
                    .align_y(alignment::Vertical::Center)
                    .width(Length::Fill)
                    .into(),
                widget::toggler(self.reader_enabled)
                    .on_toggle(|enable| Message::ScreenReaderEnabled(enable).into())
                    .into(),
            ])
        };

        let display_switcher = (self.displays.len() > 1).then(|| {
            widget::tab_bar::horizontal(&self.displays)
                .button_alignment(Alignment::Center)
                .on_activate(|e| Message::Display(e).into())
        });

        let scale = widget::settings::item::builder(fl!("accessibility-page", "scale")).control(
            widget::dropdown(DPI_SCALE_LABELS, Some(self.scale), |option| {
                Message::Scale(option).into()
            }),
        );

        let scale_options =
            widget::settings::item::builder(fl!("accessibility-page", "scale-options")).control(
                widget::spin_button(
                    format!("{}%", self.interface_adjusted_scale),
                    fl!("accessibility-page", "scale-options"),
                    self.interface_adjusted_scale,
                    5,
                    0,
                    20,
                    |value| Message::ScaleAdjust(value).into(),
                ),
            );

        let magnifier = widget::settings::item::builder(fl!("accessibility-page", "magnifier"))
            .description(fl!("accessibility-page", "magnifier-description"))
            .toggler(self.magnifier_enabled, |enable| {
                Message::MagnifierEnabled(enable).into()
            });

        let a11y_section = widget::settings::section()
            .add(screen_reader)
            .add(magnifier);

        let display_settings = widget::settings::section().add(scale).add(scale_options);

        if let Some(switcher) = display_switcher {
            widget::column::with_capacity(5)
                .push(a11y_section)
                .push(widget::vertical_space().height(spacing.space_xl))
                .push(switcher)
                .push(widget::vertical_space().height(spacing.space_xxs))
                .push(display_settings)
                .into()
        } else {
            widget::column::with_capacity(3)
                .push(a11y_section)
                .push(widget::vertical_space().height(spacing.space_xl))
                .push(display_settings.title(fl!("accessibility-page", "display-scaling")))
                .into()
        }
    }
}

impl Page {
    pub fn new() -> Self {
        Self {
            list: cosmic_randr_shell::List::default(),
            displays: segmented_button::SingleSelectModel::default(),
            magnifier_enabled: false,
            reader_enabled: false,
            scale: 2,
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
                return self.set_scale(self.scale, scale_adjust);
            }

            Message::ScaleAdjustResult(ScaleAdjustResult::Success) => {}

            Message::ScaleAdjustResult(ScaleAdjustResult::FailureCode(_code)) => {}

            Message::ScaleAdjustResult(ScaleAdjustResult::SpawnFailure(_why)) => {}

            Message::ScreenReaderEnabled(enabled) => {
                if let Some(tx) = &self.screen_reader_dbus_sender {
                    self.reader_enabled = enabled;
                    let _ = tx.send(a11y_bus::Request::ScreenReader(enabled));
                } else {
                    self.reader_enabled = false;
                }
            }

            Message::A11yBus(update) => match update {
                a11y_bus::Response::Error(err) => {
                    tracing::error!(?err, "screen reader dbus error");
                    let _ = self.screen_reader_dbus_sender.take();
                    self.reader_enabled = false;
                }
                a11y_bus::Response::ScreenReader(enabled) => {
                    self.reader_enabled = enabled;
                }
                a11y_bus::Response::IsEnabled(_) => (),
                a11y_bus::Response::Init(enabled, tx) => {
                    self.reader_enabled = enabled;
                    self.screen_reader_dbus_sender = Some(tx);
                    return cosmic::Task::done(Message::ScreenReaderEnabled(true).into());
                }
            },
            Message::Display(entity) => self.set_active_display(entity),

            Message::UpdateDisplayList(result) => match Arc::into_inner(result) {
                Some(Ok(outputs)) => {
                    self.list = outputs;
                    self.displays.clear();

                    let sorted_outputs = self
                        .list
                        .outputs
                        .iter()
                        .map(|(key, output)| (&*output.name, key))
                        .collect::<BTreeMap<_, _>>();

                    let active_display_name = self
                        .displays
                        .text_remove(self.displays.active())
                        .unwrap_or_default();

                    let mut active_tab_pos: u16 = 0;

                    for (pos, (_name, id)) in sorted_outputs.into_iter().enumerate() {
                        let Some(output) = self.list.outputs.get(id) else {
                            continue;
                        };

                        if output.name == active_display_name {
                            active_tab_pos = pos as u16;
                        }

                        self.displays
                            .insert()
                            .text(output.name.clone())
                            .data::<OutputKey>(id);
                    }

                    self.displays.activate_position(active_tab_pos);
                    self.set_active_display(self.displays.active());
                }

                Some(Err(why)) => {
                    tracing::debug!("error fetching displays: {}", why);
                }

                None => (),
            },
        }

        Task::none()
    }

    fn set_active_display(&mut self, display: segmented_button::Entity) {
        let Some(&output_id) = self.displays.data::<OutputKey>(display) else {
            return;
        };

        let Some(output) = self.list.outputs.get(output_id) else {
            return;
        };

        let scale_u32 = ((output.scale * 100.0) as u32).min(300);
        self.scale = (scale_u32 / 25).checked_sub(2).unwrap_or(2) as usize;
        self.interface_adjusted_scale = ((scale_u32 % 25).min(20) as f32 / 5.0).round() as u32 * 5;
        self.displays.activate(display);
    }

    /// Set the scale of each display.
    fn set_scale(&mut self, option: usize, scale_adjust: u32) -> Task<page::Message> {
        tracing::debug!("setting scale {option} with {scale_adjust}");
        self.scale = option;
        self.interface_adjusted_scale = scale_adjust;

        self.list
            .outputs
            .get_mut(*self.displays.active_data::<OutputKey>().unwrap())
            .and_then(|output| {
                let current = &self.list.modes[output.current?];
                let scale = (option * 25 + 50) as u32 + self.interface_adjusted_scale.min(20);
                output.scale = scale as f64 / 100.0;

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
            .unwrap_or_else(Task::none)
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
    A11yEvent(AccessibilityEvent),
    /// Change the active display tab
    Display(segmented_button::Entity),
    /// Toggle the screen magnifier.
    MagnifierEnabled(bool),
    /// Set the preferred scale for a display.
    Scale(usize),
    /// Adjust the display scale.
    ScaleAdjust(u32),
    /// Status of scale adjust command.
    ScaleAdjustResult(ScaleAdjustResult),
    /// Screen reader DBus events.
    A11yBus(a11y_bus::Response),
    /// Enable the screen reader.
    ScreenReaderEnabled(bool),
    /// Update the display list
    UpdateDisplayList(Arc<Result<cosmic_randr_shell::List, cosmic_randr_shell::Error>>),
}

impl From<Message> for page::Message {
    fn from(message: Message) -> Self {
        page::Message::A11y(message)
    }
}
