//! The labelled controls a runner's form is built from.
//!
//! One `Field` describes a control: the name it carries on the wire, the label
//! a reader reads, and the sentence that explains the parameter. Every runner
//! draws its controls from here, so the three of them stay one form.
//!
//! A hint reaches the accessibility tree on every render through
//! `aria-describedby`, and is drawn for a sighted reader only while the form's
//! help switch is on. `sr-only` is what keeps it reachable while it is out of
//! the way (<https://www.w3.org/WAI/WCAG22/Techniques/css/C7>).

use leptos::ev::Event;
use leptos::html::Input;
use leptos::prelude::*;

use crate::styles;

/// One control: what it is called on the wire, what a reader calls it, and the
/// sentence that explains it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Field {
    /// The `id` the label points at.
    pub(crate) id: &'static str,
    /// The `name` the control carries.
    pub(crate) name: &'static str,
    /// The label a reader reads.
    pub(crate) label: &'static str,
    /// The sentence that explains the parameter.
    pub(crate) hint: &'static str,
}

/// Whether a screen is drawing the hints of its controls.
///
/// One switch governs a whole screen: a reader wants every parameter explained
/// or none of them, and a screen with two runners on it explains both at once.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Help(pub(crate) RwSignal<bool>);

/// The switch that draws the hints of the screen it is placed on.
pub(crate) fn help_toggle() -> AnyView {
    let Help(help) = expect_context::<Help>();
    view! {
        <button
            type="button"
            class=styles::BUTTON_QUIET
            aria-pressed=move || if help.get() { "true" } else { "false" }
            on:click=move |_| help.update(|shown| *shown = !*shown)
        >
            {move || if help.get() { "Hide help" } else { "Explain these parameters" }}
        </button>
    }
    .into_any()
}

/// The hint under one control, drawn when the form's help is on and in the
/// accessibility tree either way.
fn hint(id: &str, text: &'static str) -> AnyView {
    let Help(help) = expect_context::<Help>();
    view! {
        <p
            id=id.to_owned()
            class=move || {
                if help.get() {
                    styles::HINT.to_owned()
                } else {
                    format!("sr-only {}", styles::HINT)
                }
            }
        >
            {text}
        </p>
    }
    .into_any()
}

/// One labelled text control, seeded from its own parameter.
pub(crate) fn text_field(field: Field, node: NodeRef<Input>, value: Memo<String>) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-tight">
            <label for=field.id class=styles::LABEL>
                {field.label}
            </label>
            <input
                id=field.id
                name=field.name
                type="text"
                class=styles::INPUT
                aria-describedby=described_by.clone()
                node_ref=node
                prop:value=move || value.get()
            />
            {hint(&described_by, field.hint)}
        </div>
    }
    .into_any()
}

/// One labelled checkbox, seeded from its own parameter.
pub(crate) fn check_field(field: Field, node: NodeRef<Input>, value: Memo<bool>) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-tight">
            <div class="flex items-center gap-default">
                <input
                    id=field.id
                    name=field.name
                    type="checkbox"
                    class="h-4 w-4 accent-accent"
                    aria-describedby=described_by.clone()
                    node_ref=node
                    prop:checked=move || value.get()
                />
                <label for=field.id class=styles::LABEL>
                    {field.label}
                </label>
            </div>
            {hint(&described_by, field.hint)}
        </div>
    }
    .into_any()
}

/// One labelled whole-number control, seeded from its own parameter.
pub(crate) fn number_field(
    field: Field,
    node: NodeRef<Input>,
    value: Memo<String>,
    max: u32,
) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-tight">
            <label for=field.id class=styles::LABEL>
                {field.label}
            </label>
            <input
                id=field.id
                name=field.name
                type="number"
                min="1"
                max=max.to_string()
                class=styles::INPUT
                aria-describedby=described_by.clone()
                node_ref=node
                prop:value=move || value.get()
            />
            {hint(&described_by, field.hint)}
        </div>
    }
    .into_any()
}

/// One labelled choice control, seeded from its own parameter.
///
/// The choice is a navigation on change: it decides which operation the runner
/// sends and which controls belong on the form, so it cannot wait for a submit.
pub(crate) fn select_field(
    field: Field,
    value: Memo<String>,
    options: impl Fn() -> Vec<AnyView> + Send + Sync + 'static,
    change: impl FnMut(Event) + 'static,
) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-tight">
            <label for=field.id class=styles::LABEL>
                {field.label}
            </label>
            <select
                id=field.id
                name=field.name
                class=styles::INPUT
                aria-describedby=described_by.clone()
                prop:value=move || value.get()
                on:change=change
            >
                {options}
            </select>
            {hint(&described_by, field.hint)}
        </div>
    }
    .into_any()
}

/// Controls that sit side by side on a wide screen.
pub(crate) fn row(fields: Vec<AnyView>) -> AnyView {
    view! { <div class="grid gap-default sm:grid-cols-2">{fields}</div> }.into_any()
}

/// A note beside a control about the run that is showing.
///
/// A parameter the address carried and the runner could not use is not help,
/// so it does not hide behind the help switch.
pub(crate) fn note(text: Memo<String>) -> AnyView {
    view! {
        <p role="status" class=styles::HINT>
            {move || text.get()}
        </p>
    }
    .into_any()
}

/// A labelled group of controls that belong together.
pub(crate) fn group(label: &'static str, fields: Vec<AnyView>) -> AnyView {
    view! {
        <fieldset class="grid gap-default">
            <legend class=format!("{} mb-tight", styles::EYEBROW)>{label}</legend>
            {fields}
        </fieldset>
    }
    .into_any()
}
