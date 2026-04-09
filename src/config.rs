use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Language ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Auto,
    Chinese,
    English,
    Japanese,
    Korean,
    French,
    German,
    Spanish,
    Russian,
    Portuguese,
    Arabic,
    Italian,
}

impl Language {
    pub fn all() -> &'static [Language] {
        use Language::*;
        &[
            Auto, Chinese, English, Japanese, Korean, French, German, Spanish,
            Russian, Portuguese, Arabic, Italian,
        ]
    }

    pub fn all_targets() -> &'static [Language] {
        use Language::*;
        &[
            Chinese, English, Japanese, Korean, French, German, Spanish,
            Russian, Portuguese, Arabic, Italian,
        ]
    }

    /// Name shown in the UI combo-box
    pub fn display_name(self) -> &'static str {
        match self {
            Language::Auto       => "Auto Detect",
            Language::Chinese    => "中文 (Chinese)",
            Language::English    => "English",
            Language::Japanese   => "日本語 (Japanese)",
            Language::Korean     => "한국어 (Korean)",
            Language::French     => "Français (French)",
            Language::German     => "Deutsch (German)",
            Language::Spanish    => "Español (Spanish)",
            Language::Russian    => "Русский (Russian)",
            Language::Portuguese => "Português (Portuguese)",
            Language::Arabic     => "العربية (Arabic)",
            Language::Italian    => "Italiano (Italian)",
        }
    }

    /// English name used in the HY-MT1.5 prompt (non-Chinese instruction branch)
    pub fn hy_mt_en_name(self) -> &'static str {
        match self {
            Language::Auto       => "English",
            Language::Chinese    => "Chinese",
            Language::English    => "English",
            Language::Japanese   => "Japanese",
            Language::Korean     => "Korean",
            Language::French     => "French",
            Language::German     => "German",
            Language::Spanish    => "Spanish",
            Language::Russian    => "Russian",
            Language::Portuguese => "Portuguese",
            Language::Arabic     => "Arabic",
            Language::Italian    => "Italian",
        }
    }

    /// Chinese name used in the HY-MT1.5 prompt (Chinese instruction branch)
    pub fn hy_mt_zh_name(self) -> &'static str {
        match self {
            Language::Auto       => "英语",
            Language::Chinese    => "中文",
            Language::English    => "英语",
            Language::Japanese   => "日语",
            Language::Korean     => "韩语",
            Language::French     => "法语",
            Language::German     => "德语",
            Language::Spanish    => "西班牙语",
            Language::Russian    => "俄语",
            Language::Portuguese => "葡萄牙语",
            Language::Arabic     => "阿拉伯语",
            Language::Italian    => "意大利语",
        }
    }

    pub fn is_chinese(self) -> bool {
        self == Language::Chinese
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::Auto
    }
}

// ── Backend ─────────────────────────────────────────────────────────────────

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

// ── AppConfig ────────────────────────────────────────────────────────────────

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
    // std::thread::available_parallelism is stable since 1.59
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
