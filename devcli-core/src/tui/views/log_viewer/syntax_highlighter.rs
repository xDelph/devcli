use syntect::{
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
};

pub struct SyntaxHighlighter {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
}

impl SyntaxHighlighter {
    /// One instance for the whole process. Loading syntect's bundled syntaxes
    /// and themes is expensive, and was repeated for every log panel opened.
    pub fn shared() -> &'static SyntaxHighlighter {
        static SHARED: std::sync::OnceLock<SyntaxHighlighter> = std::sync::OnceLock::new();
        SHARED.get_or_init(Self::new)
    }

    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();

        Self { syntax_set, theme }
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
