//! The settings screen: what this browser remembers about this reader.

use leptos::prelude::*;

use crate::components::shell::SelectedVersion;
use crate::density::Density;
use crate::fhir::FhirClient;
use crate::fhir::version::FhirVersion;
use crate::paging::MAX_COUNT;
use crate::settings::Settings;
use crate::settings::parse_page_size;
use crate::styles;
use crate::theme::ThemeMode;

/// Shows and edits the per-viewer preferences.
///
/// Every value here lives in this browser's `localStorage`. The server holds
/// nothing about a reader, so nothing on this screen is sent anywhere.
pub(crate) fn pane() -> AnyView {
    let settings = expect_context::<Settings>();
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let base = move || client.version_base(version.get());

    let in_use = view! {
        <dl class="mt-loose grid gap-default text-body sm:grid-cols-[12rem_1fr]">
            <dt class="font-medium">"FHIR base in use"</dt>
            <dd class="font-mono break-all">{base}</dd>
        </dl>
    }
    .into_any();

    view! {
        <section class="mt-section" aria-labelledby="about-settings-heading">
            <h2 id="about-settings-heading" class=styles::SECTION_TITLE>
                "Settings"
            </h2>
            <p class=styles::LEAD>
                "These preferences are stored in this browser only. The server is neither asked nor told about them."
            </p>
            {in_use}
            <form class="mt-loose grid gap-loose" on:submit=|ev| ev.prevent_default()>
                {theme_field(settings)}
                {density_field(settings)}
                {version_field(settings)}
                {language_field(settings)}
                {page_size_field(settings)}
            </form>
        </section>
    }
    .into_any()
}

/// The light and dark choice.
fn theme_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-tight">
            <label for="viewer-theme" class=styles::LABEL>
                "Theme"
            </label>
            <select
                id="viewer-theme"
                name="theme"
                class=styles::INPUT
                prop:value=move || settings.theme.get().key()
                on:change:target=move |ev| {
                    if let Some(mode) = ThemeMode::from_key(&ev.target().value()) {
                        settings.theme.set(mode);
                    }
                }
            >
                <option value="light">"Light"</option>
                <option value="dark">"Dark"</option>
            </select>
        </div>
    }
    .into_any()
}

/// How much room a row and a panel take.
fn density_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-tight">
            <label for="viewer-density" class=styles::LABEL>
                "Density"
            </label>
            <select
                id="viewer-density"
                name="density"
                aria-describedby="viewer-density-note"
                class=styles::INPUT
                prop:value=move || settings.density.get().key()
                on:change:target=move |ev| {
                    if let Some(chosen) = Density::from_key(&ev.target().value()) {
                        settings.density.set(chosen);
                    }
                }
            >
                <option value="comfortable">{Density::Comfortable.label()}</option>
                <option value="compact">{Density::Compact.label()}</option>
            </select>
            <p id="viewer-density-note" class=styles::HINT>
                "Compact tightens every table row and panel, so more of an expansion or a searchset fits one screen."
            </p>
        </div>
    }
    .into_any()
}

/// The FHIR version an address without one falls back to.
fn version_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-tight">
            <label for="viewer-fhir-version" class=styles::LABEL>
                "Default FHIR version"
            </label>
            <select
                id="viewer-fhir-version"
                name="fhir-version"
                class=styles::INPUT
                prop:value=move || settings.version.get().segment()
                on:change:target=move |ev| {
                    if let Ok(selected) = ev.target().value().parse::<FhirVersion>() {
                        settings.version.set(selected);
                    }
                }
            >
                <For each=move || FhirVersion::ALL key=|version| *version let:option>
                    <option value=option.segment()>{option.label()}</option>
                </For>
            </select>
            <p class=styles::HINT>"Used when an address carries no version of its own."</p>
        </div>
    }
    .into_any()
}

/// The BCP 47 tag sent as `displayLanguage`.
fn language_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-tight">
            <label for="viewer-display-language" class=styles::LABEL>
                "Display language"
            </label>
            <input
                id="viewer-display-language"
                name="display-language"
                type="text"
                placeholder="for example nl-NL"
                class=styles::INPUT
                prop:value=move || settings.language.get()
                on:input:target=move |ev| settings.language.set(ev.target().value())
            />
            <p class=styles::HINT>
                "A BCP 47 tag sent as displayLanguage. Leave it empty to take the server default."
            </p>
        </div>
    }
    .into_any()
}

/// How many rows a paged screen asks for.
///
/// The typed text is kept as it was typed and the stored size only moves when
/// the text names a usable one, so a reader mid-edit is never corrected under
/// their hands and is told plainly when a value was not taken.
fn page_size_field(settings: Settings) -> AnyView {
    let draft = RwSignal::new(settings.page_size.get_untracked().to_string());
    let refused = move || draft.with(|text| parse_page_size(text).is_none());
    view! {
        <div class="grid gap-tight">
            <label for="viewer-page-size" class=styles::LABEL>
                "Page size"
            </label>
            <input
                id="viewer-page-size"
                name="page-size"
                type="number"
                min="1"
                max=MAX_COUNT.to_string()
                aria-describedby="viewer-page-size-note"
                class=styles::INPUT
                prop:value=move || draft.get()
                on:input:target=move |ev| {
                    let typed = ev.target().value();
                    if let Some(size) = parse_page_size(&typed) {
                        settings.page_size.set(size);
                    }
                    draft.set(typed);
                }
            />
            <p id="viewer-page-size-note" class=styles::HINT>
                {move || {
                    if refused() {
                        format!(
                            "Not stored: a page size is a whole number from 1 to {MAX_COUNT}. {} is still in use.",
                            settings.page_size.get(),
                        )
                    } else {
                        format!("How many rows a paged screen asks for, at most {MAX_COUNT}.")
                    }
                }}
            </p>
        </div>
    }
    .into_any()
}
