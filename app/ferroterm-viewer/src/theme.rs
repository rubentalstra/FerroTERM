//! The light and dark themes, and how a choice is remembered.

/// The two themes the viewer ships.
///
/// A reader who has never chosen starts on the theme their operating system
/// asks for, and the choice is remembered from then on.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ThemeMode {
    /// Dark text on a light surface.
    #[default]
    Light,
    /// Light text on a dark surface.
    Dark,
}

impl ThemeMode {
    /// The stored form of the choice.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// The name of the theme, for a control that names what it switches to.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// The other theme, which is what a toggle switches to.
    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    /// Reads a stored choice, or `None` when the text names no theme.
    pub(crate) fn from_key(text: &str) -> Option<Self> {
        match text {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    /// The theme the operating system asks for, through `prefers-color-scheme`.
    ///
    /// A browser that cannot answer the media query is treated as asking for
    /// the light theme, which is this type's default.
    pub(crate) fn preferred() -> Self {
        let dark = web_sys::window()
            .and_then(|window| window.match_media("(prefers-color-scheme: dark)").ok())
            .flatten()
            .is_some_and(|query| query.matches());
        if dark { Self::Dark } else { Self::Light }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_choice_round_trips() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            assert_eq!(
                ThemeMode::from_key(mode.key()),
                Some(mode),
                "{} must survive a page reload",
                mode.label()
            );
        }
    }

    #[test]
    fn an_unknown_stored_value_names_no_theme() {
        assert_eq!(
            ThemeMode::from_key("solarized"),
            None,
            "a stored value the viewer does not ship falls back to the preference"
        );
    }

    #[test]
    fn toggling_twice_returns_to_the_starting_theme() {
        assert_eq!(ThemeMode::Light.toggled().toggled(), ThemeMode::Light);
        assert_eq!(ThemeMode::Dark.toggled(), ThemeMode::Light);
    }
}
