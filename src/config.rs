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

    /// English name used in the HY-MT1.5 prompt (non-Chinese instruction branch)
    pub fn hy_mt_en_name(self) -> &'static str {
        match self {
            Language::Auto              => "English",
            Language::Chinese           => "Chinese",
            Language::English           => "English",
            Language::French            => "French",
            Language::Portuguese        => "Portuguese",
            Language::Spanish           => "Spanish",
            Language::Japanese          => "Japanese",
            Language::Turkish           => "Turkish",
            Language::Russian           => "Russian",
            Language::Arabic            => "Arabic",
            Language::Korean            => "Korean",
            Language::Thai              => "Thai",
            Language::Italian           => "Italian",
            Language::German            => "German",
            Language::Vietnamese        => "Vietnamese",
            Language::Malay             => "Malay",
            Language::Indonesian        => "Indonesian",
            Language::Filipino          => "Filipino",
            Language::Polish            => "Polish",
            Language::Czech             => "Czech",
            Language::Dutch             => "Dutch",
            Language::Ukrainian         => "Ukrainian",
            Language::Kazakh            => "Kazakh",
            Language::Mongolian         => "Mongolian",
            Language::Cantonese         => "Cantonese",
        }
    }

    /// Chinese name used in the HY-MT1.5 prompt (Chinese instruction branch)
    pub fn hy_mt_zh_name(self) -> &'static str {
        match self {
            Language::Auto              => "英语",
            Language::Chinese           => "中文",
            Language::English           => "英语",
            Language::French            => "法语",
            Language::Portuguese        => "葡萄牙语",
            Language::Spanish           => "西班牙语",
            Language::Japanese          => "日语",
            Language::Turkish           => "土耳其语",
            Language::Russian           => "俄语",
            Language::Arabic            => "阿拉伯语",
            Language::Korean            => "韩语",
            Language::Thai              => "泰语",
            Language::Italian           => "意大利语",
            Language::German            => "德语",
            Language::Vietnamese        => "越南语",
            Language::Malay             => "马来语",
            Language::Indonesian        => "印尼语",
            Language::Filipino          => "菲律宾语",
            Language::Polish            => "波兰语",
            Language::Czech             => "捷克语",
            Language::Dutch             => "荷兰语",
            Language::Ukrainian         => "乌克兰语",
            Language::Kazakh            => "哈萨克语",
            Language::Mongolian         => "蒙古语",
            Language::Cantonese         => "粤语",
        }
    }

    pub fn is_chinese(self) -> bool {
        matches!(
            self,
            Language::Chinese | Language::Cantonese
        )
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::Auto
    }
}

// ── Backend ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Backend {
    #[default]
    Cpu,
    Gpu,
}

impl Backend {
    pub fn display_name(self) -> &'static str {
        match self {
            Backend::Cpu => "CPU",
            Backend::Gpu => "GPU (CUDA/Vulkan)",
        }
    }
}

// ── AppConfig ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub model_path:      PathBuf,
    pub source_language: Language,
    pub target_language: Language,
    pub backend:         Backend,
    /// Number of model layers to offload to GPU (99 = all)
    pub n_gpu_layers:    i32,
    /// Context size in tokens
    pub n_ctx:           u32,
    /// Number of CPU threads for inference
    pub n_threads:       u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        let n_threads = (num_cpus() / 2).max(1) as u32;
        Self {
            model_path:      PathBuf::new(),
            source_language: Language::Auto,
            target_language: Language::English,
            backend:         Backend::Cpu,
            n_gpu_layers:    99,
            n_ctx:           2048,
            n_threads,
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

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
