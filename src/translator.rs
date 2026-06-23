use std::sync::mpsc;
use std::time::Duration;

use crate::config::Language;

// ── Message types ────────────────────────────────────────────────────────────

pub enum InferRequest {
    Translate {
        text:   String,
        source: Language,
        target: Language,
    },
}

pub enum UiMsg {
    HotkeyFired,
    Token(String),
    TranslationDone,
    TranslationError(String),
    DetectedLanguage(Language),
}

// ── Translation thread ─────────────────────────────────────────────────────────

pub fn spawn_inference_thread(
    infer_rx: mpsc::Receiver<InferRequest>,
    ui_tx: mpsc::Sender<UiMsg>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("tensorl-translate".into())
        .spawn(move || run_translate_loop(infer_rx, ui_tx))
        .expect("failed to spawn translate thread")
}

fn run_translate_loop(
    infer_rx: mpsc::Receiver<InferRequest>,
    ui_tx: mpsc::Sender<UiMsg>,
) {
    loop {
        match infer_rx.recv() {
            Err(_) => break, // channel closed → app shutting down

            Ok(InferRequest::Translate { text, source, target }) => {
                match google_translate(&text, source, target) {
                    Ok((result, detected)) => {
                        if let Some(lang) = detected {
                            let _ = ui_tx.send(UiMsg::DetectedLanguage(lang));
                        }
                        let _ = ui_tx.send(UiMsg::Token(result));
                        let _ = ui_tx.send(UiMsg::TranslationDone);
                    }
                    Err(e) => {
                        let _ = ui_tx.send(UiMsg::TranslationError(e));
                    }
                }
            }
        }
    }
}

// ── Google Translate ────────────────────────────────────────────────────────

/// Call Google's translate backend — the same `translate_a/single` endpoint the
/// translate.google.com web page uses. No API key required. Returns the full
/// translated text, or a Chinese error message on failure.
fn google_translate(text: &str, source: Language, target: Language) -> Result<(String, Option<Language>), String> {
    let src = source.google_code();
    let tgt = target.google_target_code();

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();

    let resp = agent
        .get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", src)
        .query("tl", tgt)
        .query("dt", "t")
        .query("q", text)
        .call()
        .map_err(|e| format!("网络请求失败: {e}"))?;

    let body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败: {e}"))?;

    parse_response(&body, source)
}

/// Google returns a nested JSON array. The first element (`v[0]`) is an array of
/// translated segments; each segment's first element (`seg[0]`) is the
/// translated text. `v[2]` holds the detected source language code when
/// `sl=auto`. Concatenate segments into the full result.
fn parse_response(body: &str, source: Language) -> Result<(String, Option<Language>), String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("解析响应失败: {e}"))?;

    let segments = v
        .get(0)
        .and_then(|x| x.as_array())
        .ok_or_else(|| "响应格式异常".to_string())?;

    let mut out = String::new();
    for seg in segments {
        if let Some(s) = seg.get(0).and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }

    let detected = if source == Language::Auto {
        v.get(2)
            .and_then(|x| x.as_str())
            .and_then(Language::from_google_code)
    } else {
        None
    };

    Ok((out, detected))
}
