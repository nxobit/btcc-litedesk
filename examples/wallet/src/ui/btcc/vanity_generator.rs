use crate::ui::btcc::wallet_list::get_global_vault_password;
use crate::ui::palette;
use btcc_litedesk::{
    db::btcc_wallet::create_encrypted_btcc_wallet_blocking,
    wallet::{
        BitcoinWallet, VanityComputeMode as WalletVanityComputeMode, VanityGenerationResult,
        VanityGpuBackend, VanityGpuConfig, VanityPattern,
        generate_vanity_btcc_wallet_with_stats_cancellable_mode_progress,
    },
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    v_flex,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VanityMode {
    Prefix,
    Suffix,
    PrefixAndSuffix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VanityEngineMode {
    Cpu,
    Gpu,
}

#[derive(Clone, Debug, Default)]
struct BatchSummary {
    target_count: usize,
    processed_count: usize,
    saved_count: usize,
    total_attempts: u64,
}

pub struct VanityGeneratorPage {
    mode: VanityMode,
    engine_mode: VanityEngineMode,
    gpu_backend: VanityGpuBackend,
    prefix_input: Entity<InputState>,
    suffix_input: Entity<InputState>,
    threads_input: Entity<InputState>,
    count_input: Entity<InputState>,
    note_input: Entity<InputState>,
    save_to_wallet_list: bool,
    loading: bool,
    result: Option<VanityGenerationResult>,
    batch_summary: BatchSummary,
    started_at: Option<Instant>,
    elapsed_secs: u64,
    generation_seq: u64,
    stop_requested: Arc<AtomicBool>,
    status: Option<String>,
    error: Option<String>,
    _task: Task<()>,
    _ticker_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl VanityGeneratorPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefix_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("cc1q88")
                .default_value("cc1q")
        });
        let suffix_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("888")
                .default_value("")
        });
        let default_threads = available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .clamp(1, 32);
        let threads_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("4")
                .default_value(&default_threads.to_string())
        });
        let count_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("1")
                .default_value("1")
        });
        let note_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("备注，可选，批量保存时统一写入")
                .default_value("")
        });

        let subscriptions = vec![
            cx.subscribe_in(&prefix_input, window, Self::on_input_event),
            cx.subscribe_in(&suffix_input, window, Self::on_input_event),
            cx.subscribe_in(&threads_input, window, Self::on_input_event),
            cx.subscribe_in(&count_input, window, Self::on_input_event),
            cx.subscribe_in(&note_input, window, Self::on_input_event),
        ];

        Self {
            mode: VanityMode::Prefix,
            engine_mode: VanityEngineMode::Cpu,
            gpu_backend: VanityGpuBackend::Auto,
            prefix_input,
            suffix_input,
            threads_input,
            count_input,
            note_input,
            save_to_wallet_list: true,
            loading: false,
            result: None,
            batch_summary: BatchSummary::default(),
            started_at: None,
            elapsed_secs: 0,
            generation_seq: 0,
            stop_requested: Arc::new(AtomicBool::new(false)),
            status: None,
            error: None,
            _task: Task::ready(()),
            _ticker_task: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn on_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.apply_recommended_engine_mode(cx);
            self.status = None;
            self.error = None;
            cx.notify();
        }
    }

    fn set_mode(&mut self, mode: VanityMode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.apply_recommended_engine_mode(cx);
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn set_engine_mode(&mut self, mode: VanityEngineMode, cx: &mut Context<Self>) {
        self.engine_mode = mode;
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn set_gpu_backend(&mut self, backend: VanityGpuBackend, cx: &mut Context<Self>) {
        self.gpu_backend = backend;
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn set_save_to_wallet_list(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.save_to_wallet_list = enabled;
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn start_generation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }

        let password = if self.save_to_wallet_list {
            match get_global_vault_password() {
                Some(value) => Some(value),
                None => {
                    self.error = Some("请先在钱包列表完成密码登录，再执行自动保存".to_string());
                    self.status = None;
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };

        let prefix = input_value(&self.prefix_input, cx);
        let suffix = input_value(&self.suffix_input, cx);
        let note = input_value(&self.note_input, cx);
        let thread_count = if self.engine_mode == VanityEngineMode::Cpu {
            match self.parse_thread_count(cx) {
                Ok(value) => value,
                Err(err) => {
                    self.error = Some(err.to_string());
                    cx.notify();
                    return;
                }
            }
        } else {
            1
        };
        let target_count = match self.parse_target_count(cx) {
            Ok(value) => value,
            Err(err) => {
                self.error = Some(err.to_string());
                cx.notify();
                return;
            }
        };

        if note.chars().count() > 50 {
            self.error = Some("备注不能超过 50 个字符".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        let mode = self.mode;
        let engine_mode = self.engine_mode;
        let gpu_backend = self.gpu_backend;
        let save_to_wallet_list = self.save_to_wallet_list;
        let pattern = match mode {
            VanityMode::Prefix => VanityPattern::Prefix(prefix.clone()),
            VanityMode::Suffix => VanityPattern::Suffix(suffix.clone()),
            VanityMode::PrefixAndSuffix => VanityPattern::PrefixAndSuffix {
                prefix: prefix.clone(),
                suffix: suffix.clone(),
            },
        };

        self.loading = true;
        self.result = None;
        self.batch_summary = BatchSummary {
            target_count,
            processed_count: 0,
            saved_count: 0,
            total_attempts: 0,
        };
        self.started_at = Some(Instant::now());
        self.elapsed_secs = 0;
        self.generation_seq = self.generation_seq.saturating_add(1);
        self.stop_requested = Arc::new(AtomicBool::new(false));
        self.status = Some(format!("开始生成，目标数量 {}", target_count));
        self.error = None;

        let generation_seq = self.generation_seq;
        let progress_attempts = Arc::new(AtomicU64::new(0));
        let ticker_progress_attempts = progress_attempts.clone();
        self._ticker_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_spawn(async move {
                    thread::sleep(Duration::from_secs(1));
                })
                .await;

                let should_continue = this
                    .update(cx, |this, cx| {
                        if !this.loading || this.generation_seq != generation_seq {
                            return false;
                        }
                        this.elapsed_secs = this
                            .started_at
                            .map(|started_at| started_at.elapsed().as_secs())
                            .unwrap_or(0);
                        if this.engine_mode == VanityEngineMode::Gpu {
                            let attempts = ticker_progress_attempts.load(Ordering::Relaxed);
                            this.status = Some(build_gpu_progress_status(
                                this.batch_summary.processed_count,
                                this.batch_summary.target_count,
                                attempts,
                                this.elapsed_secs,
                                this.save_to_wallet_list,
                            ));
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);

                if !should_continue {
                    break;
                }
            }
        });

        let stop_requested = self.stop_requested.clone();
        let task_progress_attempts = progress_attempts.clone();
        self._task = cx.spawn(async move |this, cx| {
            let mut stopped = false;

            for index in 0..target_count {
                if stop_requested.load(Ordering::Relaxed) {
                    stopped = true;
                    break;
                }

                task_progress_attempts.store(0, Ordering::Relaxed);
                let step_result = cx
                    .background_spawn({
                        let pattern = pattern.clone();
                        let prefix = prefix.clone();
                        let suffix = suffix.clone();
                        let note = note.clone();
                        let password = password.clone();
                        let stop_requested = stop_requested.clone();
                        let progress_attempts = task_progress_attempts.clone();
                        async move {
                            let compute_mode = match engine_mode {
                                VanityEngineMode::Cpu => WalletVanityComputeMode::Cpu,
                                VanityEngineMode::Gpu => {
                                    WalletVanityComputeMode::Gpu(VanityGpuConfig {
                                        backend: gpu_backend,
                                        batch_size: 512 * 1024,
                                    })
                                }
                            };

                            let progress_cb = if engine_mode == VanityEngineMode::Gpu {
                                Some(Arc::new(move |attempts| {
                                    progress_attempts.store(attempts, Ordering::Relaxed);
                                })
                                    as Arc<dyn Fn(u64) + Send + Sync>)
                            } else {
                                None
                            };

                            let Some(result) = generate_vanity_btcc_wallet_with_stats_cancellable_mode_progress(
                                pattern,
                                thread_count,
                                stop_requested,
                                compute_mode,
                                progress_cb,
                            )?
                            else {
                                return Ok::<_, anyhow::Error>(None);
                            };

                            if save_to_wallet_list {
                                let wallet_name = build_wallet_name(mode, &prefix, &suffix, index);
                                create_encrypted_btcc_wallet_blocking(
                                    wallet_name,
                                    result.wallet.address.clone(),
                                    result.wallet.derivation_path.clone(),
                                    if engine_mode == VanityEngineMode::Gpu {
                                        "wif".to_string()
                                    } else {
                                        "generated".to_string()
                                    },
                                    result.wallet.public_key.to_string(),
                                    note,
                                    result.wallet.mnemonic.clone(),
                                    result.wallet.private_key_wif.clone(),
                                    password.expect("password must exist when saving"),
                                )?;
                            }

                            Ok::<_, anyhow::Error>(Some(result))
                        }
                    })
                    .await;

                match step_result {
                    Ok(Some(result)) => {
                        let should_continue = this
                            .update(cx, |this, cx| {
                                if this.generation_seq != generation_seq {
                                    return false;
                                }
                                this.batch_summary.processed_count =
                                    this.batch_summary.processed_count.saturating_add(1);
                                if save_to_wallet_list {
                                    this.batch_summary.saved_count =
                                        this.batch_summary.saved_count.saturating_add(1);
                                }
                                this.batch_summary.total_attempts = this
                                    .batch_summary
                                    .total_attempts
                                    .saturating_add(result.attempts);
                                this.result = Some(result);
                                this.elapsed_secs = this
                                    .started_at
                                    .map(|started_at| started_at.elapsed().as_secs())
                                    .unwrap_or(this.elapsed_secs);
                                this.status = Some(if this.save_to_wallet_list {
                                    format!(
                                        "生成中，已保存 {} / {}",
                                        this.batch_summary.saved_count,
                                        this.batch_summary.target_count
                                    )
                                } else {
                                    format!(
                                        "生成中，已完成 {} / {}",
                                        this.batch_summary.processed_count,
                                        this.batch_summary.target_count
                                    )
                                });
                                this.error = None;
                                cx.notify();
                                !this.stop_requested.load(Ordering::Relaxed)
                                    && this.batch_summary.processed_count
                                        < this.batch_summary.target_count
                            })
                            .unwrap_or(false);

                        if !should_continue {
                            stopped = stop_requested.load(Ordering::Relaxed);
                            break;
                        }
                    }
                    Ok(None) => {
                        stopped = true;
                        break;
                    }
                    Err(err) => {
                        _ = this.update(cx, |this, cx| {
                            if this.generation_seq != generation_seq {
                                return;
                            }
                            this.loading = false;
                            this.started_at = None;
                            this.status = None;
                            this.error = Some(err.to_string());
                            cx.notify();
                        });
                        return;
                    }
                }
            }

            _ = this.update(cx, |this, cx| {
                if this.generation_seq != generation_seq {
                    return;
                }
                this.loading = false;
                this.elapsed_secs = this
                    .started_at
                    .map(|started_at| started_at.elapsed().as_secs())
                    .unwrap_or(this.elapsed_secs);
                this.started_at = None;
                this.status = Some(if stopped {
                    if this.save_to_wallet_list {
                        format!(
                            "已停止，已保存 {} / {}",
                            this.batch_summary.saved_count, this.batch_summary.target_count
                        )
                    } else {
                        format!(
                            "已停止，已完成 {} / {}",
                            this.batch_summary.processed_count, this.batch_summary.target_count
                        )
                    }
                } else if this.save_to_wallet_list {
                    format!(
                        "生成完成，已保存 {} / {}",
                        this.batch_summary.saved_count, this.batch_summary.target_count
                    )
                } else {
                    format!(
                        "生成完成，已完成 {} / {}",
                        this.batch_summary.processed_count, this.batch_summary.target_count
                    )
                });
                this.error = None;
                cx.notify();
            });
        });

        cx.notify();
    }

    fn stop_generation(&mut self, cx: &mut Context<Self>) {
        if !self.loading {
            return;
        }
        self.stop_requested.store(true, Ordering::Relaxed);
        self.generation_seq = self.generation_seq.saturating_add(1);
        self.loading = false;
        self.elapsed_secs = self
            .started_at
            .map(|started_at| started_at.elapsed().as_secs())
            .unwrap_or(self.elapsed_secs);
        self.started_at = None;
        self.status = Some(if self.save_to_wallet_list {
            format!(
                "已停止，已保存 {} / {}",
                self.batch_summary.saved_count, self.batch_summary.target_count
            )
        } else {
            format!(
                "已停止，已完成 {} / {}",
                self.batch_summary.processed_count, self.batch_summary.target_count
            )
        });
        self.error = None;
        cx.notify();
    }

    fn parse_thread_count(&self, cx: &mut Context<Self>) -> anyhow::Result<usize> {
        let value = input_value(&self.threads_input, cx)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("线程数必须是 1 到 64 的整数"))?;
        if !(1..=64).contains(&value) {
            return Err(anyhow::anyhow!("线程数必须在 1 到 64 之间"));
        }
        Ok(value)
    }

    fn parse_target_count(&self, cx: &mut Context<Self>) -> anyhow::Result<usize> {
        let value = input_value(&self.count_input, cx)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("生成个数必须是 1 到 100 的整数"))?;
        if !(1..=100).contains(&value) {
            return Err(anyhow::anyhow!("生成个数必须在 1 到 100 之间"));
        }
        Ok(value)
    }

    fn clear_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.result = None;
        self.status = None;
        self.error = None;
        self.batch_summary = BatchSummary::default();
        self.started_at = None;
        self.elapsed_secs = 0;
        self.stop_requested.store(false, Ordering::Relaxed);
        self.prefix_input
            .update(cx, |input, cx| input.set_value("cc1q", window, cx));
        self.suffix_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.threads_input
            .update(cx, |input, cx| input.set_value("4", window, cx));
        self.count_input
            .update(cx, |input, cx| input.set_value("1", window, cx));
        self.note_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.save_to_wallet_list = true;
        self.engine_mode = VanityEngineMode::Cpu;
        self.gpu_backend = VanityGpuBackend::Auto;
        self.apply_recommended_engine_mode(cx);
        cx.notify();
    }

    fn apply_recommended_engine_mode(&mut self, cx: &mut Context<Self>) {
        let rule_len = self.current_rule_length(cx);
        self.engine_mode = if rule_len >= 4 {
            VanityEngineMode::Gpu
        } else {
            VanityEngineMode::Cpu
        };
    }

    fn current_rule_length(&self, cx: &mut Context<Self>) -> usize {
        let prefix_len = normalize_match_fragment(&input_value(&self.prefix_input, cx), true)
            .chars()
            .count();
        let suffix_len = normalize_match_fragment(&input_value(&self.suffix_input, cx), false)
            .chars()
            .count();

        match self.mode {
            VanityMode::Prefix => prefix_len,
            VanityMode::Suffix => suffix_len,
            VanityMode::PrefixAndSuffix => prefix_len + suffix_len,
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let threads = input_value(&self.threads_input, cx)
            .parse::<usize>()
            .ok()
            .unwrap_or(0);
        let target_count = input_value(&self.count_input, cx)
            .parse::<usize>()
            .ok()
            .unwrap_or(0);
        let prefix_len = input_value(&self.prefix_input, cx).chars().count();
        let suffix_len = input_value(&self.suffix_input, cx).chars().count();

        v_flex()
            .w_full()
            .gap_2()
            .p_4()
            .rounded(px(10.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.background)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("BTCC 靓号生成"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("btcc-vanity-generate")
                                    .label(if self.loading {
                                        "生成中..."
                                    } else {
                                        "开始生成"
                                    })
                                    .primary()
                                    .small()
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_generation(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("btcc-vanity-stop")
                                    .label("停止")
                                    .outline()
                                    .small()
                                    .disabled(!self.loading)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.stop_generation(cx);
                                    })),
                            )
                            .child(
                                Button::new("btcc-vanity-clear")
                                    .label("重置")
                                    .outline()
                                    .small()
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.clear_result(window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(palette::muted(&app_theme))
                    .child("按前缀或后缀规则批量生成 BTCC 靓号；GPU 模式依赖 vgen，并按 WIF 保存结果。"),
            )
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .flex_1()
                            .children([
                                compact_chip("模式", self.mode.label().to_string(), app_theme.primary, cx),
                                compact_chip("线程", threads.to_string(), app_theme.success, cx),
                                compact_chip("目标数", target_count.to_string(), app_theme.warning, cx),
                                compact_chip(
                                    "已生成",
                                    self.batch_summary.processed_count.to_string(),
                                    app_theme.info,
                                    cx,
                                ),
                                compact_chip(
                                    "已保存",
                                    self.batch_summary.saved_count.to_string(),
                                    app_theme.primary,
                                    cx,
                                ),
                                compact_chip(
                                    "已耗时",
                                    format_elapsed(self.elapsed_secs),
                                    app_theme.warning,
                                    cx,
                                ),
                                compact_chip(
                                    "总尝试",
                                    if self.batch_summary.total_attempts == 0 {
                                        "--".to_string()
                                    } else {
                                        self.batch_summary.total_attempts.to_string()
                                    },
                                    app_theme.danger,
                                    cx,
                                ),
                                compact_chip("前缀长度", prefix_len.to_string(), app_theme.primary, cx),
                                compact_chip("后缀长度", suffix_len.to_string(), app_theme.success, cx),
                            ]),
                    )
                    .child(
                        v_flex()
                            .w(px(336.))
                            .h(px(60.))
                            .justify_between()
                            .gap_1()
                            .px_3()
                            .py_2()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(app_theme.warning.opacity(0.22))
                            .bg(app_theme.warning.opacity(0.05))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_start()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .gap_0()
                                            .child(
                                                div()
                                                    .text_size(px(10.))
                                                    .text_color(palette::muted(&app_theme))
                                                    .child("打赏地址"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .line_height(px(16.))
                                                    .font_family("monospace")
                                                    .text_color(palette::text_strong(&app_theme))
                                                    .child("cc1quykauu7j98q6xe2af893cf024tdgx3pm\n4c9g39"),
                                            ),
                                    )
                                    .child(
                                        Clipboard::new("btcc-vanity-copy-donate")
                                            .value("cc1quykauu7j98q6xe2af893cf024tdgx3pm4c9g39"),
                                    ),
                            ),
                    ),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(inline_error(error, cx))
            })
            .when_some(self.status.clone(), |el, status| {
                el.child(inline_status(status, cx))
            })
            .into_any_element()
    }

    fn render_left_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let prefix_enabled = matches!(self.mode, VanityMode::Prefix | VanityMode::PrefixAndSuffix);
        let suffix_enabled = matches!(self.mode, VanityMode::Suffix | VanityMode::PrefixAndSuffix);
        let gpu_mode = self.engine_mode == VanityEngineMode::Gpu;

        v_flex()
            .w(px(380.))
            .h_full()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .gap_2()
                    .p_4()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("生成参数"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(mode_button(self.mode, VanityMode::Prefix, cx))
                            .child(mode_button(self.mode, VanityMode::Suffix, cx))
                            .child(mode_button(self.mode, VanityMode::PrefixAndSuffix, cx)),
                    )
                    .child(render_engine_picker(
                        self.engine_mode,
                        self.gpu_backend,
                        cx,
                    ))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .children([
                                field(
                                    "地址前缀",
                                    self.prefix_input.clone(),
                                    "例: cc1q88",
                                    prefix_enabled,
                                    cx,
                                ),
                                field(
                                    "地址后缀",
                                    self.suffix_input.clone(),
                                    "例: 888",
                                    suffix_enabled,
                                    cx,
                                ),
                            ]),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .children([
                                field(
                                    "并发线程",
                                    self.threads_input.clone(),
                                    if gpu_mode { "GPU 固定为 1" } else { "1 到 64" },
                                    !gpu_mode,
                                    cx,
                                ),
                                field(
                                    "生成个数",
                                    self.count_input.clone(),
                                    "1 到 100",
                                    true,
                                    cx,
                                ),
                            ]),
                    )
                    .child(field(
                        "统一备注",
                        self.note_input.clone(),
                        "",
                        true,
                        cx,
                    ))
                    .child(div().flex_1())
                    .child(self.render_save_switch(cx)),
            )
            .into_any_element()
    }

    fn render_save_switch(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        v_flex()
            .gap_2()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(palette::muted(&app_theme))
                    .child("保存到钱包列表"),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_2()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border_soft(&app_theme))
                    .bg(app_theme.muted.opacity(0.04))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(palette::text_strong(&app_theme))
                                .child(if self.save_to_wallet_list {
                                    "开启"
                                } else {
                                    "关闭"
                                }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(palette::muted_soft(&app_theme))
                                    .child(if self.save_to_wallet_list {
                                        "命中后立即加密并写入钱包列表"
                                    } else {
                                        "仅生成结果，不写入钱包列表"
                                    }),
                            ),
                    )
                    .child(render_switch(
                        self.save_to_wallet_list,
                        self.loading,
                        cx.listener(|this, _, _, cx| {
                            this.set_save_to_wallet_list(!this.save_to_wallet_list, cx);
                        }),
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_right_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        v_flex()
            .flex_1()
            .h_full()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap_3()
                    .p_4()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("最新结果"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scrollbar()
                            .child(match &self.result {
                            Some(result) => {
                                render_wallet_result(result.wallet.clone(), result.attempts, cx)
                            }
                            None => div()
                                .text_size(px(12.))
                                .text_color(palette::muted(&app_theme))
                                .child(
                                    "生成成功后，这里显示地址、派生路径、WIF 私钥和助记词。",
                                )
                                .into_any_element(),
                        }),
                    ),
            )
            .into_any_element()
    }

}

impl Render for VanityGeneratorPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .overflow_hidden()
            .gap_3()
            .child(self.render_header(cx))
            .child(
                h_flex()
                    .flex_1()
                    .gap_3()
                    .overflow_hidden()
                    .child(self.render_left_panel(cx))
                    .child(self.render_right_panel(cx)),
            )
    }
}

impl VanityMode {
    fn label(self) -> &'static str {
        match self {
            VanityMode::Prefix => "前缀",
            VanityMode::Suffix => "后缀",
            VanityMode::PrefixAndSuffix => "前后缀",
        }
    }
}

fn mode_button(
    active_mode: VanityMode,
    mode: VanityMode,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let active = active_mode == mode;
    let app_theme = cx.theme().clone();

    Button::new(match mode {
        VanityMode::Prefix => "btcc-vanity-mode-prefix",
        VanityMode::Suffix => "btcc-vanity-mode-suffix",
        VanityMode::PrefixAndSuffix => "btcc-vanity-mode-prefix-suffix",
    })
    .label(mode.label())
    .when(active, |button| button.primary())
    .when(!active, |button| button.outline())
    .small()
    .text_color(if active {
        app_theme.background
    } else {
        palette::text_strong(&app_theme)
    })
    .on_click(cx.listener(move |this, _, _, cx| {
        this.set_mode(mode, cx);
    }))
    .into_any_element()
}

fn render_engine_picker(
    engine_mode: VanityEngineMode,
    gpu_backend: VanityGpuBackend,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();

    v_flex()
        .gap_2()
        .child(
            div()
                .text_size(px(11.))
                .text_color(palette::muted(&app_theme))
                .child("生成引擎"),
        )
        .child(
            h_flex()
                .gap_2()
                .child(engine_mode_button(engine_mode, VanityEngineMode::Gpu, cx))
                .child(engine_mode_button(engine_mode, VanityEngineMode::Cpu, cx)),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(palette::muted_soft(&app_theme))
                .child(if engine_mode == VanityEngineMode::Gpu {
                    "GPU 结果仅包含地址和 WIF。"
                } else {
                    "CPU 使用内置多线程搜索。"
                }),
        )
        .when(engine_mode == VanityEngineMode::Gpu, |panel| {
            panel
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette::muted(&app_theme))
                        .child("GPU 后端"),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(gpu_backend_button(gpu_backend, VanityGpuBackend::Auto, cx))
                        .child(gpu_backend_button(gpu_backend, VanityGpuBackend::Vulkan, cx))
                        .child(gpu_backend_button(gpu_backend, VanityGpuBackend::Metal, cx))
                        .child(gpu_backend_button(gpu_backend, VanityGpuBackend::Dx12, cx))
                        .child(gpu_backend_button(gpu_backend, VanityGpuBackend::Gl, cx)),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(palette::muted_soft(&app_theme))
                        .child("默认建议 Auto；驱动异常时再手动切后端。"),
                )
        })
        .into_any_element()
}

fn engine_mode_button(
    active_mode: VanityEngineMode,
    mode: VanityEngineMode,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let active = active_mode == mode;
    let app_theme = cx.theme().clone();

    Button::new(match mode {
        VanityEngineMode::Cpu => "btcc-vanity-engine-cpu",
        VanityEngineMode::Gpu => "btcc-vanity-engine-gpu",
    })
    .label(match mode {
        VanityEngineMode::Cpu => "CPU",
        VanityEngineMode::Gpu => "GPU",
    })
    .when(active, |button| button.primary())
    .when(!active, |button| button.outline())
    .small()
    .text_color(if active {
        app_theme.background
    } else {
        palette::text_strong(&app_theme)
    })
    .on_click(cx.listener(move |this, _, _, cx| {
        this.set_engine_mode(mode, cx);
    }))
    .into_any_element()
}

fn gpu_backend_button(
    active_backend: VanityGpuBackend,
    backend: VanityGpuBackend,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let active = active_backend == backend;
    let app_theme = cx.theme().clone();

    Button::new(match backend {
        VanityGpuBackend::Auto => "btcc-vanity-gpu-auto",
        VanityGpuBackend::Vulkan => "btcc-vanity-gpu-vulkan",
        VanityGpuBackend::Metal => "btcc-vanity-gpu-metal",
        VanityGpuBackend::Dx12 => "btcc-vanity-gpu-dx12",
        VanityGpuBackend::Gl => "btcc-vanity-gpu-gl",
    })
    .label(backend.label())
    .when(active, |button| button.primary())
    .when(!active, |button| button.outline())
    .xsmall()
    .text_color(if active {
        app_theme.background
    } else {
        palette::text_strong(&app_theme)
    })
    .on_click(cx.listener(move |this, _, _, cx| {
        this.set_gpu_backend(backend, cx);
    }))
    .into_any_element()
}

fn render_switch(
    checked: bool,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    let track = if checked {
        app_theme.primary
    } else {
        app_theme.muted.opacity(0.45)
    };

    Button::new("btcc-vanity-save-switch")
        .ghost()
        .small()
        .disabled(disabled)
        .on_click(on_click)
        .child(
            div()
                .w(px(42.))
                .h(px(24.))
                .rounded_full()
                .bg(track)
                .px_1()
                .flex()
                .items_center()
                .justify_start()
                .when(checked, |el| el.justify_end())
                .child(
                    div()
                        .w(px(18.))
                        .h(px(18.))
                        .rounded_full()
                        .bg(gpui::white())
                        .shadow_sm(),
                ),
        )
        .into_any_element()
}

fn field(
    label: &'static str,
    input: Entity<InputState>,
    placeholder_hint: &'static str,
    enabled: bool,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .flex_1()
        .min_w(px(0.))
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .text_color(palette::muted(&app_theme))
                .child(label),
        )
        .child(Input::new(&input).w_full().small().disabled(!enabled))
        .when(!placeholder_hint.is_empty(), |el| {
            el.child(
                div()
                    .text_size(px(10.))
                    .text_color(palette::muted_soft(&app_theme))
                    .child(placeholder_hint),
            )
        })
        .into_any_element()
}

fn render_wallet_result(
    wallet: BitcoinWallet,
    attempts: u64,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    let mnemonic_value = wallet.mnemonic.clone();
    let has_mnemonic = !mnemonic_value.trim().is_empty();

    v_flex()
        .w_full()
        .gap_2()
        .child(compact_kv("本次尝试", attempts.to_string(), false, cx))
        .child(compact_kv("BTCC 地址", wallet.address, true, cx))
        .child(compact_kv("派生路径", wallet.derivation_path, true, cx))
        .child(compact_kv("WIF 私钥", wallet.private_key_wif, true, cx))
        .child(if has_mnemonic {
            v_flex()
                .w_full()
                .gap_1()
                .p_3()
                .rounded(px(8.))
                .border_1()
                .border_color(app_theme.danger.opacity(0.18))
                .bg(app_theme.danger.opacity(0.04))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(palette::muted(&app_theme))
                                .child("助记词"),
                        )
                        .child(
                            Clipboard::new("btcc-vanity-copy-mnemonic")
                                .value(mnemonic_value.clone()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(20.))
                        .font_family("monospace")
                        .text_color(palette::text_strong(&app_theme))
                        .child(wallet.mnemonic),
                )
                .into_any_element()
        } else {
            v_flex()
                .w_full()
                .gap_1()
                .p_3()
                .rounded(px(8.))
                .border_1()
                .border_color(app_theme.warning.opacity(0.18))
                .bg(app_theme.warning.opacity(0.05))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette::muted(&app_theme))
                        .child("助记词"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(palette::text_strong(&app_theme))
                        .child("当前结果为 GPU 生成的 WIF 钱包，不包含助记词。"),
                )
                .into_any_element()
        })
        .into_any_element()
}

fn compact_kv(
    label: &'static str,
    value: String,
    copyable: bool,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .w_full()
        .gap_1()
        .p_3()
        .rounded(px(8.))
        .border_1()
        .border_color(palette::border(&app_theme))
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette::muted(&app_theme))
                        .child(label),
                )
                .when(copyable, |el| {
                    el.child(
                        Clipboard::new(("btcc-vanity-copy-field", hash_id(label)))
                            .value(value.clone()),
                    )
                }),
        )
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(18.))
                .font_family("monospace")
                .text_color(palette::text_strong(&app_theme))
                .child(value),
        )
        .into_any_element()
}

fn compact_chip(
    label: &'static str,
    value: String,
    accent: Hsla,
    cx: &mut Context<VanityGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .min_w(px(88.))
        .gap_1()
        .px_3()
        .py_2()
        .rounded(px(8.))
        .border_1()
        .border_color(accent.opacity(0.18))
        .bg(accent.opacity(0.08))
        .child(
            div()
                .text_size(px(10.))
                .text_color(palette::muted(&app_theme))
                .child(label),
        )
        .child(
            div()
                .text_size(px(14.))
                .font_semibold()
                .font_family("monospace")
                .text_color(accent)
                .child(value),
        )
        .into_any_element()
}

fn inline_error(text: String, cx: &mut Context<VanityGeneratorPage>) -> AnyElement {
    div()
        .text_size(px(11.))
        .text_color(cx.theme().danger)
        .child(text)
        .into_any_element()
}

fn inline_status(text: String, cx: &mut Context<VanityGeneratorPage>) -> AnyElement {
    div()
        .text_size(px(11.))
        .text_color(cx.theme().primary)
        .child(text)
        .into_any_element()
}

fn input_value(input: &Entity<InputState>, cx: &mut Context<VanityGeneratorPage>) -> String {
    input.read_with(cx, |input, _| input.value().trim().to_string())
}

fn hash_id(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn build_wallet_name(mode: VanityMode, prefix: &str, suffix: &str, index: usize) -> String {
    let base = default_wallet_name(mode, prefix, suffix);
    if index == 0 {
        return base;
    }

    let suffix_number = (index + 1).to_string();
    let max_base_len = 7usize.saturating_sub(suffix_number.chars().count());
    let trimmed_base: String = base.chars().take(max_base_len.max(1)).collect();
    format!("{trimmed_base}{suffix_number}")
}

fn default_wallet_name(mode: VanityMode, prefix: &str, suffix: &str) -> String {
    let prefix_match = normalize_match_fragment(prefix, true);
    let suffix_match = normalize_match_fragment(suffix, false);
    let combined = match mode {
        VanityMode::Prefix => prefix_match,
        VanityMode::Suffix => suffix_match,
        VanityMode::PrefixAndSuffix => format!("{prefix_match}{suffix_match}"),
    };

    let trimmed = combined.trim();
    if trimmed.is_empty() {
        return "靓号钱包".to_string();
    }

    trimmed.chars().take(7).collect()
}

fn normalize_match_fragment(value: &str, strip_prefix: bool) -> String {
    let trimmed = value.trim();
    let stripped = if strip_prefix {
        trimmed
            .strip_prefix("cc1q")
            .or_else(|| trimmed.strip_prefix("CC1Q"))
            .or_else(|| trimmed.strip_prefix("cc1"))
            .or_else(|| trimmed.strip_prefix("CC1"))
            .unwrap_or(trimmed)
    } else {
        trimmed
    };

    stripped
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn format_elapsed(elapsed_secs: u64) -> String {
    let hours = elapsed_secs / 3600;
    let minutes = (elapsed_secs % 3600) / 60;
    let seconds = elapsed_secs % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn build_gpu_progress_status(
    processed_count: usize,
    target_count: usize,
    attempts: u64,
    elapsed_secs: u64,
    saving_enabled: bool,
) -> String {
    let action = if saving_enabled { "已保存" } else { "已完成" };
    if attempts == 0 {
        return format!(
            "GPU 启动中，{} {} / {}，已耗时 {}",
            action,
            processed_count,
            target_count,
            format_elapsed(elapsed_secs)
        );
    }

    let rate = if elapsed_secs == 0 {
        attempts
    } else {
        attempts / elapsed_secs.max(1)
    };

    format!(
        "GPU 生成中，{} {} / {}，已尝试 {}，约 {} 次/秒，已耗时 {}",
        action,
        processed_count,
        target_count,
        format_count(attempts),
        format_count(rate),
        format_elapsed(elapsed_secs)
    )
}

fn format_count(value: u64) -> String {
    let raw = value.to_string();
    let chars = raw.chars().rev().collect::<Vec<_>>();
    let mut formatted = String::with_capacity(raw.len() + raw.len() / 3);

    for (index, ch) in chars.into_iter().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }

    formatted.chars().rev().collect()
}
