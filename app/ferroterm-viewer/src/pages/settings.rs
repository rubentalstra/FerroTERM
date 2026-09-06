//! The settings screen: what this browser remembers about this reader.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::version::FhirVersion;
use crate::paging::MAX_COUNT;
use crate::settings::Settings;
use crate::settings::parse_page_size;
use crate::theme::ThemeMode;

/// The classes every control on this screen shares.
const CONTROL: &str = "w-56 rounded border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-700 dark:bg-slate-900";

/// Shows and edits the per-viewer preferences.
///
/// Every value here lives in this browser's `localStorage`. The server holds
/// nothing about a reader, so nothing on this screen is sent anywhere.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn SettingsPage() -> impl IntoView {
    let settings = expect_context::<Settings>();
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let base = move || client.version_base(version.get());

    let heading = view! {
        <Title text="Settings" />
        <h1 class="text-2xl font-semibold">"Settings"</h1>
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "These preferences are stored in this browser only. The server is neither asked nor told about them."
        </p>
    }
    .into_any();

    let in_use = view! {
        <dl class="mt-6 grid gap-2 text-sm sm:grid-cols-[12rem_1fr]">
            <dt class="font-medium">"FHIR base in use"</dt>
            <dd class="font-mono break-all">{base}</dd>
        </dl>
    }
    .into_any();

    view! {
        {heading}
        {in_use}
        <form class="mt-6 grid gap-5" on:submit=|ev| ev.prevent_default()>
            {theme_field(settings)}
            {version_field(settings)}
            {language_field(settings)}
            {page_size_field(settings)}
        </form>
    }
}

/// The light and dark choice.
fn theme_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-1">
            <label for="viewer-theme" class="text-sm font-medium">
                "Theme"
            </label>
            <select
                id="viewer-theme"
                name="theme"
                class=CONTROL
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

/// The FHIR version an address without one falls back to.
fn version_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-1">
            <label for="viewer-fhir-version" class="text-sm font-medium">
                "Default FHIR version"
            </label>
            <select
                id="viewer-fhir-version"
                name="fhir-version"
                class=CONTROL
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
            <p class="text-xs text-slate-500 dark:text-slate-400">
                "Used when an address carries no version of its own."
            </p>
        </div>
    }
    .into_any()
}

/// The BCP 47 tag sent as `displayLanguage`.
fn language_field(settings: Settings) -> AnyView {
    view! {
        <div class="grid gap-1">
            <label for="viewer-display-language" class="text-sm font-medium">
                "Display language"
            </label>
            <input
                id="viewer-display-language"
                name="display-language"
                type="text"
                placeholder="for example nl-NL"
                class=CONTROL
                prop:value=move || settings.language.get()
                on:input:target=move |ev| settings.language.set(ev.target().value())
            />
            <p class="text-xs text-slate-500 dark:text-slate-400">
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
        <div class="grid gap-1">
            <label for="viewer-page-size" class="text-sm font-medium">
                "Page size"
            </label>
            <input
                id="viewer-page-size"
                name="page-size"
                type="number"
                min="1"
                max=MAX_COUNT.to_string()
                aria-describedby="viewer-page-size-note"
                class=CONTROL
                prop:value=move || draft.get()
                on:input:target=move |ev| {
                    let typed = ev.target().value();
                    if let Some(size) = parse_page_size(&typed) {
                        settings.page_size.set(size);
                    }
                    draft.set(typed);
                }
            />
            <p id="viewer-page-size-note" class="text-xs text-slate-500 dark:text-slate-400">
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
