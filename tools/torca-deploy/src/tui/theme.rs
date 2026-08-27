use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeKind {
    Aurora,
    Amber,
    HighContrast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub background: Color,
    pub panel: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub selected: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub disabled: Color,
}

impl Theme {
    pub const fn monochrome() -> Self {
        Self {
            background: Color::Reset,
            panel: Color::Reset,
            border: Color::Reset,
            text: Color::Reset,
            muted: Color::Reset,
            accent: Color::Reset,
            selected: Color::Reset,
            success: Color::Reset,
            warning: Color::Reset,
            danger: Color::Reset,
            info: Color::Reset,
            disabled: Color::Reset,
        }
    }

    pub const fn aurora() -> Self {
        Self {
            background: Color::Black,
            panel: Color::Rgb(15, 25, 40),
            border: Color::Blue,
            text: Color::White,
            muted: Color::Gray,
            accent: Color::Cyan,
            selected: Color::Magenta,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Cyan,
            disabled: Color::DarkGray,
        }
    }
    pub const fn amber() -> Self {
        Self {
            background: Color::Black,
            panel: Color::Rgb(35, 25, 10),
            border: Color::Yellow,
            text: Color::Yellow,
            muted: Color::DarkGray,
            accent: Color::LightYellow,
            selected: Color::LightRed,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Yellow,
            disabled: Color::DarkGray,
        }
    }
    pub const fn high_contrast() -> Self {
        Self {
            background: Color::Black,
            panel: Color::Black,
            border: Color::White,
            text: Color::White,
            muted: Color::Gray,
            accent: Color::LightCyan,
            selected: Color::LightMagenta,
            success: Color::LightGreen,
            warning: Color::LightYellow,
            danger: Color::LightRed,
            info: Color::LightCyan,
            disabled: Color::DarkGray,
        }
    }
    pub const fn for_kind(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Aurora => Self::aurora(),
            ThemeKind::Amber => Self::amber(),
            ThemeKind::HighContrast => Self::high_contrast(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_define_status_colours() {
        for kind in [ThemeKind::Aurora, ThemeKind::Amber, ThemeKind::HighContrast] {
            let theme = Theme::for_kind(kind);
            assert_ne!(theme.success, theme.danger);
            assert_ne!(theme.accent, theme.disabled);
        }
    }

    #[test]
    fn monochrome_keeps_symbols_available_without_colour() {
        let theme = Theme::monochrome();
        assert_eq!(theme.accent, Color::Reset);
        assert_eq!(theme.danger, Color::Reset);
    }
}
