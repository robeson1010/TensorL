use std::sync::mpsc;
use std::time::Instant;

use egui::{
    Align, Color32, FontId, Layout, ProgressBar, RichText, Spinner, TextEdit, Vec2,
};

use crate::config::{config_path, AppConfig, Backend, Language};
use crate::translator::{InferRequest, UiMsg};
use crate::tray::{setup_tray, TrayHandle};

// ── Translation state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum TranslationState {
    Idle,
    Translating,
    Done,
    Error(String),
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct TensorLApp {
    // Channels
    ui_rx:    mpsc::Receiver<UiMsg>,
    infer_tx: mpsc::Sender<InferRequest>,

    // Tray (kept alive)
    _tray: TrayHandle,

    // Model state
    model_loaded:   bool,
    model_loading:  bool,
    load_progress:  f32,
    load_stage:     String,

    // Translation
    state:        TranslationState,
    source_text:  String,
    output_text:  String,
    source_lang:  Language,
    target_lang:  Language,

    // UI helpers
    copy_toast:         Option<Instant>,
    window_visible:     bool,
    show_settings:      bool,
    gpu_available:      bool,
    pending_model_path: String,

    // Config
    config: AppConfig,

    // egui context (stored so tray/hotkey callbacks can request repaints)
    ctx: Option<egui::Context>,

    // Win32 HWND for show/hide
    #[cfg(target_os = "windows")]
    hwnd: windows::Win32::Foundation::HWND,
}

impl TensorLApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        ui_rx: mpsc::Receiver<UiMsg>,
        infer_tx: mpsc::Sender<InferRequest>,
        ui_tx: mpsc::Sender<UiMsg>,
        config: AppConfig,
    ) -> Self {
        // --- egui visual style (DeepL dark theme) ---
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill         = Color32::from_rgb(20, 20, 24);
        visuals.window_fill        = Color32::from_rgb(28, 28, 34);
        visuals.extreme_bg_color   = Color32::from_rgb(14, 14, 17);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(32, 32, 40);
        visuals.widgets.inactive.bg_fill       = Color32::from_rgb(40, 40, 50);
        visuals.widgets.hovered.bg_fill        = Color32::from_rgb(55, 55, 70);
        visuals.widgets.active.bg_fill         = Color32::from_rgb(60, 100, 200);
        visuals.selection.bg_fill              = Color32::from_rgb(50, 80, 170);
        cc.egui_ctx.set_visuals(visuals);

        // Font size bump for readability
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::proportional(15.0),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            FontId::monospace(14.0),
        );
        cc.egui_ctx.set_style(style);

        // --- Capture HWND on Windows ---
        #[cfg(target_os = "windows")]
        let hwnd = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match cc.window_handle().ok().map(|h| h.as_raw()) {
                Some(RawWindowHandle::Win32(h)) => {
                    windows::Win32::Foundation::HWND(h.hwnd.get() as isize)
                }
                _ => windows::Win32::Foundation::HWND(0),
            }
        };

        // --- System tray ---
        let _tray = setup_tray(ui_tx);

        let pending_model_path = config.model_path.to_string_lossy().into_owned();

        Self {
            ui_rx,
            infer_tx,
            _tray,
            model_loaded: false,
            model_loading: !config.model_path.as_os_str().is_empty(),
            load_progress: 0.0,
            load_stage: "Loading model…".into(),
            state: TranslationState::Idle,
            source_text: String::new(),
            output_text: String::new(),
            source_lang: config.source_language,
            target_lang: config.target_language,
            copy_toast: None,
            window_visible: true,
            show_settings: false,
            gpu_available: false,
            pending_model_path,
            config,
            ctx: Some(cc.egui_ctx.clone()),
            #[cfg(target_os = "windows")]
            hwnd,
        }
    }

    // ── Window show/hide ─────────────────────────────────────────────────────

    #[cfg(target_os = "windows")]
    fn show_window(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, ShowWindow, SW_SHOWDEFAULT,
        };
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWDEFAULT);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.window_visible = true;
    }

    #[cfg(target_os = "windows")]
    fn hide_window(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        unsafe { ShowWindow(self.hwnd, SW_HIDE); }
        self.window_visible = false;
    }

    #[cfg(not(target_os = "windows"))]
    fn show_window(&mut self) { self.window_visible = true; }
    #[cfg(not(target_os = "windows"))]
    fn hide_window(&mut self) { self.window_visible = false; }

    fn toggle_window(&mut self) {
        if self.window_visible { self.hide_window(); }
        else { self.show_window(); }
    }

    // ── Message draining ─────────────────────────────────────────────────────

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMsg::HotkeyFired => self.on_hotkey(),
                UiMsg::TrayToggle  => self.toggle_window(),
                UiMsg::TrayQuit    => std::process::exit(0),

                UiMsg::GpuAvailable(v) => { self.gpu_available = v; }

                UiMsg::ModelLoaded => {
                    self.model_loaded  = true;
                    self.model_loading = false;
                    self.load_progress = 1.0;
                    self.load_stage    = "Model ready".into();
                }
                UiMsg::ModelLoadProgress { percent, stage } => {
                    self.load_progress = percent;
                    self.load_stage    = stage;
                    self.model_loading = true;
                }
                UiMsg::ModelError(e) => {
                    self.model_loading = false;
                    self.model_loaded  = false;
                    self.state = TranslationState::Error(e);
                }

                UiMsg::Token(t) => {
                    self.output_text.push_str(&t);
                    self.state = TranslationState::Translating;
                }
                UiMsg::TranslationDone  => { self.state = TranslationState::Done; }
                UiMsg::TranslationError(e) => { self.state = TranslationState::Error(e); }
            }
        }
    }

    fn on_hotkey(&mut self) {
        self.show_window();
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(text) = cb.get_text() {
                if !text.trim().is_empty() {
                    self.source_text = text;
                    self.start_translation();
                }
            }
        }
    }

    fn start_translation(&mut self) {
        if self.source_text.trim().is_empty() || !self.model_loaded { return; }
        self.output_text.clear();
        self.state = TranslationState::Translating;
        let _ = self.infer_tx.send(InferRequest::Translate {
            text:   self.source_text.clone(),
            source: self.source_lang,
            target: self.target_lang,
        });
    }

    fn save_config(&mut self) {
        self.config.source_language = self.source_lang;
        self.config.target_language = self.target_lang;
        self.config.save(&config_path());
    }

    // ── UI drawing ────────────────────────────────────────────────────────────

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar")
            .exact_height(48.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(16, 16, 20))
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // App logo + title
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/icon.png"))
                            .max_size(Vec2::splat(32.0)),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("TensorL")
                            .size(18.0)
                            .color(Color32::from_rgb(120, 180, 255))
                            .strong(),
                    );

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Source language
                    egui::ComboBox::from_id_source("src_lang")
                        .selected_text(self.source_lang.display_name())
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for &lang in Language::all() {
                                let changed = ui
                                    .selectable_value(
                                        &mut self.source_lang,
                                        lang,
                                        lang.display_name(),
                                    )
                                    .changed();
                                if changed { self.save_config(); }
                            }
                        });

                    ui.add_space(8.0);

                    // Swap button
                    if ui
                        .button(RichText::new("⇄").size(16.0))
                        .on_hover_text("Swap languages")
                        .clicked()
                    {
                        if self.source_lang != Language::Auto {
                            std::mem::swap(&mut self.source_lang, &mut self.target_lang);
                            std::mem::swap(&mut self.source_text, &mut self.output_text);
                            self.save_config();
                        }
                    }

                    ui.add_space(8.0);

                    // Target language
                    egui::ComboBox::from_id_source("tgt_lang")
                        .selected_text(self.target_lang.display_name())
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for &lang in Language::all_targets() {
                                let changed = ui
                                    .selectable_value(
                                        &mut self.target_lang,
                                        lang,
                                        lang.display_name(),
                                    )
                                    .changed();
                                if changed { self.save_config(); }
                            }
                        });

                    // Push settings gear to the right
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let settings_label = if self.show_settings { "✕" } else { "⚙" };
                        if ui
                            .button(RichText::new(settings_label).size(16.0))
                            .on_hover_text("Settings")
                            .clicked()
                        {
                            self.show_settings = !self.show_settings;
                        }
                    });
                });
            });
    }

    fn draw_main_panels(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(20, 20, 24)))
            .show(ctx, |ui| {
                // First-run: model not configured
                if !self.model_loaded
                    && !self.model_loading
                    && (self.config.model_path.as_os_str().is_empty()
                        || !self.config.model_path.exists())
                {
                    self.draw_first_run(ui);
                    return;
                }

                ui.columns(2, |cols| {
                    // ── LEFT: source text ─────────────────────────────────
                    cols[0].vertical(|ui| {
                        let available = ui.available_size();
                        let text_h = available.y - 40.0;

                        ui.add_sized(
                            [available.x, text_h],
                            TextEdit::multiline(&mut self.source_text)
                                .hint_text(
                                    "Enter text to translate,\nor press Ctrl+C twice to\npaste from clipboard…",
                                )
                                .font(egui::TextStyle::Body),
                        );

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let count = self.source_text.chars().count();
                            ui.label(
                                RichText::new(format!("{count} chars"))
                                    .color(Color32::GRAY)
                                    .small(),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let btn = egui::Button::new(
                                    RichText::new("Translate").size(13.0),
                                )
                                .min_size(Vec2::new(90.0, 28.0));

                                if !self.model_loaded {
                                    ui.add_enabled(false, btn);
                                    ui.add_space(6.0);
                                    ui.add(Spinner::new());
                                    ui.add(
                                        ProgressBar::new(self.load_progress)
                                            .desired_width(80.0),
                                    );
                                    ui.label(
                                        RichText::new(&self.load_stage)
                                            .small()
                                            .color(Color32::GRAY),
                                    );
                                } else if ui.add(btn).clicked() {
                                    self.start_translation();
                                }

                                // Clear source
                                if ui
                                    .small_button("✕")
                                    .on_hover_text("Clear")
                                    .clicked()
                                {
                                    self.source_text.clear();
                                }
                            });
                        });
                    });

                    // ── RIGHT: translation output ─────────────────────────
                    cols[1].vertical(|ui| {
                        let available = ui.available_size();
                        let text_h = available.y - 40.0;

                        egui::Frame::none()
                            .fill(Color32::from_rgb(14, 14, 17))
                            .rounding(4.0)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(text_h)
                                    .auto_shrink([false; 2])
                                    .stick_to_bottom(true)
                                    .show(ui, |ui| {
                                        ui.set_min_size(Vec2::new(available.x - 8.0, text_h));
                                        ui.add(
                                            TextEdit::multiline(&mut self.output_text.as_str())
                                                .desired_width(f32::INFINITY)
                                                .font(egui::TextStyle::Body)
                                                .frame(false),
                                        );
                                    });
                            });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            // Copy button
                            if ui
                                .add(egui::Button::new("Copy").min_size(Vec2::new(60.0, 24.0)))
                                .clicked()
                                && !self.output_text.is_empty()
                            {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(&self.output_text);
                                    self.copy_toast = Some(Instant::now());
                                }
                            }

                            if let Some(t) = self.copy_toast {
                                if t.elapsed().as_secs() < 2 {
                                    ui.label(
                                        RichText::new("Copied!")
                                            .color(Color32::from_rgb(100, 220, 100))
                                            .small(),
                                    );
                                } else {
                                    self.copy_toast = None;
                                }
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                match &self.state.clone() {
                                    TranslationState::Translating => {
                                        ui.add(Spinner::new());
                                        ui.label(
                                            RichText::new("Translating…")
                                                .small()
                                                .color(Color32::GRAY),
                                        );
                                    }
                                    TranslationState::Error(e) => {
                                        ui.label(
                                            RichText::new(format!("Error: {e}"))
                                                .small()
                                                .color(Color32::RED),
                                        );
                                    }
                                    _ => {}
                                }
                            });
                        });
                    });
                });
            });
    }

    fn draw_first_run(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.add(
                egui::Image::new(egui::include_image!("../assets/icon.png"))
                    .max_size(Vec2::splat(72.0)),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Welcome to TensorL")
                    .size(22.0)
                    .color(Color32::from_rgb(120, 180, 255))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Select the HY-MT1.5-1.8B GGUF model file to get started.")
                    .color(Color32::GRAY),
            );
            ui.add_space(20.0);

            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 4.0);
                ui.add(
                    TextEdit::singleline(&mut self.pending_model_path)
                        .hint_text("Path to .gguf file…")
                        .desired_width(300.0),
                );
                if ui.button("Browse…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("GGUF model", &["gguf"])
                        .pick_file()
                    {
                        self.pending_model_path = p.to_string_lossy().into_owned();
                    }
                }
            });

            ui.add_space(12.0);

            let valid = std::path::Path::new(&self.pending_model_path).exists();
            ui.add_enabled_ui(valid, |ui| {
                if ui
                    .add(egui::Button::new("Load Model").min_size(Vec2::new(120.0, 32.0)))
                    .clicked()
                {
                    self.config.model_path =
                        std::path::PathBuf::from(&self.pending_model_path);
                    self.config.save(&config_path());
                    self.model_loading = true;
                    let _ = self.infer_tx.send(InferRequest::Reload(self.config.clone()));
                }
            });

            if !valid && !self.pending_model_path.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("File not found.")
                        .color(Color32::RED)
                        .small(),
                );
            }

            ui.add_space(20.0);
            ui.label(
                RichText::new(
                    "Download the model from:\nhttps://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF",
                )
                .color(Color32::GRAY)
                .small(),
            );
        });
    }

    fn draw_settings_panel(&mut self, ctx: &egui::Context) {
        if !self.show_settings { return; }

        egui::SidePanel::right("settings_panel")
            .resizable(false)
            .exact_width(260.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(24, 24, 32))
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.label(RichText::new("Settings").size(16.0).strong());
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // Model path
                ui.label(RichText::new("Model file").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.pending_model_path)
                            .desired_width(160.0),
                    );
                    if ui.small_button("…").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("GGUF model", &["gguf"])
                            .pick_file()
                        {
                            self.pending_model_path = p.to_string_lossy().into_owned();
                        }
                    }
                });

                ui.add_space(12.0);

                // Backend selection
                ui.label(RichText::new("Inference backend").strong());
                ui.add_space(4.0);

                let prev_backend = self.config.backend;
                ui.radio_value(&mut self.config.backend, Backend::Cpu, "CPU");

                let gpu_label = if self.gpu_available {
                    "GPU (CUDA/Vulkan)"
                } else {
                    "GPU (not available in this build)"
                };
                ui.add_enabled_ui(self.gpu_available, |ui| {
                    ui.radio_value(&mut self.config.backend, Backend::Gpu, gpu_label);
                });

                if !self.gpu_available && self.config.backend == Backend::Gpu {
                    self.config.backend = Backend::Cpu;
                }

                ui.add_space(4.0);
                ui.label(
                    RichText::new(if self.gpu_available {
                        "Built with GPU support."
                    } else {
                        "Rebuild with --features cuda for GPU support."
                    })
                    .small()
                    .color(Color32::GRAY),
                );

                ui.add_space(12.0);

                // Threads (CPU only)
                if self.config.backend == Backend::Cpu {
                    ui.label(RichText::new("CPU threads").strong());
                    ui.add(
                        egui::Slider::new(&mut self.config.n_threads, 1..=32)
                            .text("threads"),
                    );
                    ui.add_space(12.0);
                }

                ui.separator();
                ui.add_space(12.0);

                // Apply / Reload
                let path_changed =
                    self.pending_model_path != self.config.model_path.to_string_lossy();
                let backend_changed = self.config.backend != prev_backend;
                let needs_reload = path_changed || backend_changed;

                ui.add_enabled_ui(needs_reload, |ui| {
                    if ui
                        .add(
                            egui::Button::new("Apply & Reload Model")
                                .min_size(Vec2::new(220.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.config.model_path =
                            std::path::PathBuf::from(&self.pending_model_path);
                        self.config.save(&config_path());
                        self.model_loaded  = false;
                        self.model_loading = true;
                        self.output_text.clear();
                        self.state = TranslationState::Idle;
                        let _ = self.infer_tx.send(InferRequest::Reload(self.config.clone()));
                    }
                });
            });
    }

    fn draw_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(12, 12, 16))
                    .inner_margin(egui::Margin::symmetric(10.0, 2.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let status = if self.model_loaded {
                        format!(
                            "Model: {}  |  Backend: {}",
                            self.config
                                .model_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            self.config.backend.display_name()
                        )
                    } else if self.model_loading {
                        format!("{} ({:.0}%)", self.load_stage, self.load_progress * 100.0)
                    } else {
                        "No model loaded — open Settings to select a GGUF file".into()
                    };
                    ui.label(RichText::new(status).small().color(Color32::GRAY));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("Ctrl+C+C to translate clipboard")
                                .small()
                                .color(Color32::from_rgb(70, 70, 90)),
                        );
                    });
                });
            });
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for TensorLApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();

        self.draw_settings_panel(ctx);
        self.draw_top_bar(ctx);
        self.draw_status_bar(ctx);
        self.draw_main_panels(ctx);

        // Keep the loop alive even when minimised/hidden so hotkey events are processed
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
    }
}
