// SPDX-License-Identifier: MPL-2.0

mod app;
mod config;
mod crypto;
mod i18n;
mod social;

fn main() -> cosmic::iced::Result {
    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Settings for a compact, clean application
    let settings = cosmic::app::Settings::default()
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(600.0)
                .min_height(650.0)
                .max_width(600.0)
                .max_height(650.0),
        );

    // Run as a COSMIC application
    cosmic::app::run::<app::AppModel>(settings, ())
}
