// Copyright 2025 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use crate::fl;
use cosmic::{
    Apply, Element,
    dialog::file_chooser,
    iced::Length,
    widget::{self, icon},
};
use pwhash::{bcrypt, md5_crypt, sha256_crypt, sha512_crypt};
use regex::Regex;
use std::{
    collections::HashMap,
    fs::File,
    future::Future,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::Arc,
};
use url::Url;
use zbus_polkit::policykit1::CheckAuthorizationFlags;

const DEFAULT_ICON_FILE: &str = "/usr/share/pixmaps/faces/pop-robot.png";
const USERS_ADMIN_POLKIT_POLICY_ID: &str = "com.system76.CosmicSettings.Users.Admin";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorField {
    FullName,
    Username,
    Password,
    PasswordConfirm,
}

#[derive(Clone, Debug)]
pub struct Page {
    default_icon: icon::Handle,
    profile_icon: Option<icon::Handle>,
    profile_icon_path: PathBuf,
    password: String,
    password_label: String,
    password_confirm: String,
    password_confirm_label: String,
    username: String,
    username_label: String,
    full_name: String,
    fullname_label: String,
    password_hidden: bool,
    password_confirm_hidden: bool,
    user_info_complete: bool,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            default_icon: icon::from_path(PathBuf::from(DEFAULT_ICON_FILE)),
            password_label: fl!("password"),
            password_confirm_label: fl!("password-confirm"),
            username_label: fl!("create-account-page", "user-name"),
            fullname_label: fl!("create-account-page", "full-name"),
            profile_icon: None,
            profile_icon_path: DEFAULT_ICON_FILE.into(),
            username: String::new(),
            full_name: String::new(),
            password: String::new(),
            password_confirm: String::new(),
            password_hidden: true,
            password_confirm_hidden: true,
            user_info_complete: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Edit(EditorField, String),
    SelectedProfileImage(Arc<Result<Url, file_chooser::Error>>),
    SelectProfileImage,
    TogglePasswordConfirmVisibility,
    TogglePasswordVisibility,
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::User(message)
    }
}

impl From<Message> for crate::Message {
    fn from(message: Message) -> Self {
        crate::Message::PageMessage(message.into())
    }
}

impl super::Page for Page {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn title(&self) -> String {
        fl!("create-account-page")
    }

    fn completed(&self) -> bool {
        self.user_info_complete
    }

    fn view(&self) -> Element<'_, super::Message> {
        let profile_image_selector = {
            let profile_icon_handle = self
                .profile_icon
                .clone()
                .unwrap_or_else(|| self.default_icon.clone());

            widget::button::icon(profile_icon_handle)
                .extra_large()
                .icon_size(103)
                .padding(0)
                .on_press(super::Message::User(Message::SelectProfileImage))
                .apply(widget::container)
                .center_x(Length::Fill)
        };

        let full_name_input = widget::container(
            widget::text_input("", &self.full_name)
                .label(&self.fullname_label)
                .on_input(|value| Message::Edit(EditorField::FullName, value).into()),
        );

        let username_input = widget::column::with_capacity(2)
            .push(
                widget::text_input("", &self.username)
                    .label(&self.username_label)
                    .on_input(|value| Message::Edit(EditorField::Username, value).into()),
            )
            .push(widget::text::caption(fl!(
                "create-account-page",
                "user-name-note"
            )));

        let password_input = widget::container(
            widget::secure_input(
                "",
                &self.password,
                Some(Message::TogglePasswordVisibility.into()),
                self.password_hidden,
            )
            .label(&self.password_label)
            .on_input(|value| Message::Edit(EditorField::Password, value).into()),
        );

        let password_confirm_input = widget::container(
            widget::secure_input(
                "",
                &self.password_confirm,
                Some(Message::TogglePasswordConfirmVisibility.into()),
                self.password_confirm_hidden,
            )
            .label(&self.password_confirm_label)
            .on_input(|value| Message::Edit(EditorField::PasswordConfirm, value).into()),
        );

        widget::column()
            .push(profile_image_selector)
            .push(full_name_input)
            .push(username_input)
            .push(password_input)
            .push(password_confirm_input)
            .push(widget::vertical_space().height(cosmic::theme::spacing().space_s))
            .spacing(cosmic::theme::spacing().space_s)
            .into()
    }

    fn apply_settings(&mut self) -> cosmic::Task<super::Message> {
        let username = std::mem::take(&mut self.username);
        let full_name = std::mem::take(&mut self.username);
        let password = std::mem::take(&mut self.password);
        let icon_file = self
            .profile_icon_path
            .to_str()
            .unwrap_or(DEFAULT_ICON_FILE)
            .to_owned();
        let is_admin = true;

        cosmic::Task::future(async move {
            let Ok(conn) = zbus::Connection::system().await else {
                return;
            };

            let accounts = accounts_zbus::AccountsProxy::new(&conn).await.unwrap();

            let user_result = request_permission_on_denial(&conn, || {
                accounts.create_user(&username, &full_name, if is_admin { 1 } else { 0 })
            })
            .await;

            let user_object_path = match user_result {
                Ok(path) => path,

                Err(why) => {
                    tracing::error!(?why, "failed to create user account");
                    return;
                }
            };

            let password_hashed = hash_password(&password);
            match accounts_zbus::UserProxy::new(&conn, user_object_path).await {
                Ok(user) => {
                    _ = user.set_password(&password_hashed, "").await;
                    _ = user.set_icon_file(&icon_file).await;
                    _ = user.set_account_type(1).await;

                    // Ask the greeter to move account files to the new user's home directory.
                    if let Ok(mut client) = crate::greeter::GreeterProxy::new(&conn).await {
                        _ = client.initial_setup_end(username).await;
                    }
                }

                Err(why) => {
                    tracing::error!(?why, "failed to get user by object path");
                }
            }
        })
        .discard()
    }
}

impl Page {
    pub fn update(&mut self, message: Message) -> cosmic::Task<super::Message> {
        match message {
            Message::SelectProfileImage => {
                return cosmic::task::future(async move {
                    let dialog_result = file_chooser::open::Dialog::new()
                        .title(fl!("create-account-page", "profile-add"))
                        .accept_label(fl!("create-account-page", "dialog-add"))
                        .modal(false)
                        .open_file()
                        .await
                        .map(|response| response.url().to_owned());

                    Message::SelectedProfileImage(Arc::new(dialog_result))
                });
            }

            Message::SelectedProfileImage(image_result) => {
                let url = match Arc::into_inner(image_result).unwrap() {
                    Ok(url) => url,
                    Err(why) => {
                        tracing::error!(?why, "failed to get image file");
                        return cosmic::Task::none();
                    }
                };

                let Ok(path) = url.to_file_path() else {
                    tracing::error!("selected image is not a file path");
                    return cosmic::Task::none();
                };

                self.profile_icon_path = path.clone();
                self.profile_icon = Some(icon::from_path(path));
            }

            Message::Edit(field, value) => {
                match field {
                    EditorField::FullName => {
                        self.full_name = value;

                        // Generate username based on the full name.
                        self.username.clear();
                        for char in self.full_name.chars() {
                            if char.is_alphabetic() {
                                self.username.push(char.to_ascii_lowercase());
                            }
                        }
                    }
                    EditorField::Username => {
                        if username_valid(&value) {
                            self.username = value;
                        }
                    }
                    EditorField::Password => {
                        self.password = value;
                    }
                    EditorField::PasswordConfirm => {
                        self.password_confirm = value;
                    }
                }

                self.user_info_complete = password_valid(&self.password, &self.password_confirm)
                    && username_valid(&self.username);
            }

            Message::TogglePasswordVisibility => {
                self.password_hidden = !self.password_hidden;
            }

            Message::TogglePasswordConfirmVisibility => {
                self.password_confirm_hidden = !self.password_confirm_hidden;
            }
        };

        cosmic::Task::none()
    }
}

async fn check_authorization(conn: &zbus::Connection) -> eyre::Result<()> {
    let proxy = zbus_polkit::policykit1::AuthorityProxy::new(conn).await?;
    let subject = zbus_polkit::policykit1::Subject::new_for_owner(std::process::id(), None, None)?;
    proxy
        .check_authorization(
            &subject,
            USERS_ADMIN_POLKIT_POLICY_ID,
            &HashMap::new(),
            CheckAuthorizationFlags::AllowUserInteraction.into(),
            "",
        )
        .await?;
    Ok(())
}

async fn request_permission_on_denial<T, Fun, Fut>(
    conn: &zbus::Connection,
    action: Fun,
) -> zbus::Result<T>
where
    Fun: Fn() -> Fut,
    Fut: Future<Output = zbus::Result<T>>,
{
    match action().await {
        Ok(value) => Ok(value),
        Err(why) => {
            if permission_was_denied(&why) {
                _ = check_authorization(conn).await;
                action().await
            } else {
                Err(why)
            }
        }
    }
}

fn permission_was_denied(result: &zbus::Error) -> bool {
    match result {
        zbus::Error::MethodError(name, _, _)
            if name.as_str() == "org.freedesktop.Accounts.Error.PermissionDenied" =>
        {
            true
        }
        _ => false,
    }
}

// TODO: Should we allow deprecated methods?
fn hash_password(password_plain: &str) -> String {
    match get_encrypt_method().as_str() {
        "SHA512" => sha512_crypt::hash(password_plain).unwrap(),
        "SHA256" => sha256_crypt::hash(password_plain).unwrap(),
        "MD5" => md5_crypt::hash(password_plain).unwrap(),
        _ => bcrypt::hash(password_plain).unwrap(),
    }
}

// TODO: In the future loading in the whole login.defs file into an object might be handy?
// For now, just grabbing what we need
fn get_encrypt_method() -> String {
    let mut value = String::new();
    let login_defs = if let Ok(file) = File::open("/etc/login.defs") {
        file
    } else {
        return value;
    };
    let reader = BufReader::new(login_defs);

    for line in reader.lines() {
        if let Ok(line) = line {
            if !line.trim().is_empty() {
                if let Some(index) = line.find(|c: char| c.is_whitespace()) {
                    let key = line[0..index].trim();
                    if key == "ENCRYPT_METHOD" {
                        value = line[(index + 1)..].trim().to_string();
                    }
                }
            }
        }
    }
    value
}

fn username_valid(username: &str) -> bool {
    Regex::new("^[a-z][a-z0-9-]{0,30}$")
        .unwrap()
        .is_match(&username)
}

fn password_valid(password: &str, password_confirm: &str) -> bool {
    password == password_confirm && !password.is_empty() && !password_confirm.is_empty()
}
