//! The light and dark switch.

use leptos::prelude::*;
use thaw::Button;
use thaw::ButtonAppearance;
use thaw::ButtonSize;

use crate::settings::Settings;

/// Switches between the light and the dark theme, and remembers the choice.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn ThemeToggle() -> impl IntoView {
    let settings = expect_context::<Settings>();
    let next = move || settings.theme.get().toggled();
    view! {
        <Button
            appearance=ButtonAppearance::Subtle
            size=ButtonSize::Small
            on_click=move |_| settings.theme.set(next())
            attr:aria-label=move || format!("Switch to the {} theme", next().label().to_lowercase())
        >
            {move || next().label()}
        </Button>
    }
}
