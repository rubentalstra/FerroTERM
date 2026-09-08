//! How much room a row and a panel take, and how a choice is remembered.

/// The two densities the viewer ships.
///
/// A terminology server holds tables: the served versions, an expansion, a
/// searchset, a concept's designations. A reader comparing twenty rows wants
/// more of them on the screen than a reader reading one does, so the choice is
/// theirs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Density {
    /// The default: a row and a panel with room around their content.
    #[default]
    Comfortable,
    /// Tighter rows and panels, for comparing many at once.
    Compact,
}

impl Density {
    /// The stored form of the choice, which is also what the document carries.
    ///
    /// `style/tailwind.css` selects the compact variables on
    /// `:root[data-density="compact"]`, so the stored word and the attribute
    /// value are one string.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }

    /// The name a reader reads on the control that chooses it.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "Comfortable",
            Self::Compact => "Compact",
        }
    }

    /// Reads a stored choice, or `None` when the text names no density.
    pub(crate) fn from_key(text: &str) -> Option<Self> {
        match text {
            "comfortable" => Some(Self::Comfortable),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_choice_round_trips() {
        for density in [Density::Comfortable, Density::Compact] {
            assert_eq!(
                Density::from_key(density.key()),
                Some(density),
                "{} must survive a page reload",
                density.label()
            );
        }
    }

    #[test]
    fn an_unknown_stored_value_names_no_density() {
        assert_eq!(
            Density::from_key("cosy"),
            None,
            "a stored value the viewer does not ship falls back to the default"
        );
    }

    #[test]
    fn the_compact_key_is_the_one_the_stylesheet_selects_on() {
        assert_eq!(
            Density::Compact.key(),
            "compact",
            "the attribute value and the stored word are one string"
        );
    }
}
