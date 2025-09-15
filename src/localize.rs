// SPDX-License-Identifier: GPL-3.0-only

use i18n_embed::{
    DefaultLocalizer, LanguageLoader, LanguageRequester, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
    unic_langid::LanguageIdentifier,
};
use rust_embed::RustEmbed;
use std::str::FromStr;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error while loading fallback language");

    loader
});

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::localize::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::localize::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

// Get the `Localizer` to be used for localizing this library.
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

pub fn localize() {
    let localizer = localizer();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    if let Err(error) = localizer.select(&requested_languages) {
        eprintln!("Error while loading language for App List {}", error);
    }
}

pub fn set_locale(locale: &str) {
    unsafe {
        std::env::set_var("LANG", locale);
    }

    if let Ok(locale) = LanguageIdentifier::from_str(locale) {
        let localizer = localizer();
        let mut lang_requester = i18n_embed::DesktopLanguageRequester::new();
        _ = lang_requester.set_language_override(Some(locale));

        if let Err(error) = localizer.select(&lang_requester.requested_languages()) {
            eprintln!("Error while loading language for App List {}", error);
        }
    }
}
