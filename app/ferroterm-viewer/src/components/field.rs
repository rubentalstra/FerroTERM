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

use crate::fhir::named::Choice;
use crate::styles;

/// The option value that asks for the text field rather than a listed one.
///
/// It is not a canonical, so no resource can collide with it.
const TYPE_IT: &str = "\u{1}another";

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

/// A control and the narrower one beside it, for a value and its version.
///
/// A canonical is a name a reader reads and a version is a number, so the two
/// do not deserve the same width: an even split truncates the name that the
/// picker exists to show.
pub(crate) fn versioned_row(fields: Vec<AnyView>) -> AnyView {
    view! { <div class="grid gap-default sm:grid-cols-[2fr_1fr]">{fields}</div> }.into_any()
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

/// What one picker's own two options say.
///
/// The words are the control's, because "the version this server defaults to"
/// and "another canonical" are the same option in two different forms.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Picker {
    /// What the option that names nothing says.
    pub(crate) empty: &'static str,
    /// What the option that asks for the text field says.
    pub(crate) other: &'static str,
}

/// The words a picker over a canonical carries.
pub(crate) const CANONICAL_WORDS: Picker = Picker {
    empty: "Choose one this server holds",
    other: "Another canonical",
};

/// The words a picker over a version carries.
///
/// A request that names no version is answered against the version the server
/// resolves to, which is what the empty option says rather than leaving a
/// reader to guess what an empty box means
/// (<https://hl7.org/fhir/R5/terminology-module.html#version>).
pub(crate) const VERSION_WORDS: Picker = Picker {
    empty: "The one this server resolves to",
    other: "Another version",
};

/// One labelled value, picked from what this root holds or typed.
///
/// A canonical is what every operation takes and what almost no reader knows
/// by heart, so the control offers what this root holds rather than waiting
/// for one to be typed. The text field stays, as the last option and as what
/// the form submits: a root may hold a resource it does not publish, and a
/// reader may be checking a canonical against a root on purpose.
///
/// The select is a second control over one value, so it carries no `name` and
/// nothing on the wire: picking writes the canonical into the field beside it
/// (<https://developer.mozilla.org/en-US/docs/Web/API/HTMLInputElement/value>).
pub(crate) fn picked_field(
    field: Field,
    node: NodeRef<Input>,
    value: Memo<String>,
    choices: Memo<Vec<Choice>>,
    words: Picker,
) -> AnyView {
    // Whether the reader asked for the text field. It is a signal of this
    // control rather than of the screen, because one form can carry two
    // pickers and each is on its own choice.
    let typing = RwSignal::new(false);
    let described_by = format!("{}-note", field.id);
    let picker = StoredValue::new(format!("{}-pick", field.id));
    let offered = move || {
        choices.with(|choices| {
            choices
                .iter()
                .map(|choice| {
                    let canonical = choice.canonical.clone();
                    let label = choice.label.clone();
                    view! {
                        <option value=canonical.clone() title=canonical>
                            {label}
                        </option>
                    }
                    .into_any()
                })
                .collect::<Vec<AnyView>>()
        })
    };
    // A canonical the root does not publish is a reader's own, so the control
    // opens on the text field and stays there rather than snapping back to a
    // list the value is not in.
    let unlisted = move || {
        value.with(|value| {
            !value.is_empty()
                && choices.with(|choices| !choices.iter().any(|choice| &choice.canonical == value))
        })
    };
    let typed = move || typing.get() || unlisted();
    // A select drops a value it has no option for, and the address is read
    // before the search that fills the list answers, so the offers are read
    // here: reading them is what writes the value again when they arrive.
    let selected = move || {
        choices.with(Vec::len);
        value.get()
    };
    let choose = move |event: Event| {
        let canonical = event_target_value(&event);
        if canonical == TYPE_IT {
            typing.set(true);
            return;
        }
        typing.set(false);
        if let Some(input) = node.get() {
            input.set_value(&canonical);
        }
    };
    view! {
        <div class="grid gap-tight">
            // Each control carries its own label, because a label points at
            // one control and only one of the two is ever showing. The label
            // of a hidden control is hidden with it, so a reader meets one
            // (<https://www.w3.org/TR/wai-aria-1.2/#namecalculation>).
            <label for=move || picker.with_value(Clone::clone) hidden=typed class=styles::LABEL>
                {field.label}
            </label>
            <label for=field.id hidden=move || !typed() class=styles::LABEL>
                {field.label}
            </label>
            <select
                id=move || picker.with_value(Clone::clone)
                class=styles::INPUT
                aria-describedby=described_by.clone()
                hidden=typed
                prop:value=selected
                on:change=choose
            >
                <option value="">{words.empty}</option>
                {offered}
                <option value=TYPE_IT>{words.other}</option>
            </select>
            <input
                id=field.id
                name=field.name
                type="text"
                class=styles::INPUT
                aria-describedby=described_by.clone()
                hidden=move || !typed()
                node_ref=node
                prop:value=move || value.get()
            />
            {hint(&described_by, field.hint)}
        </div>
    }
    .into_any()
}
