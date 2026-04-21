use cosmic::{
    Element, Task,
    cosmic_config::{self, ConfigSet},
    cosmic_theme,
    iced::Alignment,
    theme, widget,
};
use eyre::Context;
use slotmap::{DefaultKey, Key, SlotMap};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::{fl, page};

#[derive(Clone, Debug)]
pub enum Message {
    Refresh(Arc<eyre::Result<PageRefresh>>),
    Search(String),
    Select(DefaultKey),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::Language(message).into()
    }
}

#[derive(Debug)]
pub struct PageRefresh {
    config: Option<cosmic_config::Config>,
    registry: Registry,
    language: Option<SystemLocale>,
    region: Option<SystemLocale>,
    available_languages: SlotMap<DefaultKey, SystemLocale>,
    system_locales: BTreeMap<String, SystemLocale>,
    selected: DefaultKey,
}

pub struct Page {
    selected: DefaultKey,
    search_id: widget::Id,
    search: String,
    regex_opt: Option<regex::Regex>,
    active_context: Vec<DefaultKey>,
    config: Option<cosmic_config::Config>,
    registry: Option<locales_rs::Registry>,
    language: Option<SystemLocale>,
    region: Option<SystemLocale>,
    available_languages: SlotMap<DefaultKey, SystemLocale>,
    system_locales: BTreeMap<String, SystemLocale>,
}

impl Page {
    pub fn new() -> Self {
        Self {
            selected: DefaultKey::null(),
            search_id: widget::Id::unique(),
            search: String::new(),
            regex_opt: None,
            active_context: Vec::new(),
            config: None,
            registry: None,
            language: None,
            region: None,
            available_languages: SlotMap::default(),
            system_locales: BTreeMap::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<page::Message> {
        match message {
            Message::Search(search) => {
                self.search = search;
                self.regex_opt = None;
                if !self.search.is_empty() {
                    let pattern = regex::escape(&self.search);
                    match regex::RegexBuilder::new(&pattern)
                        .case_insensitive(true)
                        .build()
                    {
                        Ok(regex) => self.regex_opt = Some(regex),
                        Err(err) => {
                            tracing::warn!(
                                err = err.to_string(),
                                "failed to parse regex {:?}",
                                pattern
                            );
                        }
                    };
                }

                self.update_context();
            }

            Message::Select(selected) => {
                if let Some(locale) = self.available_languages.get(selected) {
                    let lang = locale.lang_code.clone();
                    tokio::spawn(async move {
                        if let Err(why) = set_locale(lang.clone(), lang).await {
                            tracing::error!(?why, "failed to set locale via D-Bus");
                        }
                    });

                    if let Some(config) = self.config.as_mut() {
                        _ = config.set("system_locales", vec![locale.lang_code.clone()]);
                    }

                    crate::localize::set_locale(&locale.lang_code);
                }

                self.selected = selected;
            }

            Message::Refresh(result) => {
                match Arc::into_inner(result).unwrap() {
                    Ok(page_refresh) => {
                        self.config = page_refresh.config;
                        self.available_languages = page_refresh.available_languages;
                        self.system_locales = page_refresh.system_locales;
                        self.language = page_refresh.language;
                        self.region = page_refresh.region;
                        self.registry = Some(page_refresh.registry.0);
                        self.selected = page_refresh.selected;
                    }

                    Err(why) => {
                        tracing::error!(?why, "failed to get locales from the system");
                    }
                }

                self.update_context();
            }
        }
        Task::none()
    }

    fn update_context(&mut self) {
        self.active_context = self
            .available_languages
            .iter()
            .filter_map(|(id, locale)| {
                let Some(regex) = &self.regex_opt else {
                    return Some(id);
                };

                //TODO: better search method (fuzzy search?)
                if regex.is_match(&locale.display_name) {
                    return Some(id);
                }

                None
            })
            .take(100)
            .collect::<Vec<DefaultKey>>();
    }
}

impl super::Page for Page {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn title(&self) -> String {
        fl!("select-language-page")
    }

    fn init(&mut self) -> cosmic::Task<page::Message> {
        let refresh = async || -> eyre::Result<PageRefresh> {
            let conn = zbus::Connection::system()
                .await
                .wrap_err("zbus system connection error")?;

            let registry = locales_rs::Registry::new().wrap_err("failed to get locale registry")?;

            let system_locales: BTreeMap<String, SystemLocale> = locale1::locale1Proxy::new(&conn)
                .await
                .wrap_err("locale1 proxy connect error")?
                .locale()
                .await
                .wrap_err("could not get locale from locale1")?
                .into_iter()
                .filter_map(|expression| {
                    let mut fields = expression.split('=');
                    let var = fields.next()?;
                    let lang_code = fields.next()?;
                    let locale = registry.locale(lang_code)?;

                    Some((
                        var.to_owned(),
                        localized_locale(&locale, lang_code.to_owned()),
                    ))
                })
                .collect();

            let config = cosmic::cosmic_config::Config::new("com.system76.CosmicSettings", 1).ok();

            let language = system_locales
                .get("LC_ALL")
                .or_else(|| system_locales.get("LANG"))
                .cloned();

            let region = system_locales
                .get("LC_TIME")
                .or_else(|| system_locales.get("LANG"))
                .cloned();

            let mut available_languages_set = BTreeSet::new();

            // Use 'locale -a' instead of 'localectl list-locales' for OpenRC compatibility
            let output_result = tokio::process::Command::new("locale")
                .arg("-a")
                .output()
                .await;

            let locale_list = match output_result {
                Ok(output) => {
                    let output_str = String::from_utf8(output.stdout).unwrap_or_default();
                    parse_locale_output(&output_str)
                }
                Err(why) => {
                    tracing::error!(?why, "failed to list available locales using 'locale -a'");
                    Vec::new()
                }
            };

            let mut available_languages = SlotMap::new();
            let mut selected = DefaultKey::null();

            let current_lang = std::env::var("LANG").ok();
            if let Some(lang) = current_lang.as_ref() {
                if let Some(locale) = registry.locale(&lang) {
                    selected = available_languages.insert(localized_locale(&locale, lang.clone()));
                }
            }

            for line in locale_list {
                if Some(line.as_str()) == current_lang.as_deref() {
                    continue;
                }

                if let Some(locale) = registry.locale(&line) {
                    available_languages_set.insert(localized_locale(&locale, line));
                }
            }

            for language in available_languages_set {
                available_languages.insert(language);
            }

            Ok(PageRefresh {
                config,
                registry: Registry(registry),
                language,
                region,
                available_languages,
                system_locales,
                selected,
            })
        };

        cosmic::task::future(async move { Message::Refresh(Arc::new(refresh().await)) })
    }

    fn open(&mut self) -> cosmic::Task<page::Message> {
        return widget::text_input::focus(self.search_id.clone());
    }

    fn completed(&self) -> bool {
        !self.selected.is_null()
    }

    fn view(&self) -> Element<'_, page::Message> {
        let cosmic_theme::Spacing {
            space_xxs, space_m, ..
        } = theme::active().cosmic().spacing;

        let mut section = widget::settings::section();

        let mut first_opt = None;
        for (id, locale) in self.active_context.iter().filter_map(|id| {
            self.available_languages
                .get(*id)
                .map(|locale| (*id, locale))
        }) {
            let item = widget::settings::item::builder(&locale.display_name);

            let selected = id == self.selected;
            section = section.add(
                //TODO: properly style this
                widget::button::custom(
                    item.control(
                        widget::row::with_children(vec![if selected {
                            widget::icon::from_name("object-select-symbolic")
                                .size(16)
                                .into()
                        } else {
                            widget::Space::with_width(16).into()
                        }])
                        .align_y(Alignment::Center)
                        .spacing(space_xxs),
                    ),
                )
                .on_press(Message::Select(id.clone()))
                .class(if selected {
                    theme::Button::Link
                } else {
                    theme::Button::MenuRoot
                }),
            );
            if first_opt.is_none() {
                first_opt = Some(id);
            }
        }

        let mut search_input = widget::search_input(fl!("type-to-search"), &self.search)
            .id(self.search_id.clone())
            .on_input(Message::Search);

        if let Some(first) = first_opt {
            if self.regex_opt.is_some() {
                // Select first item if no item is selected and there is a search
                search_input = search_input.on_submit(move |_| Message::Select(first.clone()));
            }
        }

        let element: Element<_> = widget::column::with_children(vec![
            search_input.into(),
            widget::Space::with_height(space_m).into(),
            widget::scrollable(section).into(),
        ])
        .into();
        element.map(page::Message::Language)
    }
}

/// Sets the system locale using D-Bus instead of localectl for OpenRC compatibility.
pub async fn set_locale(lang: String, region: String) -> eyre::Result<()> {
    tracing::debug!("setting locale lang={lang}, region={region}");

    let conn = zbus::Connection::system()
        .await
        .wrap_err("failed to connect to system D-Bus")?;

    let proxy = locale1::locale1Proxy::new(&conn)
        .await
        .wrap_err("failed to create locale1 D-Bus proxy")?;

    let locale_settings = build_locale_settings(&lang, &region);
    let locale_strs: Vec<&str> = locale_settings.iter().map(|s| s.as_str()).collect();

    proxy
        .set_locale(&locale_strs, true)
        .await
        .wrap_err("failed to set locale via D-Bus")?;

    tracing::debug!("successfully set locale via D-Bus");
    Ok(())
}

fn localized_iso_codes(locale: &locales_rs::Locale) -> (String, String) {
    let mut language = gettextrs::dgettext("iso_639", &locale.language.display_name);
    let country = gettextrs::dgettext("iso_3166", &locale.territory.display_name);

    // Ensure language is title-cased.
    let mut chars = language.chars();
    if let Some(c) = chars.next() {
        language = c.to_uppercase().collect::<String>() + chars.as_str();
    }

    (language, country)
}

fn localized_locale(locale: &locales_rs::Locale, lang_code: String) -> SystemLocale {
    let (language, country) = localized_iso_codes(locale);

    SystemLocale {
        lang_code,
        display_name: format!("{language} ({country})"),
    }
}

struct Registry(locales_rs::Registry);

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry").finish()
    }
}

#[derive(Clone, Debug)]
pub struct SystemLocale {
    lang_code: String,
    display_name: String,
}

impl Eq for SystemLocale {}

impl Ord for SystemLocale {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.display_name.cmp(&other.display_name)
    }
}

impl PartialOrd for SystemLocale {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.display_name.partial_cmp(&other.display_name)
    }
}

impl PartialEq for SystemLocale {
    fn eq(&self, other: &Self) -> bool {
        self.display_name == other.display_name
    }
}

/// Parses the output from `locale -a` command and returns a vector of locale strings.
/// Filters out C and POSIX pseudo-locales and accepts only UTF-8 encoded locales.
fn parse_locale_output(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| {
            // Filter out C and POSIX variants (C, C.utf8, POSIX, POSIX.iso88591, etc.)
            let is_pseudo = *line == "C" || *line == "POSIX" 
                || line.starts_with("C.") || line.starts_with("POSIX.");
            if is_pseudo {
                return false;
            }
            
            // Accept only UTF-8 encoded locales (case insensitive)
            let line_lower = line.to_lowercase();
            line_lower.contains(".utf8") || line_lower.contains(".utf-8")
        })
        .map(|line| line.to_string())
        .collect()
}

/// Builds the locale settings array for D-Bus SetLocale call.
/// Sets LANG to the language parameter and all LC_* variables to the region parameter.
fn build_locale_settings(lang: &str, region: &str) -> Vec<String> {
    vec![
        format!("LANG={}", lang),
        format!("LC_ADDRESS={}", region),
        format!("LC_IDENTIFICATION={}", region),
        format!("LC_MEASUREMENT={}", region),
        format!("LC_MONETARY={}", region),
        format!("LC_NAME={}", region),
        format!("LC_NUMERIC={}", region),
        format!("LC_PAPER={}", region),
        format!("LC_TELEPHONE={}", region),
        format!("LC_TIME={}", region),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_locale_output_filters_c_utf8() {
        let output = "C.UTF-8\nen_US.utf8\nde_DE.utf8\n";
        let result = parse_locale_output(output);
        assert!(!result.contains(&"C.UTF-8".to_string()));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_locale_output_handles_empty_input() {
        let output = "";
        let result = parse_locale_output(output);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_locale_output_preserves_locale_strings() {
        let output = "en_US.utf8\nde_DE.utf8\nfr_FR.utf8\n";
        let result = parse_locale_output(output);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"en_US.utf8".to_string()));
    }

    #[test]
    fn test_build_locale_settings_includes_all_lc_variables() {
        let lang = "en_US.UTF-8";
        let region = "de_DE.UTF-8";
        let settings = build_locale_settings(lang, region);

        assert_eq!(settings.len(), 10);
        assert!(settings.contains(&format!("LANG={}", lang)));
        assert!(settings.contains(&format!("LC_ADDRESS={}", region)));
        assert!(settings.contains(&format!("LC_IDENTIFICATION={}", region)));
        assert!(settings.contains(&format!("LC_MEASUREMENT={}", region)));
        assert!(settings.contains(&format!("LC_MONETARY={}", region)));
        assert!(settings.contains(&format!("LC_NAME={}", region)));
        assert!(settings.contains(&format!("LC_NUMERIC={}", region)));
        assert!(settings.contains(&format!("LC_PAPER={}", region)));
        assert!(settings.contains(&format!("LC_TELEPHONE={}", region)));
        assert!(settings.contains(&format!("LC_TIME={}", region)));
    }

    #[test]
    fn test_build_locale_settings_uses_correct_values() {
        let lang = "fr_FR.UTF-8";
        let region = "en_GB.UTF-8";
        let settings = build_locale_settings(lang, region);

        assert!(settings.iter().any(|s| s == "LANG=fr_FR.UTF-8"));
        assert!(settings.iter().any(|s| s == "LC_TIME=en_GB.UTF-8"));
    }

    #[test]
    fn test_parse_locale_output_filters_any_c_posix_variant() {
        let output = "C\nC.utf8\nC.UTF-8\nPOSIX\nPOSIX.utf8\nC.iso88591\nen_US.utf8\n";
        let result = parse_locale_output(output);
        
        // Should filter out all C and POSIX variants
        assert!(!result.contains(&"C".to_string()));
        assert!(!result.contains(&"C.utf8".to_string()));
        assert!(!result.contains(&"C.UTF-8".to_string()));
        assert!(!result.contains(&"POSIX".to_string()));
        assert!(!result.contains(&"POSIX.utf8".to_string()));
        assert!(!result.contains(&"C.iso88591".to_string()));
        
        // Should keep real locales
        assert!(result.contains(&"en_US.utf8".to_string()));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_locale_output_accepts_only_utf8_locales() {
        let output = "en_US.utf8\nen_US.UTF-8\nde_DE.iso88591\nfr_FR\nes_ES.utf8\n";
        let result = parse_locale_output(output);
        
        // Should accept UTF-8 encoded locales (case insensitive)
        assert!(result.contains(&"en_US.utf8".to_string()));
        assert!(result.contains(&"en_US.UTF-8".to_string()));
        assert!(result.contains(&"es_ES.utf8".to_string()));
        
        // Should reject non-UTF-8 encodings
        assert!(!result.contains(&"de_DE.iso88591".to_string()));
        
        // Should reject locales without explicit encoding
        assert!(!result.contains(&"fr_FR".to_string()));
        
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_locale_output_comprehensive_filtering() {
        // Test comprehensive scenario matching cosmic-settings PR #1961
        let output = concat!(
            "C\n",
            "C.utf8\n",
            "C.UTF-8\n",
            "POSIX\n",
            "POSIX.utf8\n",
            "C.iso88591\n",
            "en_US.utf8\n",
            "en_US.UTF-8\n",
            "de_DE.utf8\n",
            "fr_FR.UTF-8\n",
            "es_ES.iso88591\n",
            "ca_ES.utf8@valencia\n",  // Locale with modifier
            "ar_IN\n",                  // No encoding specified
            "\n",                       // Empty line
        );
        let result = parse_locale_output(output);
        
        // Should filter all C and POSIX variants
        assert!(!result.iter().any(|s| s.starts_with("C")));
        assert!(!result.iter().any(|s| s.starts_with("POSIX")));
        
        // Should accept UTF-8 locales (case insensitive)
        assert!(result.contains(&"en_US.utf8".to_string()));
        assert!(result.contains(&"en_US.UTF-8".to_string()));
        assert!(result.contains(&"de_DE.utf8".to_string()));
        assert!(result.contains(&"fr_FR.UTF-8".to_string()));
        
        // Should accept UTF-8 locales with modifiers
        assert!(result.contains(&"ca_ES.utf8@valencia".to_string()));
        
        // Should reject non-UTF-8 encodings and locales without encoding
        assert!(!result.contains(&"es_ES.iso88591".to_string()));
        assert!(!result.contains(&"ar_IN".to_string()));
        
        // Should handle empty lines
        assert!(!result.contains(&"".to_string()));
        
        assert_eq!(result.len(), 5);
    }
}
