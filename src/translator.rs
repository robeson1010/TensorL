use std::num::NonZeroU32;
use std::sync::mpsc;
use std::time::Duration;

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
};

use crate::config::{AppConfig, Backend, Language};

// ── Message types ────────────────────────────────────────────────────────────

pub enum InferRequest {
    Translate {
        text:   String,
        source: Language,
        target: Language,
    },
    Abort,
    Reload(AppConfig),
}

pub enum UiMsg {
    HotkeyFired,
    TrayToggle,
    TrayQuit,
    ModelLoaded,
    ModelLoadProgress { percent: f32, stage: String },
    ModelError(String),
    Token(String),
    TranslationDone,
    TranslationError(String),
    GpuAvailable(bool),
}

// ── Inference thread ─────────────────────────────────────────────────────────

pub fn spawn_inference_thread(
    config: AppConfig,
    infer_rx: mpsc::Receiver<InferRequest>,
    ui_tx: mpsc::Sender<UiMsg>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("tensorl-inference".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || run_inference_loop(config, infer_rx, ui_tx))
        .expect("failed to spawn inference thread")
}

fn run_inference_loop(
    mut config: AppConfig,
    infer_rx: mpsc::Receiver<InferRequest>,
    ui_tx: mpsc::Sender<UiMsg>,
) {
    // Detect GPU at build time
    let gpu_available = cfg!(any(feature = "cuda", feature = "vulkan"));
    let _ = ui_tx.send(UiMsg::GpuAvailable(gpu_available));

    let backend = match LlamaBackend::init() {
        Ok(b) => b,
        Err(e) => {
            let _ = ui_tx.send(UiMsg::ModelError(format!("Backend init failed: {e}")));
            return;
        }
    };

    // Initial model load (if path is configured)
    let mut model_opt = if config.model_path.as_os_str().is_empty()
        || !config.model_path.exists()
    {
        let _ = ui_tx.send(UiMsg::ModelError("No model file selected.".into()));
        None
    } else {
        load_model_with_progress(&backend, &config, &ui_tx)
    };

    loop {
        match infer_rx.recv() {
            Err(_) => break,

            Ok(InferRequest::Abort) => { /* no-op: token loop already stopped */ }

            Ok(InferRequest::Reload(new_cfg)) => {
                config = new_cfg;
                if config.model_path.exists() {
                    model_opt = load_model_with_progress(&backend, &config, &ui_tx);
                } else {
                    let _ = ui_tx.send(UiMsg::ModelError("Model file not found.".into()));
                    model_opt = None;
                }
            }

            Ok(InferRequest::Translate { text, source, target }) => {
                match &model_opt {
                    Some(model) => {
                        run_translation(model, &backend, &config, &text, source, target, &ui_tx);
                    }
                    None => {
                        let _ = ui_tx.send(UiMsg::TranslationError("Model not loaded.".into()));
                    }
                }
            }
        }
    }
}

// ── Model loading ─────────────────────────────────────────────────────────────

fn load_model_with_progress(
    backend: &LlamaBackend,
    config: &AppConfig,
    ui_tx: &mpsc::Sender<UiMsg>,
) -> Option<LlamaModel> {
    // Fake progress on a side thread
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let tx_prog = ui_tx.clone();
    std::thread::spawn(move || {
        let mut p = 0.0f32;
        loop {
            match done_rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }
            p = (p + 0.025).min(0.90);
            let _ = tx_prog.send(UiMsg::ModelLoadProgress {
                percent: p,
                stage: "Loading model weights…".into(),
            });
            std::thread::sleep(Duration::from_millis(200));
        }
    });

    let mut model_params = LlamaModelParams::default();
    if config.backend == Backend::Gpu {
        model_params = model_params.with_n_gpu_layers(config.n_gpu_layers as u32);
    }

    let result = LlamaModel::load_from_file(backend, &config.model_path, &model_params);
    let _ = done_tx.send(()); // signal progress thread to stop

    match result {
        Ok(model) => {
            let _ = ui_tx.send(UiMsg::ModelLoadProgress {
                percent: 1.0,
                stage: "Model ready".into(),
            });
            let _ = ui_tx.send(UiMsg::ModelLoaded);
            Some(model)
        }
        Err(e) => {
            let _ = ui_tx.send(UiMsg::ModelError(format!("Model load failed: {e}")));
            None
        }
    }
}

// ── Translation ───────────────────────────────────────────────────────────────

fn run_translation(
    model: &LlamaModel,
    backend: &LlamaBackend,
    config: &AppConfig,
    text: &str,
    source: Language,
    target: Language,
    ui_tx: &mpsc::Sender<UiMsg>,
) {
    let prompt = build_prompt(text, source, target);

    let n_ctx = NonZeroU32::new(config.n_ctx.max(512)).unwrap();
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_threads(config.n_threads as i32);

    let mut ctx = match model.new_context(backend, ctx_params) {
        Ok(c) => c,
        Err(e) => {
            let _ = ui_tx.send(UiMsg::TranslationError(format!("Context error: {e}")));
            return;
        }
    };

    let tokens = match model.str_to_token(&prompt, AddBos::Always) {
        Ok(t) => t,
        Err(e) => {
            let _ = ui_tx.send(UiMsg::TranslationError(format!("Tokenize error: {e}")));
            return;
        }
    };

    if tokens.is_empty() {
        let _ = ui_tx.send(UiMsg::TranslationDone);
        return;
    }

    // Prefill batch
    let mut batch = LlamaBatch::new(tokens.len().max(512), 1);
    for (i, &tok) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        if let Err(e) = batch.add(tok, i as i32, &[0], is_last) {
            let _ = ui_tx.send(UiMsg::TranslationError(format!("Batch add: {e}")));
            return;
        }
    }

    if let Err(e) = ctx.decode(&mut batch) {
        let _ = ui_tx.send(UiMsg::TranslationError(format!("Prefill decode: {e}")));
        return;
    }

    // Build sampler chain (HY-MT1.5 recommended params)
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::top_k(20),
        LlamaSampler::top_p(0.6, 1),
        LlamaSampler::temp(0.7),
        LlamaSampler::penalties(0, 1.05, 0.0, 0.0),
        LlamaSampler::dist(42),
    ]);

    let mut n_cur = tokens.len() as i32;
    let n_max = n_cur + 2048;
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    while n_cur < n_max {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if model.is_eog_token(token) {
            break;
        }

        let piece = model
            .token_to_piece(token, &mut decoder, false, None)
            .unwrap_or_default();

        if piece.contains("<|im_") {
            break;
        }

        if ui_tx.send(UiMsg::Token(piece)).is_err() {
            break;
        }

        batch.clear();
        if let Err(e) = batch.add(token, n_cur, &[0], true) {
            let _ = ui_tx.send(UiMsg::TranslationError(format!("Decode batch: {e}")));
            break;
        }
        if let Err(e) = ctx.decode(&mut batch) {
            let _ = ui_tx.send(UiMsg::TranslationError(format!("Decode: {e}")));
            break;
        }

        n_cur += 1;
    }

    let _ = ui_tx.send(UiMsg::TranslationDone);
    // Context drops here, releasing KV cache memory
}

// ── Prompt builder ───────────────────────────────────────────────────────────

pub fn build_prompt(text: &str, source: Language, target: Language) -> String {
    let use_chinese = source.is_chinese() || target.is_chinese();

    if use_chinese {
        let tgt = target.hy_mt_zh_name();
        format!(
            "<|im_start|>user\n将以下文本翻译为{tgt}，注意只需要输出翻译后的结果，不要额外解释：\n\n{text}<|im_end|>\n<|im_start|>assistant\n"
        )
    } else {
        let tgt = target.hy_mt_en_name();
        format!(
            "<|im_start|>user\nTranslate the following segment into {tgt}, without additional explanation.\n\n{text}<|im_end|>\n<|im_start|>assistant\n"
        )
    }
}
