// Copyright 2025 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

// TODO: Logic taken from cosmic-settings should be shared.

mod network_manager;
mod nm_secret_agent;

use crate::fl;
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::widget::operation::focus_next;
use cosmic::iced::{Alignment, Length, alignment};
use cosmic::widget::{self, column, icon};
use cosmic::{Apply, Element, Task};
use eyre::Context;
use futures::{SinkExt, StreamExt};
use network_manager::NetworkManagerState;
use network_manager::available_wifi::{AccessPoint, NetworkType};
use network_manager::current_networks::ActiveConnectionInfo;
use nm_secret_agent::SecretSender;
use secure_string::SecureString;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

pub static SECURE_INPUT_WIFI: LazyLock<widget::Id> = LazyLock::new(widget::Id::unique);

#[derive(Debug, Default)]
pub struct Page {
    nm_state: Option<NmState>,
    nm_task: Option<tokio::sync::oneshot::Sender<()>>,
    secret_tx: Option<tokio::sync::mpsc::Sender<nm_secret_agent::Request>>,
    /// When defined, displays connections for the specific device.
    active_device: Option<Arc<network_manager::devices::DeviceInfo>>,
    dialog: Option<WiFiDialog>,
    view_more_popup: Option<network_manager::Ssid>,
    connecting: BTreeSet<network_manager::Ssid>,
    ssid_to_uuid: BTreeMap<Box<str>, Box<str>>,
    /// Withhold device update if the view more popup is shown.
    withheld_devices: Option<Vec<network_manager::devices::DeviceInfo>>,
    /// Withhold state update if the view more popup is shown.
    withheld_state: Option<network_manager::NetworkManagerState>,
    /// Search query for filtering WiFi APs
    search_query: String,
}

impl super::Page for Page {
    fn title(&self) -> String {
        fl!("wireless-page")
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self) -> cosmic::Task<super::Message> {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        self.secret_tx = Some(tx);
        if self.nm_task.is_none() {
            return Task::batch(vec![
                cosmic::Task::future(async move {
                    nmrs::NetworkManager::new()
                        .await
                        .context("failed to connect to NetworkManager")
                        .map_or_else(
                            |why| Message::Error(why.to_string()),
                            Message::NetworkManagerConnect,
                        )
                        .apply(super::Message::from)
                }),
                cosmic::Task::stream(nm_secret_agent::secret_agent_stream(
                    "com.system76.CosmicSettings.WiFi.NetworkManager.SecretAgent",
                    rx,
                ))
                .map(|m| super::Message::from(Message::SecretAgent(m))),
            ]);
        }

        Task::none()
    }

    fn optional(&self) -> bool {
        true
    }

    fn view(&self) -> Element<'_, super::Message> {
        let Some(NmState { ref state, .. }) = self.nm_state else {
            return cosmic::widget::space().into();
        };

        let theme = cosmic::theme::active();
        let spacing = &theme.cosmic().spacing;

        let wifi_enable = widget::settings::item::builder(fl!("wifi"))
            .control(widget::toggler(state.wifi_enabled).on_toggle(Message::WiFiEnable));

        let description = widget::text::body(fl!("wireless-page", "explain"))
            .align_x(alignment::Horizontal::Center)
            .width(Length::Fill);

        let mut view = widget::column::with_capacity(5)
            .push(widget::container(description))
            .push(widget::list_column().add(wifi_enable))
            .push_maybe(state.airplane_mode.then(|| {
                widget::row::with_capacity(2)
                    .push(icon::from_name("airplane-mode-symbolic"))
                    .push(widget::text::body(fl!("wireless-page", "airplane-mode")))
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .apply(widget::container)
                    .center_x(Length::Fill)
            }));

        if !state.airplane_mode
            && state.known_access_points.is_empty()
            && state.wireless_access_points.is_empty()
        {
            let no_networks_found =
                widget::container(widget::text::body(fl!("wireless-page", "no-networks")))
                    .center_x(Length::Fill);

            view = view.push(no_networks_found);
        } else {
            let mut has_known = false;
            let mut has_visible = false;

            // Create separate sections for known and visible networks.
            let (known_networks, visible_networks) = state.wireless_access_points.iter().fold(
                (
                    widget::settings::section().title(fl!("wireless-page", "known-networks")),
                    widget::settings::section().title(fl!("wireless-page", "visible-networks")),
                ),
                |(mut known_networks, mut visible_networks), network| {
                    let is_connected = is_connected(state, network);

                    let is_known = state
                        .known_access_points
                        .iter()
                        .map(|known| known.ssid.as_ref())
                        .chain(state.active_conns.iter().filter_map(|active| {
                            if let ActiveConnectionInfo::WiFi { name, .. } = active {
                                Some(name.as_str())
                            } else {
                                None
                            }
                        }))
                        .any(|known| known == network.ssid.as_ref());

                    // TODO: detect if access point is secured or not.
                    let is_encrypted = true;

                    let (connect_txt, connect_msg) = if is_connected {
                        (fl!("wireless-page", "connected"), None)
                    } else if self.connecting.contains(&network.ssid) {
                        (fl!("wireless-page", "connecting"), None)
                    } else {
                        (
                            fl!("wireless-page", "connect"),
                            Some(if is_known || !is_encrypted {
                                Message::Connect(network.ssid.clone())
                            } else {
                                Message::PasswordRequest(network.ssid.clone())
                            }),
                        )
                    };

                    let identifier = widget::row::with_capacity(3)
                        .push(widget::icon::from_name(wifi_icon(network.strength)))
                        .push_maybe(
                            is_encrypted
                                .then(|| widget::icon::from_name("connection-secure-symbolic")),
                        )
                        .push(widget::text::body(network.ssid.as_ref()).wrapping(Wrapping::Glyph))
                        .spacing(spacing.space_xxs);

                    let connect: Element<'_, Message> = if let Some(msg) = connect_msg {
                        widget::button::text(connect_txt).on_press(msg).into()
                    } else {
                        widget::text::body(connect_txt)
                            .align_y(Alignment::Center)
                            .into()
                    };

                    let view_more_button =
                        widget::button::icon(widget::icon::from_name("view-more-symbolic"));

                    let view_more: Option<Element<_>> = if self
                        .view_more_popup
                        .as_deref()
                        .is_some_and(|id| id == network.ssid.as_ref())
                    {
                        widget::popover(view_more_button.on_press(Message::ViewMore(None)))
                            .position(widget::popover::Position::Bottom)
                            .on_close(Message::ViewMore(None))
                            .popup({
                                widget::column::with_capacity(3)
                                    .push_maybe(is_connected.then(|| {
                                        popup_button(
                                            Message::Disconnect(network.ssid.clone()),
                                            fl!("wireless-page", "disconnect"),
                                        )
                                    }))
                                    .push(popup_button(
                                        Message::Settings(network.ssid.clone()),
                                        fl!("settings"),
                                    ))
                                    .push_maybe(is_known.then(|| {
                                        popup_button(
                                            Message::ForgetRequest(network.ssid.clone()),
                                            fl!("wireless-page", "forget"),
                                        )
                                    }))
                                    .width(Length::Fixed(170.0))
                                    .apply(widget::container)
                                    .class(cosmic::style::Container::Dialog(true))
                            })
                            .apply(|e| Some(Element::from(e)))
                    } else if is_known {
                        view_more_button
                            .on_press(Message::ViewMore(Some(network.ssid.clone())))
                            .apply(|e| Some(Element::from(e)))
                    } else {
                        None
                    };

                    let controls = widget::row::with_capacity(2)
                        .push(connect)
                        .push_maybe(view_more)
                        .align_y(Alignment::Center)
                        .spacing(spacing.space_xxs);

                    let widget = widget::settings::item_row(vec![
                        identifier.into(),
                        widget::space::horizontal().into(),
                        controls.into(),
                    ]);

                    if is_known {
                        has_known = true;
                        known_networks = known_networks.add(widget);
                    } else {
                        has_visible = true;
                        visible_networks = visible_networks.add(widget);
                    }

                    (known_networks, visible_networks)
                },
            );

            if has_known || has_visible {
                let networks = widget::column::with_capacity(2)
                    .spacing(spacing.space_l)
                    .push_maybe((has_known).then_some(known_networks))
                    .push_maybe((has_visible).then_some(visible_networks));

                view = view.push(widget::scrollable(networks));
            }
        };

        view.spacing(spacing.space_l)
            .width(Length::Fill)
            .apply(Element::from)
            .map(super::Message::WiFi)
    }

    fn dialog(&self) -> Option<Element<'_, super::Message>> {
        self.dialog.as_ref().map(|dialog| match dialog {
            WiFiDialog::Password {
                password,
                identity,
                password_hidden,
                ..
            } => {
                let password = widget::text_input::secure_input(
                    fl!("password"),
                    password.unsecure(),
                    Some(Message::TogglePasswordVisibility.into()),
                    *password_hidden,
                )
                .id(SECURE_INPUT_WIFI.clone())
                .on_input(|input| Message::PasswordUpdate(SecureString::from(input)).into())
                .on_submit(|_| Message::ConnectWithPassword.into());

                let primary_action = widget::button::suggested(fl!("wireless-page", "connect"))
                    .on_press(Message::ConnectWithPassword.into());

                let secondary_action =
                    widget::button::standard(fl!("cancel")).on_press(Message::CancelDialog.into());

                let control: Element<_> = if let Some(identity) = identity {
                    column::with_capacity(2)
                        .spacing(8)
                        .push(
                            widget::text_input::text_input(fl!("identity"), identity)
                                .on_input(|identity| Message::IdentityUpdate(identity).into())
                                .on_submit(|_| Message::SubmitIdentity.into()),
                        )
                        .push(password)
                        .into()
                } else {
                    password.into()
                };

                widget::dialog()
                    .title(fl!("auth-dialog"))
                    .icon(icon::from_name("preferences-wireless-symbolic").size(64))
                    .body(fl!("auth-dialog", "wifi-description"))
                    .control(control)
                    .primary_action(primary_action)
                    .secondary_action(secondary_action)
                    .apply(Element::from)
            }

            WiFiDialog::Forget(ssid) => {
                let primary_action = widget::button::destructive(fl!("wireless-page", "forget"))
                    .on_press(Message::Forget(ssid.clone()).into());

                let secondary_action =
                    widget::button::standard(fl!("cancel")).on_press(Message::CancelDialog.into());

                widget::dialog()
                    .title(fl!("forget-dialog"))
                    .icon(icon::from_name("dialog-information").size(64))
                    .body(fl!("forget-dialog", "description"))
                    .primary_action(primary_action)
                    .secondary_action(secondary_action)
                    .apply(Element::from)
            }
        })
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    /// Add a network connection with nm-connection-editor
    AddNetwork,
    /// Cancels a dialog.
    CancelDialog,
    /// Connect to a WiFi network access point.
    Connect(network_manager::Ssid),
    /// Connect with a password
    ConnectWithPassword,
    /// Settings for known connections.
    ConnectionSettings(BTreeMap<Box<str>, Box<str>>),
    /// Disconnect from an access point.
    Disconnect(network_manager::Ssid),
    /// An error occurred.
    Error(String),
    /// Identity update from the dialog
    IdentityUpdate(String),
    /// Focus the secure input
    FocusSecureInput,
    /// Create a dialog to ask for confirmation on forgetting a connection.
    ForgetRequest(network_manager::Ssid),
    /// Forget a known access point.
    Forget(network_manager::Ssid),
    /// An update from the network manager daemon
    NetworkManager(network_manager::Event),
    /// Connected to the network manager daemon
    NetworkManagerConnect(nmrs::NetworkManager),
    /// Request an auth dialog
    PasswordRequest(network_manager::Ssid),
    /// Update the password from the dialog
    PasswordUpdate(SecureString),
    /// Update search query for filtering networks
    SearchQuery(String),
    /// An update from the secret agent
    SecretAgent(nm_secret_agent::Event),
    /// Selects a device to display connections from
    SelectDevice(Arc<network_manager::devices::DeviceInfo>),
    /// Opens settings page for the access point.
    Settings(network_manager::Ssid),
    /// Identity submitted from the dialog
    SubmitIdentity,
    /// Toggles visibility of the password input
    TogglePasswordVisibility,
    /// Update NetworkManagerState
    UpdateState(NetworkManagerState),
    /// Update the devices lists
    UpdateDevices(Vec<network_manager::devices::DeviceInfo>),
    /// Display more options for an access point
    ViewMore(Option<network_manager::Ssid>),
    /// Toggle WiFi access
    WiFiEnable(bool),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::WiFi(message)
    }
}

impl From<Message> for crate::Message {
    fn from(message: Message) -> Self {
        crate::Message::PageMessage(super::Message::WiFi(message))
    }
}

#[derive(Clone, Debug)]
enum WiFiDialog {
    Forget(network_manager::Ssid),
    Password {
        ssid: network_manager::Ssid,
        identity: Option<String>,
        password: SecureString,
        network_type: NetworkType,
        password_hidden: bool,
        tx: SecretSender,
    },
}

#[derive(Debug)]
pub struct NmState {
    conn: nmrs::NetworkManager,
    sender: futures::channel::mpsc::UnboundedSender<network_manager::Request>,
    state: network_manager::NetworkManagerState,
    devices: Vec<network_manager::devices::DeviceInfo>,
}

impl Page {
    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        let span = tracing::span!(tracing::Level::INFO, "wifi::update");
        let _span = span.enter();

        match message {
            Message::NetworkManager(network_manager::Event::RequestResponse {
                req,
                state,
                success,
            }) => {
                if !success {
                    tracing::error!(request = ?req, "network-manager request failed");
                }

                match req {
                    network_manager::Request::Authenticate {
                        ssid,
                        identity,
                        network_type,
                        ..
                    } => {
                        if success {
                            self.connecting.remove(ssid.as_str());
                        } else {
                            self.connecting.remove(ssid.as_str());
                            // Request to retry
                            self.dialog = Some(WiFiDialog::Password {
                                ssid: ssid.into(),
                                identity,
                                password: SecureString::from(""),
                                network_type,
                                password_hidden: true,
                                tx: Arc::new(Mutex::new(None)),
                            });
                            return cosmic::task::message(Message::FocusSecureInput);
                        }
                    }

                    network_manager::Request::SelectAccessPoint(
                        ssid,
                        network_type,
                        _tx,
                        _interface,
                    ) => {
                        if success || matches!(network_type, NetworkType::Open) {
                            self.connecting.remove(ssid.as_ref());
                        } else {
                            self.connecting.remove(ssid.as_ref());
                            self.dialog = Some(WiFiDialog::Password {
                                ssid,
                                identity: matches!(network_type, NetworkType::Eap)
                                    .then(String::new),
                                password: SecureString::from(""),
                                network_type,
                                password_hidden: true,
                                tx: Arc::new(Mutex::new(None)),
                            });
                            return cosmic::task::message(Message::FocusSecureInput);
                        }
                    }

                    _ => (),
                }

                self.update_state(state);

                if let Some(NmState { ref conn, .. }) = self.nm_state {
                    return update_devices(conn.clone());
                }
            }
            Message::UpdateDevices(devices) => {
                self.update_devices(devices);
            }
            Message::UpdateState(state) => {
                self.update_state(state);

                if let Some(NmState { ref conn, .. }) = self.nm_state {
                    return connection_settings(conn.clone());
                }
            }
            Message::NetworkManager(
                network_manager::Event::ActiveConns
                | network_manager::Event::Devices
                | network_manager::Event::WiFiEnabled(_)
                | network_manager::Event::WirelessAccessPoints,
            ) => {
                if let Some(NmState { ref conn, .. }) = self.nm_state {
                    return cosmic::Task::batch(vec![
                        update_state(conn.clone()),
                        update_devices(conn.clone()),
                    ]);
                }
            }
            Message::ConnectionSettings(settings) => {
                self.ssid_to_uuid = settings;
            }
            Message::NetworkManager(network_manager::Event::Init {
                conn,
                sender,
                state,
            }) => {
                self.nm_state = Some(NmState {
                    conn: conn.clone(),
                    sender,
                    state,
                    devices: Vec::new(),
                });

                return update_devices(conn);
            }
            Message::NetworkManager(network_manager::Event::WiFiCredentials { .. }) => (),
            Message::AddNetwork => {
                tokio::task::spawn(nm_add_wifi());
            }
            Message::Connect(ssid) => {
                if let Some(nm) = self.nm_state.as_mut() {
                    let Some(ap) = nm
                        .state
                        .wireless_access_points
                        .iter()
                        .chain(nm.state.known_access_points.iter())
                        .find(|ap| ap.ssid == ssid)
                    else {
                        return Task::none();
                    };
                    self.connecting.insert(ssid.clone());
                    _ = nm
                        .sender
                        .unbounded_send(network_manager::Request::SelectAccessPoint(
                            ssid,
                            ap.network_type,
                            self.secret_tx.clone(),
                            self.active_device.as_ref().map(|d| d.interface.clone()),
                        ));
                }
            }
            Message::IdentityUpdate(new_identity) => {
                if let Some(WiFiDialog::Password {
                    ref mut identity, ..
                }) = self.dialog
                {
                    *identity = Some(new_identity);
                }
            }
            Message::PasswordRequest(ssid) => {
                if let Some(nm) = self.nm_state.as_mut() {
                    let Some(ap) = nm
                        .state
                        .wireless_access_points
                        .iter()
                        .chain(nm.state.known_access_points.iter())
                        .find(|ap| ap.ssid == ssid)
                    else {
                        return Task::none();
                    };

                    self.dialog = Some(WiFiDialog::Password {
                        ssid,
                        identity: matches!(ap.network_type, NetworkType::Eap).then(String::new),
                        password: SecureString::from(""),
                        network_type: ap.network_type,
                        password_hidden: true,
                        tx: Arc::new(Mutex::new(None)),
                    });
                    return cosmic::task::message(Message::FocusSecureInput);
                }
            }
            Message::PasswordUpdate(pass) => {
                if let Some(WiFiDialog::Password {
                    ref mut password, ..
                }) = self.dialog
                {
                    *password = pass;
                }
            }
            Message::ConnectWithPassword => {
                let Some(dialog) = self.dialog.take() else {
                    return Task::none();
                };

                if let WiFiDialog::Password {
                    ssid,
                    identity,
                    password,
                    network_type,
                    tx,
                    ..
                } = dialog
                    && let Some(nm) = self.nm_state.as_mut()
                {
                    self.connecting.insert(ssid.clone());
                    let nm_sender = nm.sender.clone();
                    let secret_tx = self.secret_tx.clone();
                    let interface = self.active_device.as_ref().map(|d| d.interface.clone());
                    return Task::future(async move {
                        let mut guard = tx.lock().await;
                        if let Some(tx) = guard.take() {
                            _ = tx.send(password);
                        } else {
                            _ = nm_sender.unbounded_send(network_manager::Request::Authenticate {
                                ssid: ssid.to_string(),
                                identity,
                                password,
                                network_type,
                                secret_tx,
                                interface,
                            });
                        }
                    })
                    .discard();
                }
            }
            Message::TogglePasswordVisibility => {
                if let Some(WiFiDialog::Password {
                    ref mut password_hidden,
                    ..
                }) = self.dialog
                {
                    *password_hidden = !*password_hidden;
                }
            }
            Message::ViewMore(ssid) => {
                self.view_more_popup = ssid;
                if self.view_more_popup.is_none() {
                    self.close_popup_and_apply_updates();
                }
            }
            Message::Disconnect(ssid) => {
                self.close_popup_and_apply_updates();
                self.connecting.remove(ssid.as_ref());
                if let Some(nm) = self.nm_state.as_mut() {
                    _ = nm
                        .sender
                        .unbounded_send(network_manager::Request::Disconnect(ssid));
                }
            }
            Message::ForgetRequest(ssid) => {
                self.dialog = Some(WiFiDialog::Forget(ssid));
                self.view_more_popup = None;
            }
            Message::Forget(ssid) => {
                self.dialog = None;
                self.close_popup_and_apply_updates();
                self.connecting.remove(ssid.as_ref());
                if let Some(nm) = self.nm_state.as_mut() {
                    _ = nm
                        .sender
                        .unbounded_send(network_manager::Request::Forget(ssid));
                }
            }
            Message::Settings(ssid) => {
                self.close_popup_and_apply_updates();

                if let Some(uuid) = self.ssid_to_uuid.get(ssid.as_ref()).cloned() {
                    tokio::task::spawn(async move { nm_edit_connection(uuid.as_ref()).await });
                }
            }
            Message::SubmitIdentity => {
                if self.dialog.is_some() {
                    return focus_next();
                }
            }
            Message::WiFiEnable(enable) => {
                if !enable {
                    self.connecting.clear();
                }
                if let Some(nm) = self.nm_state.as_mut() {
                    _ = nm
                        .sender
                        .unbounded_send(network_manager::Request::SetWiFi(enable));
                    _ = nm.sender.unbounded_send(network_manager::Request::Reload);
                }
            }
            Message::CancelDialog => {
                if let Some(WiFiDialog::Password { ssid, .. }) = self.dialog.take() {
                    self.connecting.remove(ssid.as_ref());
                }
            }
            Message::Error(why) => {
                tracing::error!(why);
            }
            Message::SelectDevice(device) => {
                // TODO: Per-device wifi connection handling.
                self.active_device = Some(device);
            }

            Message::SearchQuery(query) => {
                self.search_query = query;
            }

            Message::NetworkManagerConnect(conn) => {
                return cosmic::task::batch(vec![
                    self.connect(conn.clone()),
                    connection_settings(conn),
                ]);
            }
            Message::SecretAgent(event) => match event {
                nm_secret_agent::Event::RequestSecret {
                    uuid,
                    name,
                    description: _, // TODO do we want to display the description?
                    previous,
                    tx,
                } => {
                    let ssid = self
                        .ssid_to_uuid
                        .iter()
                        .find_map(|(ssid, conn_uuid)| {
                            if conn_uuid.as_ref() == name.as_str() {
                                Some(network_manager::Ssid::from(ssid.as_ref()))
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();
                    let Some(ap): Option<&AccessPoint> = self.nm_state.as_ref().and_then(|nm| {
                        nm.state
                            .wireless_access_points
                            .iter()
                            .chain(nm.state.known_access_points.iter())
                            .find(|ap| ap.ssid == ssid)
                    }) else {
                        tracing::error!(
                            %uuid,
                            %name,
                            "received secret request for unknown connection"
                        );
                        return Task::none();
                    };

                    self.dialog = Some(WiFiDialog::Password {
                        ssid,
                        password: previous,
                        password_hidden: true,
                        identity: matches!(ap.network_type, NetworkType::Eap).then(String::new),
                        network_type: ap.network_type,
                        tx,
                    });
                    return cosmic::task::message(Message::FocusSecureInput);
                }
                nm_secret_agent::Event::CancelGetSecrets { uuid: _, name: _ } => {
                    match self.dialog.take() {
                        Some(WiFiDialog::Password { ssid, .. }) => {
                            self.connecting.remove(ssid.as_ref());
                        }
                        other => {
                            self.dialog = other;
                        }
                    }
                }
                nm_secret_agent::Event::Failed(error) => {
                    tracing::error!(%error, "secret agent failure");
                    if let Some(WiFiDialog::Password {
                        ssid,
                        password,
                        identity,
                        network_type,
                        ..
                    }) = self.dialog.take()
                    {
                        self.connecting.remove(ssid.as_ref());
                        self.dialog = Some(WiFiDialog::Password {
                            password,
                            password_hidden: true,
                            tx: Arc::new(Mutex::new(None)),
                            ssid,
                            identity,
                            network_type,
                        });
                        return cosmic::task::message(Message::FocusSecureInput);
                    }
                }
            },
            Message::FocusSecureInput => {
                // retry until the widget is in the tree and focused or the dialog is removed.
                if matches!(self.dialog, Some(WiFiDialog::Password { .. })) {
                    return cosmic::iced::runtime::task::widget(
                        cosmic::iced::core::widget::operation::focusable::find_focused(),
                    )
                    .collect()
                    .then(|id| {
                        if id
                            .first()
                            .is_some_and(|id| *id == SECURE_INPUT_WIFI.clone())
                        {
                            Task::none()
                        } else {
                            cosmic::widget::text_input::focus(SECURE_INPUT_WIFI.clone())
                                .chain(cosmic::task::message(Message::FocusSecureInput))
                        }
                    });
                }
            }
        }

        Task::none()
    }

    fn connect(&mut self, conn: nmrs::NetworkManager) -> Task<crate::Message> {
        fn forward_event_loop<M: 'static + Send, T: Future<Output = ()> + Send + 'static>(
            event_loop: impl FnOnce(futures::channel::mpsc::Sender<M>) -> T + Send + 'static,
        ) -> (tokio::sync::oneshot::Sender<()>, cosmic::Task<M>) {
            let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

            let task =
                cosmic::Task::stream(cosmic::iced::stream::channel(1, |emitter| async move {
                    futures::future::select(
                        std::pin::pin!(cancel_rx),
                        std::pin::pin!(event_loop(emitter)),
                    )
                    .await;
                }));

            (cancel_tx, task)
        }

        if self.nm_task.is_none() {
            let (canceller, task) = forward_event_loop(move |mut sender| async move {
                let (tx, mut rx) = futures::channel::mpsc::channel(1);

                let watchers = std::pin::pin!(async move {
                    network_manager::watch(conn, tx).await;
                });

                let forwarder = std::pin::pin!(async move {
                    while let Some(event) = rx.next().await {
                        _ = sender.send(Message::NetworkManager(event).into()).await;
                    }
                });

                futures::future::select(watchers, forwarder).await;
            });

            self.nm_task = Some(canceller);
            return task.map(crate::Message::PageMessage);
        }

        Task::none()
    }

    /// Closes the view more popup and applies any withheld updates.
    fn close_popup_and_apply_updates(&mut self) {
        self.view_more_popup = None;
        if let Some(ref mut nm_state) = self.nm_state {
            if let Some(state) = self.withheld_state.take() {
                nm_state.state = state;
            }

            if let Some(devices) = self.withheld_devices.take() {
                nm_state.devices = devices;
            }
        }
    }

    /// Withholds updates if the view more popup is displayed.
    fn update_devices(&mut self, devices: Vec<network_manager::devices::DeviceInfo>) {
        if let Some(ref mut nm_state) = self.nm_state {
            if self.view_more_popup.is_some() {
                self.withheld_devices = Some(devices);
            } else {
                nm_state.devices = devices;
            }
        }
    }

    /// Withholds updates if the view more popup is displayed.
    fn update_state(&mut self, state: NetworkManagerState) {
        for active in &state.active_conns {
            if let ActiveConnectionInfo::WiFi { name, .. } = active {
                self.connecting.remove(name.as_str());
            }
        }

        if let Some(ref mut nm_state) = self.nm_state {
            if self.view_more_popup.is_some() {
                self.withheld_state = Some(state);
            } else {
                nm_state.state = state;
            }
        }
    }
}
fn is_connected(state: &NetworkManagerState, network: &AccessPoint) -> bool {
    state.active_conns.iter().any(|active| {
        if let ActiveConnectionInfo::WiFi { name, .. } = active {
            *name == network.ssid.as_ref()
        } else {
            false
        }
    })
}

fn popup_button(message: Message, text: String) -> Element<'static, Message> {
    let spacing = cosmic::theme::spacing();
    widget::text::body(text)
        .align_y(Alignment::Center)
        .apply(widget::button::custom)
        .padding([spacing.space_xxxs, spacing.space_xs])
        .width(Length::Fill)
        .class(cosmic::theme::Button::MenuItem)
        .on_press(message)
        .into()
}

fn connection_settings(conn: nmrs::NetworkManager) -> Task<crate::Message> {
    cosmic::task::future(async move {
        network_manager::wifi_connection_settings(conn)
            .await
            .context("failed to get connection settings")
            .map_or_else(
                |why| Message::Error(why.to_string()),
                Message::ConnectionSettings,
            )
    })
}

pub fn update_state(conn: nmrs::NetworkManager) -> Task<crate::Message> {
    cosmic::task::future(async move {
        match NetworkManagerState::new(&conn).await {
            Ok(state) => Message::UpdateState(state),
            Err(why) => Message::Error(why.to_string()),
        }
    })
}

pub fn update_devices(conn: nmrs::NetworkManager) -> Task<crate::Message> {
    cosmic::task::future(async move {
        let filter =
            |device_type| matches!(device_type, network_manager::devices::DeviceType::Wifi);
        match network_manager::devices::list(&conn, filter).await {
            Ok(devices) => Message::UpdateDevices(devices),
            Err(why) => Message::Error(why.to_string()),
        }
    })
}

fn wifi_icon(strength: u8) -> &'static str {
    if strength < 25 {
        "network-wireless-signal-weak-symbolic"
    } else if strength < 50 {
        "network-wireless-signal-ok-symbolic"
    } else if strength < 75 {
        "network-wireless-signal-good-symbolic"
    } else {
        "network-wireless-signal-excellent-symbolic"
    }
}

async fn nm_add_wifi() -> Result<(), String> {
    nm_connection_editor(&["--type=802-11-wireless", "-c"]).await
}

async fn nm_edit_connection(uuid: &str) -> Result<(), String> {
    nm_connection_editor(&[&["--edit=", uuid].concat()]).await
}

async fn nm_connection_editor(args: &[&str]) -> Result<(), String> {
    tokio::process::Command::new("nm-connection-editor")
        .args(args)
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|why| why.to_string())
        .and_then(|output| {
            if !output.status.success() {
                Err(String::from_utf8(output.stderr).unwrap_or_default())
            } else {
                Ok(())
            }
        })
}
