use futures::{FutureExt, SinkExt, StreamExt};
use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretRequest, SecretResponder, SecretSetting};
use secure_string::SecureString;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::oneshot;
use zbus::zvariant::{OwnedValue, Str};

const SECRET_ID: &str = "com.system76.CosmicSettings.NetworkManager";

pub type SecretSender = Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<SecureString>>>>;

#[derive(Clone, Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone)]
pub enum Event {
    RequestSecret {
        uuid: String,
        name: String,
        description: Option<String>,
        previous: SecureString,
        tx: SecretSender,
    },
    CancelGetSecrets {
        uuid: String,
        name: String,
    },
    Failed(Error),
}

#[derive(Debug)]
pub enum Request {
    SetSecrets {
        setting_name: String,
        uuid: String,
        secrets: HashMap<String, SecureString>,
        applied_tx: oneshot::Sender<()>,
    },
    GetSecrets {
        setting_name: String,
        uuid: String,
        resp_tx: oneshot::Sender<HashMap<String, SecureString>>,
    },
}

pub fn secret_agent_stream(
    identifier: impl AsRef<str>,
    rx: tokio::sync::mpsc::Receiver<Request>,
) -> impl futures::Stream<Item = Event> {
    let identifier = identifier.as_ref().to_string();
    cosmic::iced::stream::channel(
        4,
        move |mut msg_tx: futures::channel::mpsc::Sender<Event>| async move {
            if let Err(error) = secret_agent_stream_impl(&identifier, msg_tx.clone(), rx).await {
                _ = msg_tx.send(Event::Failed(error)).await;
            }
        },
    )
}

async fn secret_agent_stream_impl(
    identifier: &str,
    mut msg_tx: futures::channel::mpsc::Sender<Event>,
    mut rx: tokio::sync::mpsc::Receiver<Request>,
) -> Result<(), Error> {
    unlock_collection().await?;

    let (_handle, mut requests) = SecretAgent::builder()
        .with_identifier(identifier)
        .register()
        .await
        .map_err(|err| Error(err.to_string()))?;

    loop {
        futures::select! {
            request = rx.recv().fuse() => {
                let Some(request) = request else {
                    break;
                };
                handle_control_request(request).await?;
            }
            request = requests.next().fuse() => {
                let Some(request) = request else {
                    break;
                };
                handle_secret_request(&mut msg_tx, request).await?;
            }
        }
    }

    Ok(())
}

async fn handle_control_request(request: Request) -> Result<(), Error> {
    match request {
        Request::SetSecrets {
            setting_name,
            uuid,
            secrets,
            applied_tx,
        } => {
            if secrets.is_empty() {
                delete_secrets(&uuid).await?;
            } else {
                store_secrets(&uuid, &setting_name, &secrets).await?;
            }
            _ = applied_tx.send(());
        }
        Request::GetSecrets {
            setting_name,
            uuid,
            resp_tx,
        } => {
            let secrets = get_secrets(&uuid, &setting_name).await?;
            _ = resp_tx.send(secrets);
        }
    }

    Ok(())
}

async fn handle_secret_request(
    msg_tx: &mut futures::channel::mpsc::Sender<Event>,
    request: SecretRequest,
) -> Result<(), Error> {
    let setting_name = setting_name(&request.setting);
    if !request.flags.contains(SecretAgentFlags::REQUEST_NEW) {
        let stored = get_secrets(&request.connection_uuid, setting_name).await?;
        if !stored.is_empty() {
            return respond_with_stored(request, stored).await;
        }
    }

    if !request
        .flags
        .intersects(SecretAgentFlags::ALLOW_INTERACTION | SecretAgentFlags::REQUEST_NEW)
    {
        request
            .responder
            .no_secrets()
            .await
            .map_err(|err| Error(err.to_string()))?;
        return Ok(());
    }

    let (tx, rx) = oneshot::channel();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));
    let description = match &request.setting {
        SecretSetting::WifiPsk { ssid } => Some(ssid.clone()),
        SecretSetting::WifiEap { identity, method } => Some(
            format!(
                "{} {}",
                identity.clone().unwrap_or_default(),
                method.clone().unwrap_or_default()
            )
            .trim()
            .to_string(),
        ),
        SecretSetting::Vpn {
            service_type,
            user_name,
        } => Some(user_name.clone().unwrap_or_else(|| service_type.clone())),
        SecretSetting::Other(setting) => Some(setting.clone()),
        _ => None,
    }
    .filter(|value| !value.is_empty());

    let event_name = match &request.setting {
        SecretSetting::WifiPsk { .. } | SecretSetting::WifiEap { .. } => {
            request.connection_uuid.clone()
        }
        _ => request.connection_id.clone(),
    };

    let response_kind = ResponseKind::from(&request);
    let uuid = request.connection_uuid.clone();
    msg_tx
        .send(Event::RequestSecret {
            uuid: uuid.clone(),
            name: event_name,
            description,
            previous: SecureString::from(""),
            tx,
        })
        .await
        .map_err(|err| Error(err.to_string()))?;

    match rx.await {
        Ok(secret) => {
            let mut stored = HashMap::new();
            let key = response_kind.key();
            stored.insert(key.to_string(), secret.clone());
            store_secrets(&uuid, setting_name, &stored).await?;
            response_kind.respond(request.responder, secret).await?;
        }
        Err(_) => {
            request
                .responder
                .cancel()
                .await
                .map_err(|err| Error(err.to_string()))?;
        }
    }

    Ok(())
}

enum ResponseKind {
    WifiPsk,
    WifiEap { identity: Option<String> },
    Vpn { key: String },
    Raw { setting_name: String, key: String },
}

impl ResponseKind {
    fn from(request: &SecretRequest) -> Self {
        match &request.setting {
            SecretSetting::WifiPsk { .. } => Self::WifiPsk,
            SecretSetting::WifiEap { identity, .. } => Self::WifiEap {
                identity: identity.clone(),
            },
            SecretSetting::Vpn { .. } => Self::Vpn {
                key: request
                    .hints
                    .iter()
                    .find(|hint| !hint.contains(':'))
                    .cloned()
                    .unwrap_or_else(|| "password".to_string()),
            },
            SecretSetting::Other(setting_name) => Self::Raw {
                setting_name: setting_name.clone(),
                key: request
                    .hints
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "password".to_string()),
            },
            _ => Self::Raw {
                setting_name: setting_name(&request.setting).to_string(),
                key: "password".to_string(),
            },
        }
    }

    fn key(&self) -> &str {
        match self {
            Self::WifiPsk => "psk",
            Self::WifiEap { .. } => "password",
            Self::Vpn { key } | Self::Raw { key, .. } => key.as_str(),
        }
    }

    async fn respond(self, responder: SecretResponder, secret: SecureString) -> Result<(), Error> {
        match self {
            Self::WifiPsk => responder
                .wifi_psk(secret.unsecure().to_string())
                .await
                .map_err(|err| Error(err.to_string())),
            Self::WifiEap { identity } => responder
                .wifi_eap(identity, secret.unsecure().to_string())
                .await
                .map_err(|err| Error(err.to_string())),
            Self::Vpn { key } => responder
                .vpn_secrets(HashMap::from([(key, secret.unsecure().to_string())]))
                .await
                .map_err(|err| Error(err.to_string())),
            Self::Raw { setting_name, key } => {
                let mut inner = HashMap::new();
                inner.insert(
                    key,
                    OwnedValue::from(Str::from(secret.unsecure().to_string())),
                );
                responder
                    .raw(setting_name, inner)
                    .await
                    .map_err(|err| Error(err.to_string()))
            }
        }
    }
}

async fn respond_with_stored(
    request: SecretRequest,
    stored: HashMap<String, SecureString>,
) -> Result<(), Error> {
    match &request.setting {
        SecretSetting::WifiPsk { .. } => {
            if let Some(psk) = stored.get("psk") {
                request
                    .responder
                    .wifi_psk(psk.unsecure().to_string())
                    .await
                    .map_err(|err| Error(err.to_string()))?;
            } else {
                request
                    .responder
                    .no_secrets()
                    .await
                    .map_err(|err| Error(err.to_string()))?;
            }
        }
        SecretSetting::WifiEap { identity, .. } => {
            if let Some(password) = stored.get("password") {
                request
                    .responder
                    .wifi_eap(identity.clone(), password.unsecure().to_string())
                    .await
                    .map_err(|err| Error(err.to_string()))?;
            } else {
                request
                    .responder
                    .no_secrets()
                    .await
                    .map_err(|err| Error(err.to_string()))?;
            }
        }
        SecretSetting::Vpn { .. } => {
            // Earlier versions stored the username next to the password. openvpn
            // fails to reconnect when one is present in the secrets dictionary, so
            // profiles saved by those versions have to be filtered on read too.
            let secrets: HashMap<String, String> = stored
                .into_iter()
                .filter(|(key, _)| key != "username")
                .map(|(key, value)| (key, value.unsecure().to_string()))
                .collect();

            if secrets.is_empty() {
                request
                    .responder
                    .no_secrets()
                    .await
                    .map_err(|err| Error(err.to_string()))?;
            } else {
                request
                    .responder
                    .vpn_secrets(secrets)
                    .await
                    .map_err(|err| Error(err.to_string()))?;
            }
        }
        _ => {
            let setting_name = setting_name(&request.setting).to_string();
            let inner = stored
                .into_iter()
                .map(|(key, value)| {
                    (
                        key,
                        OwnedValue::from(Str::from(value.unsecure().to_string())),
                    )
                })
                .collect();
            request
                .responder
                .raw(setting_name, inner)
                .await
                .map_err(|err| Error(err.to_string()))?;
        }
    }

    Ok(())
}

fn setting_name(setting: &SecretSetting) -> &'static str {
    match setting {
        SecretSetting::WifiPsk { .. } => "802-11-wireless-security",
        SecretSetting::WifiEap { .. } => "802-1x",
        SecretSetting::Vpn { .. } => "vpn",
        SecretSetting::Gsm => "gsm",
        SecretSetting::Cdma => "cdma",
        SecretSetting::Pppoe => "pppoe",
        SecretSetting::Other(_) => "connection",
        _ => "connection",
    }
}

async fn unlock_collection() -> Result<(), Error> {
    let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
        .await
        .map_err(|err| Error(err.to_string()))?;
    let collection = ss
        .get_default_collection()
        .await
        .map_err(|err| Error(err.to_string()))?;
    if collection
        .is_locked()
        .await
        .map_err(|err| Error(err.to_string()))?
    {
        collection
            .unlock()
            .await
            .map_err(|err| Error(err.to_string()))?;
    }
    Ok(())
}

async fn store_secrets(
    uuid: &str,
    setting_name: &str,
    secrets: &HashMap<String, SecureString>,
) -> Result<(), Error> {
    let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
        .await
        .map_err(|err| Error(err.to_string()))?;
    let collection = ss
        .get_default_collection()
        .await
        .map_err(|err| Error(err.to_string()))?;

    for (name, secret) in secrets {
        let mut attributes = HashMap::new();
        attributes.insert("application", SECRET_ID);
        attributes.insert("uuid", uuid);
        attributes.insert("setting_name", setting_name);
        attributes.insert("name", name.as_str());
        collection
            .create_item(
                "NetworkManager Secret",
                attributes,
                secret.unsecure().as_bytes(),
                true,
                "text/plain",
            )
            .await
            .map_err(|err| Error(err.to_string()))?;
    }

    Ok(())
}

async fn get_secrets(
    uuid: &str,
    setting_name: &str,
) -> Result<HashMap<String, SecureString>, Error> {
    let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
        .await
        .map_err(|err| Error(err.to_string()))?;
    let collection = ss
        .get_default_collection()
        .await
        .map_err(|err| Error(err.to_string()))?;
    let mut attributes = HashMap::new();
    attributes.insert("application", SECRET_ID);
    attributes.insert("uuid", uuid);
    attributes.insert("setting_name", setting_name);

    let search_items = collection
        .search_items(attributes)
        .await
        .map_err(|err| Error(err.to_string()))?;
    let mut secrets = HashMap::new();
    for item in &search_items {
        let name = item
            .get_attributes()
            .await
            .map_err(|err| Error(err.to_string()))?
            .get("name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let secret = item
            .get_secret()
            .await
            .map_err(|err| Error(err.to_string()))?;
        let secret = String::from_utf8(secret).map_err(|err| Error(err.to_string()))?;
        secrets.insert(name, SecureString::from(secret));
    }
    Ok(secrets)
}

async fn delete_secrets(uuid: &str) -> Result<(), Error> {
    let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
        .await
        .map_err(|err| Error(err.to_string()))?;
    let collection = ss
        .get_default_collection()
        .await
        .map_err(|err| Error(err.to_string()))?;
    let mut attributes = HashMap::new();
    attributes.insert("application", SECRET_ID);
    attributes.insert("uuid", uuid);

    let search_items = collection
        .search_items(attributes)
        .await
        .map_err(|err| Error(err.to_string()))?;
    for item in &search_items {
        item.delete().await.map_err(|err| Error(err.to_string()))?;
    }
    Ok(())
}
