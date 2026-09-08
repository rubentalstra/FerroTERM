//! The runs a reader has made, as the links that re-run them.
//!
//! Every runner draws this the same way and records into it the same way, so
//! a reader who learns it on one screen has learnt it on all three.

use leptos::prelude::*;

use crate::runs::Run;
use crate::runs::Runs;
use crate::runs::remember;
use crate::runs::stored;
use crate::styles;

/// The list a runner holds, and the recorder that keeps it current.
///
/// The signal is what the list draws from, and `localStorage` is what it is
/// read from and written to. A runner creates one of these and hands the
/// `Memo` its own address reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct History(RwSignal<Runs>);

impl History {
    /// A history that starts from what this browser already holds, and records
    /// every run `made` names.
    ///
    /// The effect exists to sync with `localStorage`, which is the outside
    /// world and what an effect is for
    /// (<https://github.com/leptos-rs/book/blob/main/src/reactivity/working_with_signals.md>).
    /// The signal it writes is the list storage now holds, read back so the
    /// screen draws what a reload would.
    pub(crate) fn recording(made: Memo<Option<Run>>) -> Self {
        let held = RwSignal::new(stored());
        Effect::new(move |_| {
            if let Some(run) = made.get() {
                held.set(remember(&run));
            }
        });
        Self(held)
    }

    /// The list, as the section a runner draws.
    pub(crate) fn view(self) -> AnyView {
        let Self(held) = self;
        view! {
            <Show when=move || !held.with(Runs::is_empty) fallback=|| ()>
                {move || {
                    let rows: Vec<AnyView> = held
                        .with(|held| held.entries().iter().map(row).collect());
                    view! {
                        <details class="mt-loose rounded-md border border-line text-small">
                            <summary class=format!(
                                "cursor-pointer px-default py-tight font-medium {}",
                                styles::MUTED,
                            )>"The runs this browser remembers"</summary>
                            <div class="border-t border-line px-default py-tight">
                                <p class=styles::HINT>
                                    "Each one is the address that made it. They live in this browser and are never sent anywhere."
                                </p>
                                <ul class="mt-default grid gap-tight">{rows}</ul>
                            </div>
                        </details>
                    }
                        .into_any()
                }}
            </Show>
        }
        .into_any()
    }
}

/// One remembered run: the link that re-runs it, and what it was about.
fn row(run: &Run) -> AnyView {
    let address = run.address.clone();
    let screen = run.screen.clone();
    let subject = run.subject.clone();
    view! {
        <li>
            <a href=address class="flex flex-wrap items-baseline gap-default">
                <span class=styles::LINK>{screen}</span>
                <span class=styles::CODE_MUTED>{subject}</span>
            </a>
        </li>
    }
    .into_any()
}
