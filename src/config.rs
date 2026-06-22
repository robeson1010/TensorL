use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Language ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Auto,
    // ── Common (shown by default) ─────────────────────────────────────────
    Chinese,
    English,
    // ── Extended (shown after "More languages…") ──────────────────────────
    French,
    Portuguese,
    Spanish,
    Japanese,
    Turkish,
    Russian,
    Arabic,
    Korean,
    Thai,
    Italian,
    German,
    Vietnamese,
    Malay,
    Indonesian,
    Filipino,
    Polish,
    Czech,
    Dutch,
    Ukrainian,
    Kazakh,
    Mongolian,
    Cantonese,
}

impl Language {
    /// Source language list (common only — includes Auto)
    pub fn common_sources() -> &'static [Language] {
        use Language::*;
        &[Auto, Chinese, English]
    }

    /// Target language list (common only — no Auto)
    pub fn common_targets() -> &'static [Language] {
        use Language::*;
        &[Chinese, English]
    }

    /// All source languages (common + extended)
    pub fn all() -> &'static [Language] {
        use Language::*;
        &[
            Auto,
            Chinese, English,
            French, Portuguese, Spanish, Japanese, Turkish, Russian, Arabic,
            Korean, Thai, Italian, German, Vietnamese, Malay, Indonesian,
            Filipino, Polish, Czech, Dutch,
            Ukrainian, Kazakh, Mongolian, Cantonese,
        ]
    }

    /// All target languages (common + extended, no Auto)
    pub fn all_targets() -> &'static [Language] {
        use Language::*;
        &[
            Chinese, English,
            French, Portuguese, Spanish, Japanese, Turkish, Russian, Arabic,
            Korean, Thai, Italian, German, Vietnamese, Malay, Indonesian,
            Filipino, Polish, Czech, Dutch,
            Ukrainian, Kazakh, Mongolian, Cantonese,
        ]
    }

    /// Name shown in the UI combo-box
    pub fn display_name(self) -> &'static str {
        match self {
            Language::Auto              => "Auto Detect",
            Language::Chinese           => "中文 (Chinese)",
            Language::English           => "English",
            Language::French            => "Français (French)",
            Language::Portuguese        => "Português (Portuguese)",
            Language::Spanish           => "Español (Spanish)",
            Language::Japanese          => "日本語 (Japanese)",
            Language::Turkish           => "Türkçe (Turkish)",
            Language::Russian           => "Русский (Russian)",
            Language::Arabic            => "العربية (Arabic)",
            Language::Korean            => "한국어 (Korean)",
            Language::Thai              => "ภาษาไทย (Thai)",
            Language::Italian           => "Italiano (Italian)",
            Language::German            => "Deutsch (German)",
            Language::Vietnamese        => "Tiếng Việt (Vietnamese)",
            Language::Malay             => "Bahasa Melayu (Malay)",
            Language::Indonesian        => "Bahasa Indonesia (Indonesian)",
            Language::Filipino          => "Filipino",
            Language::Polish            => "Polski (Polish)",
            Language::Czech             => "Čeština (Czech)",
            Language::Dutch             => "Nederlands (Dutch)",
            Language::Ukrainian         => "Українська (Ukrainian)",
            Language::Kazakh            => "Қазақша (Kazakh)",
            Language::Mongolian         => "Монгол (Mongolian)",
            Language::Cantonese         => "粤语 (Cantonese)",
        }
    }

    /// Google Translate language code (used in `sl`/`tl` query params).
    /// `Auto` maps to `"auto"` for source detection; as a target it falls
    /// back to English.
    pub fn google_code(self) -> &'static str {
        match self {
            Language::Auto              => "auto",
            Language::Chinese           => "zh-CN",
            Language::English           => "en",
            Language::French            => "fr",
            Language::Portuguese        => "pt",
            Language::Spanish           => "es",
            Language::Japanese          => "ja",
            Language::Turkish           => "tr",
            Language::Russian           => "ru",
            Language::Arabic            => "ar",
            Language::Korean            => "ko",
            Language::Thai              => "th",
            Language::Italian           => "it",
            Language::German            => "de",
            Language::Vietnamese        => "vi",
            Language::Malay             => "ms",
            Language::Indonesian        => "id",
            Language::Filipino          => "tl",
            Language::Polish            => "pl",
            Language::Czech             => "cs",
            Language::Dutch             => "nl",
            Language::Ukrainian         => "uk",
            Language::Kazakh            => "kk",
            Language::Mongolian         => "mn",
            Language::Cantonese         => "yue",
        }
    }

    /// Google Translate target code — `Auto` is not a valid target, so it
    /// falls back to English.
    pub fn google_target_code(self) -> &'static str {
        match self {
            Language::Auto => "en",
            other          => other.google_code(),
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::Auto
    }
}

// ── AppConfig ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub source_language: Language,
    pub target_language: Language,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            source_language: Language::Auto,
            target_language: Language::English,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Self {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, data);
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TensorL")
        .join("config.json")
}
