use std::sync::mpsc;
use std::time::Instant;

use egui::{
    Align, Color32, FontId, Layout, RichText, Rounding, Sense, Stroke,
    TextEdit, Vec2,
};

use crate::config::{config_path, AppConfig, Language};
use crate::translator::{InferRequest, UiMsg};

// ── Constants ────────────────────────────────────────────────────────────────

const MAX_CHARS: usize = 5000;

// Color palette (matching LocalTrans dark theme)
const BG_MAIN: Color32       = Color32::from_rgb(26, 26, 30);
const BG_TOPBAR: Color32     = Color32::from_rgb(22, 22, 26);
const BG_STATUS: Color32     = Color32::from_rgb(18, 18, 22);
const BG_PANEL: Color32      = Color32::from_rgb(36, 36, 42);
const BG_INPUT: Color32      = Color32::from_rgb(30, 30, 36);
const BG_PILL: Color32       = Color32::from_rgb(44, 44, 52);
const BG_PILL_HOVER: Color32 = Color32::from_rgb(54, 54, 64);
const ACCENT_PURPLE: Color32 = Color32::from_rgb(110, 80, 220);
const TEXT_PRIMARY: Color32  = Color32::from_rgb(230, 230, 235);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 155);
const TEXT_HINT: Color32     = Color32::from_rgb(80, 80, 100);
const GREEN_DOT: Color32     = Color32::from_rgb(70, 200, 70);
const BORDER_SUBTLE: Color32 = Color32::from_rgb(50, 50, 60);
const RED_ERROR: Color32     = Color32::from_rgb(220, 70, 70);

// ── Translation state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum TranslationState {
    Idle,
    Translating,
    Done,
    Error(String),
}

// ── History ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct HistoryEntry {
    source: String,
    result: String,
    source_lang: Language,
    target_lang: Language,
}

// ── App ──────────────────────────────────────────────────────────────────────

pub struct TensorLApp {
    // Channels
    ui_rx:    mpsc::Receiver<UiMsg>,
    infer_tx: mpsc::Sender<InferRequest>,

    // Translation
    state:        TranslationState,
    source_text:  String,
    output_text:  String,
    source_lang:  Language,
    target_lang:  Language,

    // History
    history:      Vec<HistoryEntry>,
    show_history: bool,
    show_donate:  bool,
    show_quit_confirm: bool,

    // UI helpers
    copy_toast:          Option<Instant>,
    show_all_src_langs:  bool,
    show_all_tgt_langs:  bool,

    // Config
    config: AppConfig,

    // egui context for viewport commands
    ctx: egui::Context,

    // Win32 HWND for minimize/restore
    #[cfg(target_os = "windows")]
    hwnd: windows::Win32::Foundation::HWND,
}

impl TensorLApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        ui_rx: mpsc::Receiver<UiMsg>,
        infer_tx: mpsc::Sender<InferRequest>,
        config: AppConfig,
    ) -> Self {
        // --- Install PNG/image loader ---
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // --- Font setup ---
        let mut fonts = egui::FontDefinitions::default();

        // 1. Symbol font
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\seguisym.ttf") {
            fonts.font_data.insert("symbol_font".to_owned(), egui::FontData::from_owned(data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("symbol_font".to_owned());
        }

        // 2. CJK font
        let cjk_font_paths = [
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\simsun.ttc",
        ];
        for path in &cjk_font_paths {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert("cjk_font".to_owned(), egui::FontData::from_owned(data));
                fonts.families.entry(egui::FontFamily::Proportional).or_default().push("cjk_font".to_owned());
                fonts.families.entry(egui::FontFamily::Monospace).or_default().push("cjk_font".to_owned());
                break;
            }
        }

        // 3. Korean font (Malgun Gothic)
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\malgun.ttf") {
            fonts.font_data.insert("korean_font".to_owned(), egui::FontData::from_owned(data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("korean_font".to_owned());
        }

        // 4. Thai font (Leelawadee UI)
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\leelawui.ttf") {
            fonts.font_data.insert("thai_font".to_owned(), egui::FontData::from_owned(data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("thai_font".to_owned());
        }

        // 5. Arabic font (Segoe UI)
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts.font_data.insert("arabic_font".to_owned(), egui::FontData::from_owned(data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("arabic_font".to_owned());
        }

        cc.egui_ctx.set_fonts(fonts);

        // --- Visual style ---
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill         = BG_MAIN;
        visuals.window_fill        = Color32::from_rgb(32, 32, 38);
        visuals.extreme_bg_color   = BG_INPUT;
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(36, 36, 42);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
        visuals.widgets.inactive.bg_fill       = BG_PILL;
        visuals.widgets.inactive.fg_stroke     = Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.inactive.rounding      = Rounding::same(8.0);
        visuals.widgets.hovered.bg_fill        = BG_PILL_HOVER;
        visuals.widgets.hovered.fg_stroke      = Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.hovered.rounding       = Rounding::same(8.0);
        visuals.widgets.active.bg_fill         = ACCENT_PURPLE;
        visuals.widgets.active.rounding        = Rounding::same(8.0);
        visuals.selection.bg_fill              = Color32::from_rgb(60, 50, 120);
        visuals.window_rounding                = Rounding::same(12.0);
        visuals.menu_rounding                  = Rounding::same(8.0);
        cc.egui_ctx.set_visuals(visuals);

        // Font size
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(14.0));
        style.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        cc.egui_ctx.set_style(style);

        // --- Capture HWND on Windows and fix minimize support ---
        #[cfg(target_os = "windows")]
        let hwnd = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let h = match cc.window_handle().ok().map(|h| h.as_raw()) {
                Some(RawWindowHandle::Win32(h)) => {
                    windows::Win32::Foundation::HWND(h.hwnd.get() as *mut std::ffi::c_void)
                }
                _ => windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            };
            // Add WS_MINIMIZEBOX so frameless window can minimize without crashing
            if !h.is_invalid() {
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::*;
                    let style = GetWindowLongPtrW(h, GWL_STYLE);
                    let ws_minimizebox = 0x0002_0000i32;
                    let ws_sysmenu     = 0x0008_0000i32;
                    SetWindowLongPtrW(h, GWL_STYLE, style | ws_minimizebox as isize | ws_sysmenu as isize);
                }
            }
            h
        };

        Self {
            ui_rx,
            infer_tx,
            state: TranslationState::Idle,
            source_text: String::new(),
            output_text: String::new(),
            source_lang: config.source_language,
            target_lang: config.target_language,
            history: Vec::new(),
            show_history: false,
            show_donate: false,
            show_quit_confirm: false,
            copy_toast: None,
            show_all_src_langs: false,
            show_all_tgt_langs: false,
            config,
            ctx: cc.egui_ctx.clone(),
            #[cfg(target_os = "windows")]
            hwnd,
        }
    }

    // ── Message draining ─────────────────────────────────────────────────────

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMsg::HotkeyFired => self.on_hotkey(),

                UiMsg::Token(t) => {
                    self.output_text.push_str(&t);
                    self.state = TranslationState::Translating;
                }
                UiMsg::TranslationDone => {
                    self.output_text = self.output_text.trim_end().to_string();
                    self.state = TranslationState::Done;

                    // Save to history
                    if !self.source_text.is_empty() && !self.output_text.is_empty() {
                        self.history.insert(0, HistoryEntry {
                            source: self.source_text.clone(),
                            result: self.output_text.clone(),
                            source_lang: self.source_lang,
                            target_lang: self.target_lang,
                        });
                        if self.history.len() > 50 {
                            self.history.truncate(50);
                        }
                    }
                }
                UiMsg::TranslationError(e) => { self.state = TranslationState::Error(e); }
            }
        }
    }

    fn on_hotkey(&mut self) {
        // Restore and bring window to front via Win32 API
        // (ViewportCommand::Minimized crashes on frameless windows)
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::*;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);

                extern "system" {
                    fn GetCurrentThreadId() -> u32;
                    fn AttachThreadInput(attach: u32, attach_to: u32, do_attach: i32) -> i32;
                }
                let fg = GetForegroundWindow();
                let fg_thread = GetWindowThreadProcessId(fg, None);
                let our_thread = GetCurrentThreadId();
                if fg_thread != our_thread && fg_thread != 0 {
                    AttachThreadInput(our_thread, fg_thread, 1);
                    let _ = SetForegroundWindow(self.hwnd);
                    AttachThreadInput(our_thread, fg_thread, 0);
                } else {
                    let _ = SetForegroundWindow(self.hwnd);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

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
        if self.source_text.trim().is_empty() { return; }
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

    fn detect_is_chinese(text: &str) -> bool {
        let total = text.chars().count();
        if total == 0 { return false; }
        let cjk = text.chars().filter(|&c| matches!(c,
            '\u{4E00}'..='\u{9FFF}' |
            '\u{3400}'..='\u{4DBF}' |
            '\u{F900}'..='\u{FAFF}'
        )).count();
        cjk * 100 / total > 20
    }

    // ── UI: Top Bar ──────────────────────────────────────────────────────────

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar")
            .exact_height(48.0)
            .frame(
                egui::Frame::none()
                    .fill(BG_TOPBAR)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                // The entire top bar acts as a drag area for the frameless window
                let title_bar_rect = ui.max_rect();
                let title_bar_response = ui.interact(
                    title_bar_rect,
                    egui::Id::new("title_bar_drag"),
                    Sense::click(),
                );
                if title_bar_response.is_pointer_button_down_on() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.horizontal_centered(|ui| {
                    // App logo + title
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/icon.png"))
                            .max_size(Vec2::splat(24.0)),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("TensorL")
                            .size(20.0)
                            .color(TEXT_PRIMARY)
                            .strong(),
                    );

                    ui.add_space(20.0);

                    // Source language pill
                    self.draw_lang_pill(ui, true);

                    ui.add_space(4.0);

                    // Arrow indicator
                    ui.label(
                        RichText::new("\u{25B6}").size(12.0).color(TEXT_SECONDARY),
                    );

                    ui.add_space(4.0);

                    // Target language pill
                    self.draw_lang_pill(ui, false);

                    // Push buttons to right
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Close / quit button
                        if ui.add(
                            egui::Button::new(
                                RichText::new("\u{2715}").size(14.0).color(TEXT_SECONDARY),
                            )
                            .rounding(Rounding::same(12.0))
                            .min_size(Vec2::new(28.0, 28.0))
                            .fill(Color32::TRANSPARENT),
                        ).on_hover_text("退出").clicked() {
                            self.show_quit_confirm = true;
                        }

                        // Minimize
                        if ui.add(
                            egui::Button::new(
                                RichText::new("\u{2014}").size(14.0).color(TEXT_SECONDARY),
                            )
                            .rounding(Rounding::same(12.0))
                            .min_size(Vec2::new(28.0, 28.0))
                            .fill(Color32::TRANSPARENT),
                        ).on_hover_text("最小化").clicked() {
                            // Use Win32 ShowWindow to minimize frameless window
                            // (ViewportCommand::Minimized causes exit code 101)
                            #[cfg(target_os = "windows")]
                            unsafe {
                                use windows::Win32::UI::WindowsAndMessaging::*;
                                let _ = ShowWindow(self.hwnd, SW_MINIMIZE);
                            }
                            #[cfg(not(target_os = "windows"))]
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }

                        // Only show the history button when no panel is open
                        if !self.show_history {
                            ui.add_space(8.0);

                            // History button
                            if ui.add(
                                egui::Button::new(
                                    RichText::new("\u{1F552}").size(14.0).color(TEXT_SECONDARY),
                                )
                                .rounding(Rounding::same(12.0))
                                .min_size(Vec2::new(28.0, 28.0))
                                .fill(Color32::TRANSPARENT),
                            ).on_hover_text("翻译历史").clicked() {
                                self.show_history = true;
                            }
                        }
                    });
                });
            });
    }

    /// Draw a pill-shaped language selector (ComboBox with custom style)
    fn draw_lang_pill(&mut self, ui: &mut egui::Ui, is_source: bool) {
        let id_salt = if is_source { "src_lang_pill" } else { "tgt_lang_pill" };
        let current = if is_source { self.source_lang } else { self.target_lang };

        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(RichText::new(current.display_name()).size(14.0).color(TEXT_PRIMARY))
            .width(160.0)
            .show_ui(ui, |ui| {
                let show_all = if is_source { self.show_all_src_langs } else { self.show_all_tgt_langs };
                let langs = if is_source {
                    if show_all { Language::all() } else { Language::common_sources() }
                } else {
                    if show_all { Language::all_targets() } else { Language::common_targets() }
                };

                for &lang in langs {
                    let lang_ref = if is_source { &mut self.source_lang } else { &mut self.target_lang };
                    let changed = ui
                        .selectable_value(lang_ref, lang, lang.display_name())
                        .changed();
                    if changed { self.save_config(); }
                }

                if !show_all {
                    ui.separator();
                    if ui.button("更多语言\u{2026}").clicked() {
                        if is_source {
                            self.show_all_src_langs = true;
                        } else {
                            self.show_all_tgt_langs = true;
                        }
                    }
                }
            });
    }

    // ── UI: Main Panels ──────────────────────────────────────────────────────

    fn draw_main_panels(&mut self, ctx: &egui::Context) {
        let source_id = egui::Id::new("source_text_edit");
        let source_focused = ctx.memory(|m| m.has_focus(source_id));

        // Capture paste event text before the TextEdit consumes it
        let pasted_text: Option<String> = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Paste(s) = e { Some(s.clone()) } else { None }
            })
        });

        // Consume plain Enter (no Shift) when source box is focused so the
        // TextEdit does not insert a newline — we use it to trigger translation.
        let enter_to_translate = source_focused && ctx.input_mut(|i| {
            let pressed = i.key_pressed(egui::Key::Enter) && !i.modifiers.shift;
            if pressed {
                i.events.retain(|e| {
                    !matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, .. })
                });
            }
            pressed
        });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(BG_MAIN)
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0)),
            )
            .show(ctx, |ui| {
                let total_width = ui.available_width();
                let panel_width = (total_width - 12.0) / 2.0;
                let panel_height = ui.available_height();

                ui.horizontal(|ui| {
                    // ── Left panel: Source ────────────────────────────
                    ui.vertical(|ui| {
                        ui.set_width(panel_width);
                        ui.set_height(panel_height);

                        // Section header
                        ui.label(
                            RichText::new("源文本")
                                .size(13.0)
                                .color(TEXT_SECONDARY)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        let text_h = panel_height - 60.0;

                        // Text area with rounded frame
                        egui::Frame::none()
                            .fill(BG_PANEL)
                            .rounding(Rounding::same(10.0))
                            .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                            .inner_margin(egui::Margin::same(10.0))
                            .show(ui, |ui| {
                                // Enforce character limit
                                if self.source_text.chars().count() > MAX_CHARS {
                                    let truncated: String = self.source_text.chars().take(MAX_CHARS).collect();
                                    self.source_text = truncated;
                                }

                                egui::ScrollArea::vertical()
                                    .id_salt("source_scroll")
                                    .max_height(text_h - 24.0)
                                    .auto_shrink([false; 2])
                                    .show(ui, |ui| {
                                        ui.add_sized(
                                            [panel_width - 34.0, text_h - 28.0],
                                            TextEdit::multiline(&mut self.source_text)
                                                .id(source_id)
                                                .hint_text(
                                                    RichText::new("在此输入文本...")
                                                        .color(TEXT_HINT)
                                                        .size(15.0),
                                                )
                                                .font(egui::TextStyle::Body)
                                                .text_color(TEXT_PRIMARY)
                                                .frame(false),
                                        );
                                    });
                            });

                        ui.add_space(4.0);

                        // Bottom bar: trash left, char count right
                        ui.horizontal(|ui| {
                            // Trash / clear button
                            if ui.add(
                                egui::Button::new(
                                    RichText::new("\u{1F5D1}").size(21.0).color(TEXT_SECONDARY),
                                )
                                .fill(Color32::TRANSPARENT)
                                .rounding(Rounding::same(6.0))
                                .min_size(Vec2::new(28.0, 24.0)),
                            ).on_hover_text("清空").clicked() {
                                self.source_text.clear();
                                self.output_text.clear();
                                self.state = TranslationState::Idle;
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let count = self.source_text.chars().count();
                                let count_color = if count >= MAX_CHARS { RED_ERROR } else { TEXT_SECONDARY };
                                ui.label(
                                    RichText::new(format!("{} / {}\u{5B57}", count, MAX_CHARS))
                                        .size(12.0)
                                        .color(count_color),
                                );

                                // Spinner while a translation request is in flight
                                if self.state == TranslationState::Translating {
                                    ui.spinner();
                                }
                            });
                        });
                    });

                    ui.add_space(12.0);

                    // ── Right panel: Translation ─────────────────────
                    ui.vertical(|ui| {
                        ui.set_width(panel_width);
                        ui.set_height(panel_height);

                        // Section header
                        ui.label(
                            RichText::new("翻译结果")
                                .size(13.0)
                                .color(TEXT_SECONDARY)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        let text_h = panel_height - 60.0;

                        // Output text area
                        egui::Frame::none()
                            .fill(BG_PANEL)
                            .rounding(Rounding::same(10.0))
                            .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                            .inner_margin(egui::Margin::same(10.0))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("output_scroll")
                                    .max_height(text_h - 24.0)
                                    .auto_shrink([false; 2])
                                    .stick_to_bottom(true)
                                    .show(ui, |ui| {
                                        ui.set_min_size(Vec2::new(panel_width - 34.0, text_h - 28.0));
                                        if self.output_text.is_empty() && self.state == TranslationState::Idle {
                                            ui.label(
                                                RichText::new("翻译结果将在此显示...")
                                                    .color(TEXT_HINT)
                                                    .size(15.0),
                                            );
                                        } else {
                                            ui.add(
                                                TextEdit::multiline(&mut self.output_text.as_str())
                                                    .desired_width(f32::INFINITY)
                                                    .font(egui::TextStyle::Body)
                                                    .text_color(TEXT_PRIMARY)
                                                    .frame(false),
                                            );
                                        }
                                    });
                            });

                        ui.add_space(4.0);

                        // Bottom bar: status left, copy + retry right
                        ui.horizontal(|ui| {
                            // Toast
                            if let Some(t) = self.copy_toast {
                                if t.elapsed().as_secs() < 2 {
                                    ui.label(
                                        RichText::new("已复制!")
                                            .size(12.0)
                                            .color(GREEN_DOT),
                                    );
                                } else {
                                    self.copy_toast = None;
                                }
                            }

                            // Error display
                            if let TranslationState::Error(ref e) = self.state.clone() {
                                ui.label(
                                    RichText::new(format!("错误: {e}"))
                                        .size(12.0)
                                        .color(RED_ERROR),
                                );
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // Retry button
                                if ui.add(
                                    egui::Button::new(
                                        RichText::new("\u{27A4}").size(24.0).color(TEXT_SECONDARY),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .rounding(Rounding::same(6.0))
                                    .min_size(Vec2::new(28.0, 24.0)),
                                ).on_hover_text("重新翻译").clicked() {
                                    self.start_translation();
                                }

                                ui.add_space(2.0);

                                // Copy button
                                if ui.add(
                                    egui::Button::new(
                                        RichText::new("\u{29C9}").size(24.0).color(TEXT_SECONDARY),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .rounding(Rounding::same(6.0))
                                    .min_size(Vec2::new(28.0, 24.0)),
                                ).on_hover_text("复制").clicked()
                                    && !self.output_text.is_empty()
                                {
                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                        let _ = cb.set_text(&self.output_text);
                                        self.copy_toast = Some(Instant::now());
                                    }
                                }
                            });
                        });
                    });
                });
            });

        // Auto-detect language and translate after paste
        if let Some(ref pasted) = pasted_text {
            if !pasted.trim().is_empty() {
                self.target_lang = if Self::detect_is_chinese(pasted) {
                    Language::English
                } else {
                    Language::Chinese
                };
                self.save_config();
                self.start_translation();
            }
        } else if enter_to_translate && !self.source_text.trim().is_empty() {
            self.start_translation();
        }
    }

    // ── UI: History Panel ────────────────────────────────────────────────────

    fn draw_history_panel(&mut self, ctx: &egui::Context) {
        if !self.show_history { return; }

        egui::SidePanel::right("history_panel")
            .resizable(false)
            .exact_width(280.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(28, 28, 34))
                    .inner_margin(egui::Margin::same(16.0))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("翻译历史").size(16.0).color(TEXT_PRIMARY).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("\u{2715}").size(14.0).color(TEXT_SECONDARY))
                                .fill(Color32::TRANSPARENT)
                                .rounding(Rounding::same(10.0))
                                .min_size(Vec2::new(24.0, 24.0)),
                        ).clicked() {
                            self.show_history = false;
                        }
                        if ui.small_button("清空").clicked() {
                            self.history.clear();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                if self.history.is_empty() {
                    ui.label(
                        RichText::new("暂无翻译历史")
                            .color(TEXT_HINT)
                            .size(13.0),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let mut load_entry = None;
                            for (i, entry) in self.history.iter().enumerate() {
                                let source_preview: String = entry.source.chars().take(40).collect();
                                let result_preview: String = entry.result.chars().take(40).collect();

                                egui::Frame::none()
                                    .fill(BG_PANEL)
                                    .rounding(Rounding::same(8.0))
                                    .inner_margin(egui::Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} \u{2192} {}",
                                                entry.source_lang.display_name(),
                                                entry.target_lang.display_name()
                                            ))
                                            .size(11.0)
                                            .color(TEXT_HINT),
                                        );
                                        ui.label(
                                            RichText::new(&source_preview)
                                                .size(12.0)
                                                .color(TEXT_SECONDARY),
                                        );
                                        ui.label(
                                            RichText::new(&result_preview)
                                                .size(12.0)
                                                .color(TEXT_PRIMARY),
                                        );
                                        if ui.small_button("加载").clicked() {
                                            load_entry = Some(i);
                                        }
                                    });

                                ui.add_space(4.0);
                            }

                            if let Some(i) = load_entry {
                                let entry = self.history[i].clone();
                                self.source_text = entry.source;
                                self.output_text = entry.result;
                                self.source_lang = entry.source_lang;
                                self.target_lang = entry.target_lang;
                                self.state = TranslationState::Done;
                            }
                        });
                }
            });
    }

    // ── UI: Status Bar ───────────────────────────────────────────────────────

    fn draw_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .frame(
                egui::Frame::none()
                    .fill(BG_STATUS)
                    .inner_margin(egui::Margin::symmetric(12.0, 3.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Left: online status with colored dot
                    if let TranslationState::Error(_) = self.state {
                        let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(dot_rect.center(), 4.0, RED_ERROR);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("翻译失败")
                                .size(12.0)
                                .color(TEXT_HINT),
                        );
                    } else {
                        let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(dot_rect.center(), 4.0, GREEN_DOT);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Google 翻译 · 在线")
                                .size(12.0)
                                .color(TEXT_SECONDARY),
                        );
                    }

                    // Right: donate heart → translating status / hint
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Heart / donate button (rightmost corner)
                        if ui.add(
                            egui::Button::new(
                                RichText::new("\u{2764}").size(12.0).color(Color32::from_rgb(220, 80, 80)),
                            )
                            .fill(Color32::TRANSPARENT)
                            .rounding(Rounding::same(8.0))
                            .min_size(Vec2::new(20.0, 20.0)),
                        ).on_hover_text("Buy me a Coffee").clicked() {
                            self.show_donate = !self.show_donate;
                        }

                        if self.state == TranslationState::Translating {
                            ui.label(
                                RichText::new("正在翻译\u{2026}")
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            );
                        } else {
                            ui.label(
                                RichText::new("Ctrl+C+C 翻译剪贴板内容")
                                    .size(11.0)
                                    .color(TEXT_HINT),
                            );
                        }
                    });
                });
            });
    }

    fn draw_donate_window(&mut self, ctx: &egui::Context) {
        if !self.show_donate { return; }

        egui::Window::new("donate_window")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(280.0, 340.0))
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(32, 32, 38))
                    .rounding(Rounding::same(12.0))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("\u{2764}")
                            .size(28.0)
                            .color(Color32::from_rgb(220, 80, 80)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Buy me a Coffee")
                            .size(16.0)
                            .color(TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("感谢你的支持!")
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.add_space(12.0);

                    ui.add(
                        egui::Image::new(egui::include_image!("../pay.png"))
                            .max_size(Vec2::splat(200.0))
                            .rounding(Rounding::same(8.0)),
                    );

                    ui.add_space(12.0);

                    if ui.add(
                        egui::Button::new(RichText::new("关闭").size(13.0))
                            .rounding(Rounding::same(8.0))
                            .min_size(Vec2::new(80.0, 28.0)),
                    ).clicked() {
                        self.show_donate = false;
                    }
                });
            });
    }

    fn draw_quit_confirm(&mut self, ctx: &egui::Context) {
        if !self.show_quit_confirm { return; }

        egui::Window::new("quit_confirm")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(240.0, 100.0))
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(32, 32, 38))
                    .rounding(Rounding::same(12.0))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::same(20.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("确定要退出 TensorL 吗？")
                            .size(14.0)
                            .color(TEXT_PRIMARY),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() / 2.0 - 90.0);
                        if ui.add(
                            egui::Button::new(RichText::new("取消").size(13.0))
                                .rounding(Rounding::same(8.0))
                                .min_size(Vec2::new(80.0, 28.0)),
                        ).clicked() {
                            self.show_quit_confirm = false;
                        }
                        ui.add_space(8.0);
                        if ui.add(
                            egui::Button::new(
                                RichText::new("退出").size(13.0).color(Color32::WHITE),
                            )
                            .fill(RED_ERROR)
                            .rounding(Rounding::same(8.0))
                            .min_size(Vec2::new(80.0, 28.0)),
                        ).clicked() {
                            std::process::exit(0);
                        }
                    });
                });
            });
    }
}

// ── eframe::App ──────────────────────────────────────────────────────────────

impl eframe::App for TensorLApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();

        // Skip all drawing when minimized — the 0×0 surface causes egui to panic
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::IsIconic;
            if !self.hwnd.is_invalid() && unsafe { IsIconic(self.hwnd) }.as_bool() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
        }

        self.draw_history_panel(ctx);
        self.draw_top_bar(ctx);
        self.draw_status_bar(ctx);
        self.draw_main_panels(ctx);
        self.draw_donate_window(ctx);
        self.draw_quit_confirm(ctx);

        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
    }
}
