use ratatui::style::Color;
use std::env;

/// Color scheme that respects NO_COLOR environment variable
/// See https://no-color.org/
#[derive(Clone, Copy)]
pub struct ColorScheme {
    enabled: bool,
}

impl ColorScheme {
    /// Create a new color scheme based on NO_COLOR environment variable
    pub fn from_env() -> Self {
        // NO_COLOR disables colors if set to any value (even empty string)
        let enabled = env::var("NO_COLOR").is_err();
        Self { enabled }
    }

    /// Create a new color scheme with explicit enabled state (for testing)
    #[cfg(test)]
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Get color for running status
    pub fn running(&self) -> Color {
        if self.enabled {
            Color::Yellow
        } else {
            Color::Reset
        }
    }

    /// Get color for success status
    pub fn success(&self) -> Color {
        if self.enabled {
            Color::Green
        } else {
            Color::Reset
        }
    }

    /// Get color for failed status
    pub fn failed(&self) -> Color {
        if self.enabled {
            Color::Red
        } else {
            Color::Reset
        }
    }

    /// Get color for pending status
    pub fn pending(&self) -> Color {
        if self.enabled {
            Color::Gray
        } else {
            Color::Reset
        }
    }

    /// Get color for input waiting indicator
    pub fn input_waiting(&self) -> Color {
        if self.enabled {
            Color::Magenta
        } else {
            Color::Reset
        }
    }

    /// Get dim color for input waiting indicator
    pub fn input_waiting_dim(&self) -> Color {
        if self.enabled {
            Color::Rgb(139, 69, 139)
        } else {
            Color::Reset
        }
    }

    /// Get color for diff indicators
    pub fn diff_marker(&self) -> Color {
        if self.enabled {
            Color::Yellow
        } else {
            Color::Reset
        }
    }

    /// Get color for host gutter
    pub fn gutter(&self) -> Color {
        if self.enabled {
            Color::Cyan
        } else {
            Color::Reset
        }
    }

    /// Get color for variant text
    pub fn variant_text(&self) -> Color {
        if self.enabled {
            Color::Gray
        } else {
            Color::Reset
        }
    }

    /// Get color for selection background
    pub fn selection_bg(&self) -> Color {
        if self.enabled {
            Color::DarkGray
        } else {
            Color::Reset
        }
    }

    /// Get color for dark gray elements
    pub fn dark_gray(&self) -> Color {
        if self.enabled {
            Color::DarkGray
        } else {
            Color::Reset
        }
    }

    /// Format a string with ANSI color codes (for text mode)
    /// Returns the string unchanged if colors are disabled
    pub fn ansi_format(&self, text: &str, ansi_code: &str) -> String {
        if self.enabled {
            format!("\x1b[{}m{}\x1b[0m", ansi_code, text)
        } else {
            text.to_string()
        }
    }

    /// ANSI yellow (33)
    pub fn ansi_yellow(&self, text: &str) -> String {
        self.ansi_format(text, "33")
    }

    /// ANSI green (32)
    pub fn ansi_green(&self, text: &str) -> String {
        self.ansi_format(text, "32")
    }

    /// ANSI red (31)
    pub fn ansi_red(&self, text: &str) -> String {
        self.ansi_format(text, "31")
    }

    /// ANSI cyan (36)
    pub fn ansi_cyan(&self, text: &str) -> String {
        self.ansi_format(text, "36")
    }

    /// ANSI gray/dark (90)
    pub fn ansi_gray(&self, text: &str) -> String {
        self.ansi_format(text, "90")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colors_enabled() {
        let scheme = ColorScheme::new(true);
        assert!(matches!(scheme.running(), Color::Yellow));
        assert!(matches!(scheme.success(), Color::Green));
        assert!(matches!(scheme.failed(), Color::Red));
    }

    #[test]
    fn test_ansi_colors_enabled() {
        let scheme = ColorScheme::new(true);
        assert_eq!(scheme.ansi_yellow("test"), "\x1b[33mtest\x1b[0m");
        assert_eq!(scheme.ansi_green("test"), "\x1b[32mtest\x1b[0m");
    }

    #[test]
    fn test_colors_disabled() {
        let scheme = ColorScheme::new(false);
        assert!(matches!(scheme.running(), Color::Reset));
        assert!(matches!(scheme.success(), Color::Reset));
        assert!(matches!(scheme.failed(), Color::Reset));
    }

    #[test]
    fn test_ansi_colors_disabled() {
        let scheme = ColorScheme::new(false);
        assert_eq!(scheme.ansi_yellow("test"), "test");
        assert_eq!(scheme.ansi_green("test"), "test");
    }
}
