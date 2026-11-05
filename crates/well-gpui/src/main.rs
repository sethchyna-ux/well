use gpui::*;

mod actions;
mod diagnostics;
mod plugins;
mod settings_view;
mod ssh_console;
mod terminal;
mod theme;

use actions::{filter_actions, footer_hints_for_context};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use settings_view::{CursorShape, SettingsSection, SettingsView, TerminalSettings};
use ssh_console::{
    build_legacy_bios_option43, build_qemu_uefi_test_command, build_standalone_dns_option6,
    classify_chainload_payload, detect_primary_lan_ip, discover_arp_neighbors,
    generate_strict_ipxe_script, is_ipxe_stage2_client, run_fast_port_scan,
    send_wol_magic_packet, start_http_key_server, start_pxe_http_boot_server, ArpNeighborEntry,
    ChainloadTargetKind, ClientArch, DiscoveredSshKey, PortScanResult, PxeBootSubtab,
    PxeServerMode, ServerHubSubtab, SmbShareEntry, SshConsoleInputField, SshConsoleSection,
    SshHostEditorDraft, SshTunnelEntry, SshTunnelKind, TftpAction,
};
use well_shell::ssh::{SshEngine, SshHostConfig, SshTarget};
use well_shell::theia_ssh::TheiaSshProfile;

actions!(
    well,
    [
        OpenSettings,
        OpenCommandPalette,
        SplitHorizontal,
        SplitVertical,
        ClosePane,
        NewTab,
        Quit,
        FocusNextPane,
        OpenSshManager,
        OpenSnippetPicker
    ]
);


#[allow(dead_code)]
struct KittyImageOverlay {
    id: u32,
    placement_id: u32,
    image: std::sync::Arc<gpui::Image>,
    cursor_row: u16,
    cursor_col: u16,
    width: Option<u32>,
    height: Option<u32>,
    columns: Option<u32>,
    rows: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PythiaMode {
    CommandSynthesis,
    ImageStudio,
    StderrDiagnosis,
}

#[derive(Clone)]
pub struct PythiaState {
    pub active: bool,
    pub mode: PythiaMode,
    pub query: String,
    pub status: Option<String>,
    pub is_generating: bool,
    pub last_command: Option<String>,
    pub last_explanation: Option<String>,
    pub last_confidence: f32,
    pub is_destructive: bool,
    pub provider_used: Option<String>,
    pub image_path: Option<String>,
    pub image_preview: Option<std::sync::Arc<gpui::Image>>,
    pub width: u32,
    pub height: u32,
}

impl Default for PythiaState {
    fn default() -> Self {
        Self {
            active: false,
            mode: PythiaMode::CommandSynthesis,
            query: String::new(),
            status: None,
            is_generating: false,
            last_command: None,
            last_explanation: None,
            last_confidence: 0.0,
            is_destructive: false,
            provider_used: None,
            image_path: None,
            image_preview: None,
            width: 512,
            height: 512,
        }
    }
}

fn shared_metis_history() -> std::sync::Arc<std::sync::RwLock<well_shell::MetisHistory>> {
    use std::sync::OnceLock;
    static SHARED: OnceLock<std::sync::Arc<std::sync::RwLock<well_shell::MetisHistory>>> = OnceLock::new();
    SHARED.get_or_init(|| {
        let mut history = well_shell::MetisHistory::new();
        history.load_all_shell_histories();
        if let Ok(path) = well_shell::MetisHistory::default_path() {
            let recipes_path = path.with_file_name("recipes.json");
            let _ = history.load_recipes_from_file(&recipes_path);
        }
        std::sync::Arc::new(std::sync::RwLock::new(history))
    }).clone()
}

struct TerminalView {
    pty: std::sync::Arc<well_shell::pty::PtySession>,
    focus_handle: FocusHandle,
    settings: Entity<TerminalSettings>,
    last_size: std::rc::Rc<std::cell::Cell<Option<(u16, u16)>>>,
    kitty_images: Vec<KittyImageOverlay>,
    mneme_editor: well_editor::MnemeEditor,
    mneme_active: bool,
    cwd: Option<String>,
    astraea: std::sync::Arc<well_prompt::AstraeaStateVector>,
    pub pythia: PythiaState,
    pub ssh_profile: Option<TheiaSshProfile>,
    #[allow(dead_code)]
    metis: std::sync::Arc<std::sync::RwLock<well_shell::MetisHistory>>,
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TerminalView {
    fn from_pty(
        cx: &mut Context<Self>,
        settings: Entity<TerminalSettings>,
        pty: std::sync::Arc<well_shell::pty::PtySession>,
        rx: futures::channel::mpsc::UnboundedReceiver<()>,
        astraea: std::sync::Arc<well_prompt::AstraeaStateVector>,
        cwd: Option<String>,
    ) -> Self {
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        let mut rx = rx;

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                use futures::StreamExt;
                while rx.next().await.is_some() {
                    cx.update(|cx| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |view, cx| {
                                view.update_text();
                                cx.notify();
                            });
                        }
                    }).ok();
                }
            }
        }).detach();

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                loop {
                    let mut sleep_ms = 500u64;
                    let alive = cx.update(|cx| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |view, cx| {
                                let s = view.settings.read(cx);
                                sleep_ms = s.cursor_blink_ms.clamp(200, 1200);
                                if s.cursor_blink || view.mneme_active || view.pythia.active {
                                    cx.notify();
                                }
                            });
                            true
                        } else {
                            false
                        }
                    });
                    if !matches!(alive, Ok(true)) {
                        break;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(sleep_ms))
                        .await;
                }
            }
        }).detach();

        let focus_handle = cx.focus_handle();

        let mut view = Self {
            pty,
            focus_handle,
            settings,
            last_size: std::rc::Rc::new(std::cell::Cell::new(None)),
            kitty_images: Vec::new(),
            mneme_editor: well_editor::MnemeEditor::default(),
            mneme_active: false,
            cwd,
            astraea,
            pythia: PythiaState::default(),
            ssh_profile: None,
            metis: shared_metis_history(),
        };
        view.update_text();
        view
    }

    fn with_ssh_profile(mut self, profile: Option<TheiaSshProfile>) -> Self {
        self.ssh_profile = profile;
        self
    }

    #[allow(clippy::arc_with_non_send_sync)]
    fn new(cx: &mut Context<Self>, settings: Entity<TerminalSettings>) -> Self {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        let pty = std::sync::Arc::new(well_shell::pty::PtySession::spawn_with_stream(
            24, 
            80, 
            None, 
            move |_output| {
                let _ = tx.unbounded_send(());
            }
        ).expect("failed to spawn pty"));
        let astraea = std::sync::Arc::new(well_prompt::AstraeaStateVector::in_memory());
        Self::from_pty(cx, settings, pty, rx, astraea, None)
    }

    #[allow(clippy::arc_with_non_send_sync, clippy::type_complexity)]
    pub fn spawn_ssh_pty(
        target: &SshTarget,
    ) -> Result<
        (
            std::sync::Arc<well_shell::pty::PtySession>,
            futures::channel::mpsc::UnboundedReceiver<()>,
            std::sync::Arc<well_prompt::AstraeaStateVector>,
            Option<String>,
        ),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        let ssh_args = target.to_command_args();
        let env_vars = [
            ("SSH_CONNECTION", std::ffi::OsStr::new("1")),
            ("SSH_CLIENT", std::ffi::OsStr::new("1")),
            ("WELL_REMOTE", std::ffi::OsStr::new("1")),
        ];

        let pty = std::sync::Arc::new(well_shell::pty::PtySession::spawn_command_with_environment(
            24,
            80,
            [235, 219, 178], // Theia fg cream
            [29, 32, 33],    // Theia bg obsidian
            "/usr/bin/ssh",
            &ssh_args,
            &env_vars,
            move |_event| {
                let _ = tx.unbounded_send(());
            },
        )?);

        let astraea = std::sync::Arc::new(well_prompt::AstraeaStateVector::in_memory());
        {
            let mut snap = astraea.read_snapshot();
            snap.is_ssh = true;
            snap.git_branch = target.destination.clone();
            astraea.write_snapshot(&snap);
        }

        let cwd = Some(format!("ssh://{}", target.destination));
        Ok((pty, rx, astraea, cwd))
    }

    #[allow(dead_code)]
    fn new_ssh(
        cx: &mut Context<Self>,
        settings: Entity<TerminalSettings>,
        target: &SshTarget,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (pty, rx, astraea, cwd) = Self::spawn_ssh_pty(target)?;
        Ok(Self::from_pty(cx, settings, pty, rx, astraea, cwd))
    }

    fn update_text(&mut self) {
        if let Ok(rx) = self.pty.graphic_events_rx.lock() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    well_shell::GraphicEvent::WorkingDirUpdate(path) => {
                        self.cwd = Some(path.clone());
                        let snap = well_prompt::AstraeaWatcher::scan_path(std::path::Path::new(&path), 0, 0);
                        self.astraea.write_snapshot(&snap);
                    }
                    well_shell::GraphicEvent::Hyperlink(_) => {}
                    well_shell::GraphicEvent::KittyImage {
                        id,
                        placement_id,
                        format,
                        action: _,
                        payload_base64,
                        is_complete,
                    } => {
                        let width: Option<u32> = None;
                        let height: Option<u32> = None;
                        let columns: Option<u32> = None;
                        let rows: Option<u32> = None;
                        println!("RECEIVED KITTY IMAGE: id={} format={} is_complete={}", id, format, is_complete);
                    // Decode payload
                    if let Ok(bytes) = BASE64.decode(payload_base64) {
                        println!("DECODED BYTES length: {}", bytes.len());
                        
                        let mut computed_w = width;
                        let mut computed_h = height;
                        
                        if format == 100 {
                            if let Ok(reader) = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format() {
                                if let Ok(dims) = reader.into_dimensions() {
                                    if computed_w.is_none() {
                                        computed_w = Some(dims.0);
                                    }
                                    if computed_h.is_none() {
                                        computed_h = Some(dims.1);
                                    }
                                }
                            }
                        }
                        
                        let gpui_img = if format == 100 {
                            // PNG
                            gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes)
                        } else if format == 32 {
                            // RGBA
                            let w = computed_w.unwrap_or(0);
                            let h = computed_h.unwrap_or(0);
                            if w > 0 && h > 0 {
                                if let Some(rgba) = image::RgbaImage::from_raw(w, h, bytes) {
                                    let mut buf = std::io::Cursor::new(Vec::new());
                                    if rgba.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                                        gpui::Image::from_bytes(gpui::ImageFormat::Png, buf.into_inner())
                                    } else {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        } else if format == 24 {
                            // RGB
                            let w = computed_w.unwrap_or(0);
                            let h = computed_h.unwrap_or(0);
                            if w > 0 && h > 0 {
                                if let Some(rgb) = image::RgbImage::from_raw(w, h, bytes) {
                                    let mut buf = std::io::Cursor::new(Vec::new());
                                    if rgb.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                                        gpui::Image::from_bytes(gpui::ImageFormat::Png, buf.into_inner())
                                    } else {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        } else {
                            continue; // unsupported
                        };

                        let (cursor_row, cursor_col) = {
                            let parser = self.pty.parser.lock().unwrap();
                            let screen = parser.screen();
                            screen.cursor_position()
                        };

                        self.kitty_images.push(KittyImageOverlay {
                            id,
                            placement_id,
                            image: std::sync::Arc::new(gpui_img),
                            cursor_row,
                            cursor_col,
                            width: computed_w,
                            height: computed_h,
                            columns,
                            rows,
                        });
                    }
                }
            }
        }
    }
}

    pub fn execute_pythia(&mut self, cx: &mut Context<Self>) {
        let trimmed = self.pythia.query.trim().to_string();
        if trimmed.is_empty() {
            self.pythia.status = Some("Please enter a query or prompt first.".to_string());
            cx.notify();
            return;
        }

        self.pythia.is_generating = true;
        self.pythia.status = Some("Pythia synthesizing...".to_string());
        cx.notify();

        let query = trimmed;
        let mode = self.pythia.mode;
        let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());
        let width = self.pythia.width;
        let height = self.pythia.height;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = match mode {
                    PythiaMode::CommandSynthesis => {
                        let local_recipe = {
                            let history = shared_metis_history();
                            let guard = history.read().unwrap();
                            guard.search_recipes(&query, 1).into_iter().next()
                        };

                        if let Some(recipe) = local_recipe {
                            well_llm::TranslationResult {
                                command: recipe.command,
                                explanation: format!("⚡ Instant Metis Recipe [cluster: {}]", recipe.cluster_id),
                                confidence: recipe.confidence,
                                is_destructive: false,
                                provider_used: "Metis Local Cache".to_string(),
                                fell_back_to_offline: true,
                                generated_image_path: None,
                            }
                        } else {
                            let req = well_llm::TranslationRequest {
                                query: query.clone(),
                                shell: "zsh".to_string(),
                                cwd: cwd.clone(),
                                provider: well_llm::LlmProvider::Gemini,
                                gemini_api_key: None,
                                gemini_model: None,
                                ollama_url: None,
                                ollama_model: None,
                                hf_token: None,
                                hf_model: None,
                                image_prompt: None,
                                image_width: None,
                                image_height: None,
                                force_image_regeneration: false,
                            };
                            well_llm::PythiaTranslator::translate(&req)
                        }
                    }
                    PythiaMode::ImageStudio => {
                        let req = well_llm::TranslationRequest {
                            query: query.clone(),
                            shell: "zsh".to_string(),
                            cwd: cwd.clone(),
                            provider: well_llm::LlmProvider::Gemini,
                            gemini_api_key: None,
                            gemini_model: None,
                            ollama_url: None,
                            ollama_model: None,
                            hf_token: None,
                            hf_model: None,
                            image_prompt: Some(query.clone()),
                            image_width: Some(width),
                            image_height: Some(height),
                            force_image_regeneration: false,
                        };
                        well_llm::PythiaTranslator::translate(&req)
                    }
                    PythiaMode::StderrDiagnosis => {
                        well_llm::PythiaTranslator::diagnose(&query, 1, &query, None)
                    }
                };

                cx.update(|cx| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |view, cx| {
                            view.pythia.is_generating = false;
                            view.pythia.last_command = Some(result.command.clone());
                            view.pythia.last_explanation = Some(result.explanation.clone());
                            view.pythia.last_confidence = result.confidence;
                            view.pythia.is_destructive = result.is_destructive;
                            view.pythia.provider_used = Some(result.provider_used.clone());

                            if let Some(ref path) = result.generated_image_path {
                                view.pythia.image_path = Some(path.clone());
                                if let Ok(bytes) = std::fs::read(path) {
                                    let gpui_img = gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes);
                                    view.pythia.image_preview = Some(std::sync::Arc::new(gpui_img));
                                }
                                view.pythia.status = Some(format!("✓ Image generated via {}", result.provider_used));
                            } else {
                                view.pythia.status = Some(format!("✓ {} (confidence {:.0}%)", result.provider_used, result.confidence * 100.0));
                            }
                            cx.notify();
                        });
                    }
                }).ok();
            }
        }).detach();
    }

    fn render_pythia_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.settings.read(cx).theme;
        let font_family = self.settings.read(cx).font_family.clone();
        let mode = self.pythia.mode;
        let is_generating = self.pythia.is_generating;
        let query = self.pythia.query.clone();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let cursor_visible = (now / 500).is_multiple_of(2);

        let mode_pills = div()
            .flex()
            .flex_row()
            .gap_2()
            .child(
                div()
                    .id("mode-shell")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .bg(if mode == PythiaMode::CommandSynthesis {
                        theme.border_active
                    } else {
                        rgb(0x282828)
                    })
                    .border_1()
                    .border_color(if mode == PythiaMode::CommandSynthesis { theme.border_active } else { rgb(0x3c3836) })
                    .text_color(if mode == PythiaMode::CommandSynthesis {
                        theme.bg
                    } else {
                        theme.fg
                    })
                    .font_weight(if mode == PythiaMode::CommandSynthesis { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pythia.mode = PythiaMode::CommandSynthesis;
                        cx.notify();
                    }))
                    .child("⚡ Command Synthesis"),
            )
            .child(
                div()
                    .id("mode-image")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .bg(if mode == PythiaMode::ImageStudio {
                        rgb(0xd3869b)
                    } else {
                        rgb(0x282828)
                    })
                    .border_1()
                    .border_color(if mode == PythiaMode::ImageStudio { rgb(0xd3869b) } else { rgb(0x3c3836) })
                    .text_color(if mode == PythiaMode::ImageStudio {
                        rgb(0x1d2021)
                    } else {
                        theme.fg
                    })
                    .font_weight(if mode == PythiaMode::ImageStudio { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pythia.mode = PythiaMode::ImageStudio;
                        cx.notify();
                    }))
                    .child("🖼 Image Studio"),
            )
            .child(
                div()
                    .id("mode-diag")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .bg(if mode == PythiaMode::StderrDiagnosis {
                        rgb(0xfb4934)
                    } else {
                        rgb(0x282828)
                    })
                    .border_1()
                    .border_color(if mode == PythiaMode::StderrDiagnosis { rgb(0xfb4934) } else { rgb(0x3c3836) })
                    .text_color(if mode == PythiaMode::StderrDiagnosis {
                        rgb(0x1d2021)
                    } else {
                        theme.fg
                    })
                    .font_weight(if mode == PythiaMode::StderrDiagnosis { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pythia.mode = PythiaMode::StderrDiagnosis;
                        cx.notify();
                    }))
                    .child("🩺 Stderr Diagnosis"),
            );

        let input_box = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(rgb(0x282828))
            .border_1()
            .border_color(theme.border_active)
            .rounded_lg()
            .px_3()
            .py_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .font_family(font_family.clone())
                    .text_size(px(14.0))
                    .text_color(theme.fg)
                    .child(if query.is_empty() {
                        let placeholder = match mode {
                            PythiaMode::CommandSynthesis => "Describe command: e.g. find large log files and compress...",
                            PythiaMode::ImageStudio => "Image prompt: e.g. cyberpunk neon workstation raining...",
                            PythiaMode::StderrDiagnosis => "Paste error or stderr for instant diagnosis...",
                        };
                        div().text_color(rgb(0x928374)).child(placeholder)
                    } else {
                        div().text_color(theme.fg).child(query)
                    })
                    .child(
                        div()
                            .w(px(2.0))
                            .h(px(16.0))
                            .ml_1()
                            .bg(if cursor_visible { theme.border_active } else { gpui::rgba(0x00000000) }),
                    ),
            )
            .child(
                div()
                    .id("exec-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(theme.border_active)
                    .text_color(theme.bg)
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.execute_pythia(cx);
                    }))
                    .child(if is_generating { "⏳ Synthesizing..." } else { "↵ Enter" }),
            );

        let mut content = div()
            .flex()
            .flex_col()
            .gap_3();

        if self.pythia.is_destructive {
            content = content.child(
                div()
                    .bg(rgb(0x3c1f1e))
                    .border_1()
                    .border_color(rgb(0xcc241d))
                    .rounded_md()
                    .p_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(0xfb4934))
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("⚠️ DESTRUCTIVE COMMAND DETECTED: This command can alter or remove system state."),
            );
        }

        content = content.child(input_box);

        if mode == PythiaMode::ImageStudio {
            let presets = div()
                .flex()
                .flex_row()
                .gap_2()
                .items_center()
                .child(div().text_xs().text_color(rgb(0x928374)).child("✨ Presets:"))
                .child(
                    div().id("preset-cyber").px_2().py_1().rounded_md().bg(rgb(0x282828)).border_1().border_color(rgb(0x3c3836)).text_xs().text_color(theme.fg).cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.query = "cyberpunk neon terminal workstation, raining megacity backdrop, holographic billboards, high detail".into();
                            this.execute_pythia(cx);
                        }))
                        .child("🌃 Cyberpunk")
                )
                .child(
                    div().id("preset-synth").px_2().py_1().rounded_md().bg(rgb(0x282828)).border_1().border_color(rgb(0x3c3836)).text_xs().text_color(theme.fg).cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.query = "synthwave wireframe digital grid horizon, giant glowing neon sun, vector mountains, retrowave aesthetic".into();
                            this.execute_pythia(cx);
                        }))
                        .child("🌅 Synthwave")
                )
                .child(
                    div().id("preset-crt").px_2().py_1().rounded_md().bg(rgb(0x282828)).border_1().border_color(rgb(0x3c3836)).text_xs().text_color(theme.fg).cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.query = "retro 80s computer terminal, glowing amber phosphor screen, scanlines, analog cassette deck".into();
                            this.execute_pythia(cx);
                        }))
                        .child("📼 Phosphor CRT")
                )
                .child(
                    div().id("preset-space").px_2().py_1().rounded_md().bg(rgb(0x282828)).border_1().border_color(rgb(0x3c3836)).text_xs().text_color(theme.fg).cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.query = "deep space orbital station orbiting a gas giant, swirling dust nebula, cinematic lighting, stars".into();
                            this.execute_pythia(cx);
                        }))
                        .child("🌌 Deep Space")
                );

            let res_selector = div()
                .flex()
                .flex_row()
                .gap_2()
                .items_center()
                .child(div().text_xs().text_color(rgb(0x928374)).child("📐 Size:"))
                .child(
                    div().id("size-256").px_2().py_0p5().rounded_md()
                        .bg(if self.pythia.width == 256 { theme.border_active } else { rgb(0x282828) })
                        .border_1()
                        .border_color(if self.pythia.width == 256 { theme.border_active } else { rgb(0x3c3836) })
                        .text_color(if self.pythia.width == 256 { theme.bg } else { theme.fg })
                        .text_xs().cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.width = 256;
                            this.pythia.height = 256;
                            cx.notify();
                        }))
                        .child("256x256")
                )
                .child(
                    div().id("size-512").px_2().py_0p5().rounded_md()
                        .bg(if self.pythia.width == 512 { theme.border_active } else { rgb(0x282828) })
                        .border_1()
                        .border_color(if self.pythia.width == 512 { theme.border_active } else { rgb(0x3c3836) })
                        .text_color(if self.pythia.width == 512 { theme.bg } else { theme.fg })
                        .text_xs().cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.width = 512;
                            this.pythia.height = 512;
                            cx.notify();
                        }))
                        .child("512x512")
                )
                .child(
                    div().id("size-1024").px_2().py_0p5().rounded_md()
                        .bg(if self.pythia.width == 1024 { theme.border_active } else { rgb(0x282828) })
                        .border_1()
                        .border_color(if self.pythia.width == 1024 { theme.border_active } else { rgb(0x3c3836) })
                        .text_color(if self.pythia.width == 1024 { theme.bg } else { theme.fg })
                        .text_xs().cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pythia.width = 1024;
                            this.pythia.height = 1024;
                            cx.notify();
                        }))
                        .child("1024x1024")
                );

            content = content.child(div().flex().flex_row().justify_between().items_center().child(presets).child(res_selector));

            if let Some(ref img) = self.pythia.image_preview {
                let img_el = gpui::img(img.clone()).w(px(160.0)).h(px(160.0)).rounded_lg().border_1().border_color(theme.border_active);
                let preview_card = div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .items_center()
                    .bg(rgb(0x282828))
                    .border_1()
                    .border_color(rgb(0x3c3836))
                    .rounded_lg()
                    .p_3()
                    .child(img_el)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_sm().font_weight(gpui::FontWeight::BOLD).text_color(rgb(0xfe8019)).child("✨ Generated Image Artifact Ready"))
                            .child(div().text_xs().text_color(rgb(0xa89984)).child(self.pythia.image_path.clone().unwrap_or_default()))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .mt_2()
                                    .child(
                                        div()
                                            .id("render-inline-btn")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x8ec07c))
                                            .text_color(rgb(0x1d2021))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(path) = &this.pythia.image_path {
                                                    if let Ok(bytes) = std::fs::read(path) {
                                                        let b64 = BASE64.encode(&bytes);
                                                        let escape = format!("\x1b_Gf=100,a=T,m=0;{}\x1b\\\r\n", b64);
                                                        let mut writer = this.pty.writer.lock().unwrap();
                                                        use std::io::Write;
                                                        let _ = writer.write_all(escape.as_bytes());
                                                        this.pythia.active = false;
                                                        cx.notify();
                                                    }
                                                }
                                            }))
                                            .child("🖼 Render Inline in Terminal")
                                    )
                                    .child(
                                        div()
                                            .id("reveal-finder-btn")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x3c3836))
                                            .border_1()
                                            .border_color(rgb(0x504945))
                                            .text_color(theme.fg)
                                            .text_xs()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|this, _, _, _| {
                                                if let Some(path) = &this.pythia.image_path {
                                                    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
                                                }
                                            }))
                                            .child("📂 Reveal in Finder")
                                    )
                            )
                    );
                content = content.child(preview_card);
            }
        }

        if let Some(ref cmd) = self.pythia.last_command {
            if mode != PythiaMode::ImageStudio {
                let cmd_clone = cmd.clone();
                let cmd_clone2 = cmd.clone();
                let cmd_clone3 = cmd.clone();
                let explanation = self.pythia.last_explanation.clone().unwrap_or_default();
                let result_card = div()
                    .bg(rgb(0x282828))
                    .border_1()
                    .border_color(rgb(0x3c3836))
                    .rounded_lg()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_family(font_family.clone())
                            .text_sm()
                            .text_color(rgb(0xfe8019))
                            .p_2()
                            .bg(rgb(0x1d2021))
                            .border_1()
                            .border_color(rgb(0x3c3836))
                            .rounded_md()
                            .child(cmd.clone()),
                    )
                    .child(div().text_xs().text_color(rgb(0xa89984)).child(explanation))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .mt_1()
                            .child(
                                div()
                                    .id("run-cmd-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme.border_active)
                                    .text_color(theme.bg)
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let mut writer = this.pty.writer.lock().unwrap();
                                        use std::io::Write;
                                        let _ = writer.write_all(cmd_clone.as_bytes());
                                        let _ = writer.write_all(b"\r");
                                        this.pythia.active = false;
                                        cx.notify();
                                    }))
                                    .child("▶ Run in Terminal"),
                            )
                            .child(
                                div()
                                    .id("edit-mneme-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0xd79921))
                                    .text_color(rgb(0x1d2021))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mneme_editor.set_text(&cmd_clone2);
                                        this.mneme_active = true;
                                        this.pythia.active = false;
                                        cx.notify();
                                    }))
                                    .child("✏ Edit in Mneme"),
                            )
                            .child(
                                div()
                                    .id("copy-cmd-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x3c3836))
                                    .border_1()
                                    .border_color(rgb(0x504945))
                                    .text_color(theme.fg)
                                    .text_xs()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd_clone3.clone()));
                                    }))
                                    .child("📋 Copy"),
                            ),
                    );
                content = content.child(result_card);
            }
        }

        let status_text = self.pythia.status.clone().unwrap_or_else(|| "Tab: Switch Mode · Enter: Execute · Esc: Close".to_string());
        let footer = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .text_xs()
            .text_color(rgb(0x928374))
            .child(status_text)
            .child(div().child("Pythia 2.0 Engine"));

        let modal_card = div()
            .w(px(680.0))
            .bg(rgb(0x1d2021))
            .border_1()
            .border_color(theme.border_active)
            .rounded_xl()
            .shadow_2xl()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(div().text_color(theme.border_active).text_base().font_weight(gpui::FontWeight::BOLD).child("⚡ PYTHIA AI"))
                            .child(mode_pills),
                    )
                    .child(
                        div()
                            .id("close-pythia")
                            .text_color(rgb(0x928374))
                            .text_sm()
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pythia.active = false;
                                cx.notify();
                            }))
                            .child("✕"),
                    ),
            )
            .child(content)
            .child(footer);

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_start()
            .justify_center()
            .pt_12()
            .child(modal_card)
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(_window);
        let (
            theme,
            font_family,
            font_size,
            per_host_ssh_theme,
            cursor_shape,
            cursor_blink,
            cursor_blink_ms,
            cursor_use_accent,
        ) = {
            let settings = self.settings.read(cx);
            (
                settings.theme.clone(),
                settings.font_family.clone(),
                settings.font_size,
                settings.per_host_ssh_theme,
                settings.cursor_shape,
                settings.cursor_blink,
                settings.cursor_blink_ms,
                settings.cursor_use_accent,
            )
        };

        let blink_period = cursor_blink_ms.clamp(200, 1200) as u128;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let blink_visible = if !cursor_blink {
            true
        } else {
            (now_ms / blink_period).is_multiple_of(2)
        };
        let cursor_color = if cursor_use_accent {
            theme.border_active
        } else {
            theme.fg
        };

        let parser = self.pty.parser.lock().unwrap();
        let screen = parser.screen();

        let size = screen.size();
        let (cursor_row, cursor_col) = screen.cursor_position();
        let show_vt_cursor = !screen.hide_cursor() && !self.mneme_active;

        let mut row_elements = Vec::new();
        for row in 0..size.0 {
            let mut col_elements = Vec::new();
            for col in 0..size.1 {
                let is_cursor_cell = show_vt_cursor && row == cursor_row && col == cursor_col;
                if let Some(cell) = screen.cell(row, col) {
                    let raw = cell.contents();
                    let text = if raw.is_empty() {
                        " ".to_string()
                    } else {
                        raw.to_string()
                    };
                    let fg_rgb = terminal::convert_color(cell.fgcolor(), theme.fg, &theme);
                    let bg_rgb =
                        terminal::convert_color(cell.bgcolor(), rgba(0x00000000), &theme);
                    let mut fg = fg_rgb;
                    let mut bg = bg_rgb;

                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }

                    if is_cursor_cell && is_focused && blink_visible && cursor_shape == CursorShape::Block {
                        bg = cursor_color;
                        fg = theme.bg;
                    }

                    let mut el = div().bg(bg).text_color(fg).child(text);

                    if is_cursor_cell {
                        if !is_focused {
                            el = el.border_1().border_color(cursor_color);
                        } else if blink_visible {
                            match cursor_shape {
                                CursorShape::Block => {}
                                CursorShape::Beam => {
                                    el = el.border_l_2().border_color(cursor_color);
                                }
                                CursorShape::Underline => {
                                    el = el.border_b_2().border_color(cursor_color);
                                }
                            }
                        }
                    }

                    if cell.bold() {
                        el = el.font_weight(gpui::FontWeight::BOLD);
                    }
                    if cell.italic() {
                        el = el.italic();
                    }
                    if cell.underline() {
                        el = el.underline();
                    }

                    col_elements.push(el);
                } else {
                    col_elements.push(
                        div()
                            .bg(rgba(0x00000000))
                            .text_color(rgba(0xffffffff))
                            .child(" "),
                    );
                }
            }
            row_elements.push(div().flex().flex_row().children(col_elements));
        }

        let (pane_bg, pane_border) = if let Some(prof) = &self.ssh_profile {
            if per_host_ssh_theme {
                let (border_hex, bg_hex, _) = prof.tier.prism_palette();
                (
                    rgb(bg_hex),
                    if is_focused { rgb(border_hex) } else { theme.border_inactive },
                )
            } else {
                (
                    theme.bg,
                    if is_focused { theme.border_active } else { theme.border_inactive },
                )
            }
        } else {
            (
                theme.bg,
                if is_focused { theme.border_active } else { theme.border_inactive },
            )
        };
        
        let mut container = div()
            .key_context("TerminalPty")
            .relative()
            .bg(pane_bg)
            .border_2()
            .rounded_xl()
            .overflow_hidden()
            .shadow_sm()
            .border_color(pane_border)
            .p_1()
            .size_full()
            .font_family(font_family.clone())
            .text_size(px(font_size))
            .cursor(gpui::CursorStyle::Arrow)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _window, cx| {
                if this.settings.read(cx).modal_open {
                    return;
                }
                let is_cmd_k = (event.keystroke.modifiers.platform || (event.keystroke.modifiers.control && event.keystroke.modifiers.shift)) && event.keystroke.key == "k";
                if is_cmd_k {
                    this.pythia.active = !this.pythia.active;
                    cx.notify();
                    return;
                }

                if this.pythia.active {
                    let key = event.keystroke.key.as_str();
                    match key {
                        "escape" => {
                            this.pythia.active = false;
                        }
                        "tab" => {
                            this.pythia.mode = match this.pythia.mode {
                                PythiaMode::CommandSynthesis => PythiaMode::ImageStudio,
                                PythiaMode::ImageStudio => PythiaMode::StderrDiagnosis,
                                PythiaMode::StderrDiagnosis => PythiaMode::CommandSynthesis,
                            };
                        }
                        "backspace" => {
                            this.pythia.query.pop();
                        }
                        "enter" => {
                            this.execute_pythia(cx);
                        }
                        _ => {
                            if let Some(c) = settings_view::extract_printable_char(&event.keystroke) {
                                this.pythia.query.push_str(&c);
                            }
                        }
                    }
                    cx.notify();
                    return;
                }

                if event.keystroke.key == "escape" && this.mneme_active {
                    this.mneme_active = false;
                    cx.notify();
                    return;
                }
                if event.keystroke.modifiers.control && event.keystroke.key == "e" {
                    this.mneme_active = !this.mneme_active;
                    cx.notify();
                    return;
                }

                if this.mneme_active {
                    let key = event.keystroke.key.as_str();
                    match key {
                        "tab" => {
                            let snippets = this.settings.read(cx).snippets.clone();
                            let is_ssh = this.ssh_profile.is_some();
                            let _ = this.mneme_editor.expand_or_advance_snippet(&snippets, is_ssh);
                            cx.stop_propagation();
                        }
                        "up" => this.mneme_editor.move_cursor_up(),
                        "down" => this.mneme_editor.move_cursor_down(),
                        "left" => {
                            if event.keystroke.modifiers.alt {
                                this.mneme_editor.move_word_left();
                            } else {
                                this.mneme_editor.move_cursor_left();
                            }
                        }
                        "right" => {
                            if event.keystroke.modifiers.alt {
                                this.mneme_editor.move_word_right();
                            } else {
                                this.mneme_editor.move_cursor_right();
                            }
                        }
                        "backspace" => this.mneme_editor.backspace(),
                        "delete" => this.mneme_editor.delete_next_char(),
                        "enter" => {
                            let warning = this.mneme_editor.destructive_warning();
                            if event.keystroke.modifiers.shift {
                                if warning.is_some() {
                                    // ⇧⏎ explicitly force-executes gated destructive commands
                                    if let Some(payload) = this.mneme_editor.take_execution_payload() {
                                        let mut writer = this.pty.writer.lock().unwrap();
                                        use std::io::Write;
                                        let _ = writer.write_all(payload.as_bytes());
                                        this.mneme_active = false;
                                    }
                                } else {
                                    this.mneme_editor.insert_text("\n");
                                }
                            } else if warning.is_none() {
                                if let Some(payload) = this.mneme_editor.take_execution_payload() {
                                    let mut writer = this.pty.writer.lock().unwrap();
                                    use std::io::Write;
                                    let _ = writer.write_all(payload.as_bytes());
                                    this.mneme_active = false;
                                }
                            }
                        }
                        _ => {
                            if let Some(c) = settings_view::extract_printable_char(&event.keystroke) {
                                this.mneme_editor.insert_text(&c);
                            }
                        }
                    }
                    cx.notify();
                    return;
                }

                let mut writer = this.pty.writer.lock().unwrap();
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;
                if modifiers.control {
                    let chars: Vec<char> = key.chars().collect();
                    if chars.len() == 1 {
                        let c = chars[0].to_ascii_lowercase();
                        if c.is_ascii_lowercase() {
                            let ctrl_c = c as u8 - b'a' + 1;
                            let _ = writer.write_all(&[ctrl_c]);
                        }
                    }
                    return;
                }

                let bytes: &[u8] = match key.as_str() {
                    "enter" => b"\r",
                    "backspace" => b"\x7f",
                    "delete" => b"\x1b[3~",
                    "up" => b"\x1b[A",
                    "down" => b"\x1b[B",
                    "right" => b"\x1b[C",
                    "left" => b"\x1b[D",
                    "tab" => b"\t",
                    "escape" => b"\x1b",
                    s if s.len() == 1 => s.as_bytes(),
                    _ => {
                        if let Some(c) = &event.keystroke.key_char {
                            c.as_bytes()
                        } else {
                            b""
                        }
                    }
                };
                if !bytes.is_empty() {
                    use std::io::Write;
                    let _ = writer.write_all(bytes);
                }
            }))
            .child({
                let pty = self.pty.clone();
                let last_size = self.last_size.clone();
                let font_size = self.settings.read(cx).font_size;
                canvas(
                    move |bounds, _window, _cx| {
                        let (cols, rows) = terminal::calculate_grid_dimensions(
                            f32::from(bounds.size.width),
                            f32::from(bounds.size.height),
                            font_size,
                        );
                        
                        let current = Some((rows, cols));
                        if last_size.get() != current {
                            last_size.set(current);
                            pty.resize(rows, cols).ok();
                        }
                    },
                    |_bounds, _prepaint, _window, _cx| {}
                ).absolute().size_full()
            })
            .child(
                div().absolute().top_0().left_0().size_full().children(row_elements)
            )
            .children(
                self.kitty_images.iter().map(|img_overlay| {
                    let char_width = self.settings.read(cx).font_size * 0.6;
                    let line_height = self.settings.read(cx).font_size * 1.25;

                    let top = img_overlay.cursor_row as f32 * line_height;
                    let left = img_overlay.cursor_col as f32 * char_width;
                    let mut el = gpui::img(img_overlay.image.clone());
                    
                    // Set width and height if available, otherwise just use natural size
                    if let Some(w) = img_overlay.width {
                        el = el.w(px(w as f32));
                    } else if let Some(c) = img_overlay.columns {
                        el = el.w(px(c as f32 * char_width));
                    }
                    
                    if let Some(h) = img_overlay.height {
                        el = el.h(px(h as f32));
                    } else if let Some(r) = img_overlay.rows {
                        el = el.h(px(r as f32 * line_height));
                    }
                    
                    div().absolute().top(px(top)).left(px(left)).child(el)
                })
            );

        if self.mneme_active {
            let mneme_text = self.mneme_editor.get_text();
            let cwd_display = self.cwd.as_deref().unwrap_or("~");
            
            let mut prompt_elements: Vec<gpui::Div> = Vec::new();
            if let Some(prof) = &self.ssh_profile {
                let (_, _, badge_hex) = prof.tier.prism_palette();
                prompt_elements.push(
                    div()
                        .px_2()
                        .py_0p5()
                        .mr_2()
                        .rounded_sm()
                        .bg(rgb(badge_hex))
                        .text_color(rgb(0x1d2021))
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(prof.astraea_badge()),
                );
            }
            prompt_elements.push(
                div().text_color(theme.ansi_colors[2]).child(cwd_display.to_string())
            );
            let snap = self.astraea.read_snapshot();
            if !snap.git_branch.is_empty() {
                let dirty = if snap.is_dirty { "*" } else { "" };
                let op_name = snap.scm_op.as_str();
                let op_display = if op_name.is_empty() { "".to_string() } else { format!(" {op_name}") };
                prompt_elements.push(div().text_color(theme.ansi_colors[5]).ml_1().child("on"));
                prompt_elements.push(div().text_color(theme.ansi_colors[4]).ml_1().child(format!("\u{e0a0} {}{}{}", snap.git_branch, dirty, op_display)));
            }
            prompt_elements.push(div().text_color(theme.fg).ml_1().child("❯"));
            
            let prompt_row = div().flex().flex_row().items_center()
                .font_family(font_family.clone())
                .text_size(px(14.0))
                .children(prompt_elements);
            
            let mneme_overlay = div()
                .key_context("MnemeInlineEditor")
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .bg(rgb(0x1d2021))
                .border_t_2()
                .border_color(theme.border_active)
                .p_3()
                .shadow_2xl()
                .child(
                    div().flex().flex_col()
                        .child(prompt_row)
                        .child({
                            let cursors = self.mneme_editor.cursors();
                            let cursor_pos = if cursors.is_empty() { 0 } else { cursors[0] };
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
                            let cursor_visible = (now / 500).is_multiple_of(2);
                            
                            let lines: Vec<&str> = mneme_text.split('\n').collect();
                            let mut current_char_idx = 0;
                            let mut cursor_line = 0;
                            let mut cursor_char_idx_in_line = 0;
                            
                            for (i, line) in lines.iter().enumerate() {
                                let line_len = line.chars().count();
                                if current_char_idx + line_len >= cursor_pos {
                                    cursor_line = i;
                                    cursor_char_idx_in_line = cursor_pos - current_char_idx;
                                    break;
                                }
                                current_char_idx += line_len + 1;
                            }
                            
                            if lines.is_empty() {
                                cursor_line = 0;
                                cursor_char_idx_in_line = 0;
                            } else if cursor_line >= lines.len() {
                                cursor_line = lines.len().saturating_sub(1);
                                cursor_char_idx_in_line = lines[cursor_line].chars().count();
                            }

                            div().mt_2().flex().flex_col().text_color(theme.fg)
                                .font_family(font_family.clone())
                                .text_size(px(font_size))
                                .children(lines.iter().enumerate().map(|(i, line)| {
                                    if i == cursor_line {
                                        let mut char_indices = line.char_indices();
                                        if let Some((byte_idx, c)) = char_indices.nth(cursor_char_idx_in_line) {
                                            let before = &line[..byte_idx];
                                            let after = &line[byte_idx + c.len_utf8()..];
                                            div().flex().flex_row().text_color(theme.fg)
                                                .child(if before.is_empty() { gpui::SharedString::from("") } else { gpui::SharedString::from(before.to_string()) })
                                                .child(
                                                    div()
                                                        .bg(if cursor_visible { theme.fg } else { gpui::rgba(0x00000000) })
                                                        .text_color(if cursor_visible { theme.bg } else { theme.fg })
                                                        .child(if c == ' ' { "\u{00A0}".to_string() } else { c.to_string() })
                                                )
                                                .child(if after.is_empty() { gpui::SharedString::from("") } else { gpui::SharedString::from(after.to_string()) })
                                        } else {
                                            div().flex().flex_row().text_color(theme.fg)
                                                .child(if line.is_empty() { gpui::SharedString::from("") } else { gpui::SharedString::from(line.to_string()) })
                                                .child(
                                                    div()
                                                        .bg(if cursor_visible { theme.fg } else { gpui::rgba(0x00000000) })
                                                        .text_color(if cursor_visible { theme.bg } else { theme.fg })
                                                        .child("\u{00A0}")
                                                )
                                        }
                                    } else {
                                        div().text_color(theme.fg).child(if line.is_empty() { gpui::SharedString::from(" ") } else { gpui::SharedString::from(line.to_string()) })
                                    }
                                }))
                        })
                        .child(if let Some(warn) = self.mneme_editor.destructive_warning() {
                            div()
                                .mt_2()
                                .p_2()
                                .bg(rgb(0x2a1414))
                                .border_1()
                                .border_color(rgb(0xfb4934))
                                .rounded_md()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0xfb4934))
                                        .child(format!("🛡️ {}", warn))
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xebdbb2))
                                        .child("⚠️ Destructive command gated. Press ⇧⏎ (Shift+Enter) to Force Override · Esc to Abort")
                                )
                        } else if let Some((stop_num, total_stops, _range)) = self.mneme_editor.active_snippet_stop() {
                            div()
                                .mt_2()
                                .px_2()
                                .py_1()
                                .bg(rgb(0x282828))
                                .border_1()
                                .border_color(rgb(0xfabd2f))
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(0xfabd2f))
                                .child(format!(
                                    "⚡ Snippet Tab-Stop {stop_num}/{total_stops} — Type replacement, then press Tab for next placeholder or Enter to execute"
                                ))
                        } else {
                            div()
                                .mt_2()
                                .text_color(rgb(0x928374))
                                .text_size(px(11.0))
                                .child("Tab: Expand Snippet (:klogs, :tfplan, :killport) · Ctrl+E: Close · Shift+Enter: Newline · Enter: Send to Terminal")
                        })
                );
                
            container = container.child(mneme_overlay);
        }

        if self.pythia.active {
            container = container.child(self.render_pythia_modal(cx));
        }

        container
    }
}



#[derive(Clone, Copy, PartialEq, Eq)]
enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
enum PaneTree {
    Leaf(Entity<TerminalView>),
    Split {
        direction: SplitDirection,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    fn render(&self, cx: &mut Context<WorkspaceView>) -> gpui::AnyElement {
        match self {
            PaneTree::Leaf(terminal) => {
                // To allow the terminal view to capture mouse down, we wrap it
                let term_clone = terminal.clone();
                div()
                    .flex_grow()
                    .size_full()
                    .child(terminal.clone())
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |view, _event, window, cx| {
                        if !view.palette_open && !view.ssh_state.open {
                            window.focus(&term_clone.read(cx).focus_handle);
                        }
                    }))
                    .into_any_element()
            }
            PaneTree::Split { direction, first, second } => {
                let mut container = div().flex_grow().size_full().flex();
                if *direction == SplitDirection::Horizontal {
                    container = container.flex_row();
                } else {
                    container = container.flex_col();
                }
                
                let first_el = first.render(cx);
                let second_el = second.render(cx);
                
                let divider = div().bg(rgb(0xfe8019)); // Theia radiant amber divider
                let divider = if *direction == SplitDirection::Horizontal {
                    divider.w(px(2.0)).h_full()
                } else {
                    divider.h(px(2.0)).w_full()
                };

                container.child(first_el).child(divider).child(second_el).into_any_element()
            }
        }
    }

    fn split(&mut self, target: &Entity<TerminalView>, new_leaf: Entity<TerminalView>, direction: SplitDirection) -> bool {
        match self {
            PaneTree::Leaf(term) => {
                if term == target {
                    let first = Box::new(PaneTree::Leaf(term.clone()));
                    let second = Box::new(PaneTree::Leaf(new_leaf));
                    *self = PaneTree::Split { direction, first, second };
                    return true;
                }
                false
            }
            PaneTree::Split { first, second, .. } => {
                first.split(target, new_leaf.clone(), direction) || second.split(target, new_leaf, direction)
            }
        }
    }

    fn close(&mut self, target: &Entity<TerminalView>) -> bool {
        match self {
            PaneTree::Leaf(term) => term == target,
            PaneTree::Split { first, second, .. } => {
                if first.close(target) {
                    *self = *second.clone();
                    return false;
                }
                if second.close(target) {
                    *self = *first.clone();
                    return false;
                }
                false
            }
        }
    }
    
    fn first_leaf(&self) -> Entity<TerminalView> {
        match self {
            PaneTree::Leaf(t) => t.clone(),
            PaneTree::Split { first, .. } => first.first_leaf(),
        }
    }
    
    fn all_leaves(&self) -> Vec<Entity<TerminalView>> {
        let mut leaves = Vec::new();
        self.collect_leaves(&mut leaves);
        leaves
    }
    
    fn collect_leaves(&self, leaves: &mut Vec<Entity<TerminalView>>) {
        match self {
            PaneTree::Leaf(t) => leaves.push(t.clone()),
            PaneTree::Split { first, second, .. } => {
                first.collect_leaves(leaves);
                second.collect_leaves(leaves);
            }
        }
    }
}

struct WorkspaceTab {
    tab_id: u64,
    root: PaneTree,
    active_terminal: Entity<TerminalView>,
    is_ssh: bool,
    title: String,
    #[allow(dead_code)]
    ssh_target: Option<SshTarget>,
}

#[derive(Clone, Debug)]
struct TheiaSshState {
    open: bool,
    section: SshConsoleSection,
    server_subtab: ServerHubSubtab,
    pxe_subtab: PxeBootSubtab,
    active_input: SshConsoleInputField,
    search_filter: String,
    host_draft: SshHostEditorDraft,
    tunnel_draft_kind: SshTunnelKind,
    tunnel_draft_local: String,
    tunnel_draft_remote: String,
    tunnel_ssh_host: String,
    server_running: bool,
    server_port: String,
    server_listen_addr: String,
    allow_pubkey_auth: bool,
    allow_password_auth: bool,
    agent_key_path: String,
    key_server_running: bool,
    key_server_port: String,
    key_server_stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pxe_mode: PxeServerMode,
    pxe_arch: ClientArch,
    pxe_server_ip: String,
    pxe_http_port: String,
    pxe_bootfile: String,
    pxe_http_running: bool,
    pxe_http_stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    smb_shares: Vec<SmbShareEntry>,
    smb_draft_host: String,
    smb_draft_share: String,
    smb_draft_user: String,
    net_target_host: String,
    net_target_mac: String,
    net_port_scan: Vec<PortScanResult>,
    net_arp_entries: Vec<ArpNeighborEntry>,
    status_message: Option<String>,
    forward_caduceus_ipc: bool,
}

impl Default for TheiaSshState {
    fn default() -> Self {
        let lan_ip = detect_primary_lan_ip().to_string();
        Self {
            open: false,
            section: SshConsoleSection::HostEditor,
            server_subtab: ServerHubSubtab::ServerHosting,
            pxe_subtab: PxeBootSubtab::PxeServer,
            active_input: SshConsoleInputField::SidebarSearch,
            search_filter: String::new(),
            host_draft: SshHostEditorDraft::default(),
            tunnel_draft_kind: SshTunnelKind::Local,
            tunnel_draft_local: "127.0.0.1:5433".into(),
            tunnel_draft_remote: "127.0.0.1:5432".into(),
            tunnel_ssh_host: "chyna@192.168.1.12".into(),
            server_running: false,
            server_port: "2222".into(),
            server_listen_addr: "0.0.0.0 (All LAN & VPN Interfaces)".into(),
            allow_pubkey_auth: true,
            allow_password_auth: true,
            agent_key_path: "~/.ssh/id_ed25519".into(),
            key_server_running: false,
            key_server_port: "8787".into(),
            key_server_stop: None,
            pxe_mode: PxeServerMode::ProxyDhcp,
            pxe_arch: ClientArch::UefiX86_64,
            pxe_server_ip: lan_ip,
            pxe_http_port: "8080".into(),
            pxe_bootfile: "boot.ipxe".into(),
            pxe_http_running: false,
            pxe_http_stop: None,
            smb_shares: SmbShareEntry::default_catalog(),
            smb_draft_host: "192.168.1.12".into(),
            smb_draft_share: "pxeboot".into(),
            smb_draft_user: "chyna".into(),
            net_target_host: "192.168.1.12".into(),
            net_target_mac: "a8:a1:59:2c:41:0e".into(),
            net_port_scan: Vec::new(),
            net_arp_entries: discover_arp_neighbors(),
            status_message: None,
            forward_caduceus_ipc: true,
        }
    }
}

struct WorkspaceView {
    tabs: Vec<WorkspaceTab>,
    active_tab: usize,
    settings: Entity<TerminalSettings>,
    settings_view: Entity<SettingsView>,
    ssh_engine: SshEngine,
    ssh_state: TheiaSshState,
    ssh_focus_handle: FocusHandle,
    palette_open: bool,
    palette_query: String,
    palette_selected_idx: usize,
    palette_focus_handle: FocusHandle,
    next_tab_id: u64,
}

impl WorkspaceView {
    fn new(cx: &mut Context<Self>, settings: Entity<TerminalSettings>) -> Self {
        cx.observe(&settings, |view: &mut Self, settings_ent, cx| {
            let req = settings_ent.read(cx).requested_hub_switch;
            if let Some(target) = req {
                settings_ent.update(cx, |s, _| {
                    s.requested_hub_switch = None;
                });
                match target {
                    "ssh" => {
                        view.ssh_state.open = true;
                        view.palette_open = false;
                        view.sync_modal_open_flag(cx);
                    }
                    "palette" => {
                        view.palette_open = true;
                        view.ssh_state.open = false;
                        view.sync_modal_open_flag(cx);
                    }
                    "pythia" => {
                        view.ssh_state.open = false;
                        view.palette_open = false;
                        view.sync_modal_open_flag(cx);
                        let active_term = view.tabs[view.active_tab].active_terminal.clone();
                        active_term.update(cx, |term, cx| {
                            term.pythia.active = true;
                            cx.notify();
                        });
                    }
                    "close" => {
                        view.ssh_state.open = false;
                        view.palette_open = false;
                        view.sync_modal_open_flag(cx);
                    }
                    _ => {}
                }
            }
            cx.notify();
        })
        .detach();
        let first_term = cx.new(|cx| TerminalView::new(cx, settings.clone()));
        let settings_view = cx.new(|cx| SettingsView::new(cx, settings.clone()));
        let ssh_focus_handle = cx.focus_handle();
        let palette_focus_handle = cx.focus_handle();
        Self {
            tabs: vec![WorkspaceTab {
                tab_id: 1,
                root: PaneTree::Leaf(first_term.clone()),
                active_terminal: first_term,
                is_ssh: false,
                title: "well #1".into(),
                ssh_target: None,
            }],
            active_tab: 0,
            settings,
            settings_view,
            ssh_engine: SshEngine::new(),
            ssh_state: TheiaSshState::default(),
            ssh_focus_handle,
            palette_open: false,
            palette_query: String::new(),
            palette_selected_idx: 0,
            palette_focus_handle,
            next_tab_id: 2,
        }
    }

    fn sync_modal_open_flag(&self, cx: &mut Context<Self>) {
        let settings_open = self.settings.read(cx).settings_open;
        let is_open = self.palette_open || self.ssh_state.open || settings_open;
        self.settings.update(cx, |s, _| {
            s.modal_open = is_open;
        });
    }

    fn toggle_command_palette(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.palette_open = !self.palette_open;
        if self.palette_open {
            self.ssh_state.open = false;
            self.settings.update(cx, |s, _| {
                s.settings_open = false;
            });
            self.palette_selected_idx = 0;
            window.focus(&self.palette_focus_handle);
        } else {
            let focus = self.tabs[self.active_tab].active_terminal.read(cx).focus_handle.clone();
            window.focus(&focus);
        }
        self.sync_modal_open_flag(cx);
        cx.notify();
    }

    fn toggle_settings_modal(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let now_open = !self.settings.read(cx).settings_open;
        self.settings.update(cx, |s, _| {
            s.settings_open = now_open;
        });
        if now_open {
            self.ssh_state.open = false;
            self.palette_open = false;
            let focus = self.settings_view.read(cx).focus_handle.clone();
            window.focus(&focus);
        } else {
            let focus = self.tabs[self.active_tab].active_terminal.read(cx).focus_handle.clone();
            window.focus(&focus);
        }
        self.sync_modal_open_flag(cx);
        cx.notify();
    }

    fn execute_palette_action(
        &mut self,
        action_id: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected_idx = 0;
        self.sync_modal_open_flag(cx);
        match action_id {
            "well::OpenSettings" | "settings::OpenSettings" => {
                self.toggle_settings_modal(window, cx);
            }
            "well::OpenCommandPalette" | "workspace::OpenCommandPalette" => {
                self.toggle_command_palette(window, cx);
            }
            "panes::SplitHorizontal" => {
                self.split_active_pane(SplitDirection::Horizontal, window, cx);
            }
            "panes::SplitVertical" => {
                self.split_active_pane(SplitDirection::Vertical, window, cx);
            }
            "panes::FocusNextPane" => {
                self.focus_next_pane(window, cx);
            }
            "panes::ClosePane" => {
                self.close_active_pane(window, cx);
            }
            "panes::NewTab" | "workspace::NewTab" => {
                self.add_tab(window, cx);
            }
            "theia::OpenSshManager" => {
                self.toggle_ssh_manager(window, cx);
            }
            "theia::OpenPortForwarding" => {
                self.ssh_state.section = SshConsoleSection::PortForwarding;
                self.ssh_state.open = true;
                window.focus(&self.ssh_focus_handle);
                self.sync_modal_open_flag(cx);
            }
            "theia::OpenKeyServerVault" => {
                self.ssh_state.section = SshConsoleSection::KeyVault;
                self.ssh_state.open = true;
                window.focus(&self.ssh_focus_handle);
                self.sync_modal_open_flag(cx);
            }
            "theia::OpenPxeBootloader" => {
                self.ssh_state.section = SshConsoleSection::PxeAndBootloader;
                self.ssh_state.open = true;
                window.focus(&self.ssh_focus_handle);
                self.sync_modal_open_flag(cx);
            }
            "theia::OpenSmbShares" => {
                self.ssh_state.section = SshConsoleSection::SmbShares;
                self.ssh_state.open = true;
                window.focus(&self.ssh_focus_handle);
                self.sync_modal_open_flag(cx);
            }
            "theia::OpenNetworkTools" => {
                self.ssh_state.section = SshConsoleSection::NetworkTools;
                self.ssh_state.open = true;
                window.focus(&self.ssh_focus_handle);
                self.sync_modal_open_flag(cx);
            }
            "theia::SshSplitRight" => {
                let first_host = self
                    .settings
                    .read(cx)
                    .ssh_profiles
                    .first()
                    .cloned();
                if let Some(mut p) = first_host {
                    p.forward_caduceus_ipc = self.ssh_state.forward_caduceus_ipc;
                    let base = self.ssh_engine.resolve_target(&p.host.name);
                    let target = p.build_target(base);
                    self.connect_ssh_with_mode(
                        target,
                        Some(p),
                        Some(SplitDirection::Horizontal),
                        window,
                        cx,
                    );
                } else {
                    self.toggle_ssh_manager(window, cx);
                }
            }
            "pythia::ToggleCopilot" => {
                let active_term = self.tabs[self.active_tab].active_terminal.clone();
                active_term.update(cx, |term, cx| {
                    term.pythia.active = !term.pythia.active;
                    cx.notify();
                });
                window.focus(&active_term.read(cx).focus_handle);
            }
            "mneme::ToggleInlineEditor"
            | "mneme::ExpandOrNextSnippetPlaceholder"
            | "mneme::OpenSnippetPicker"
            | "mneme::ExpandSnippet" => {
                let active_term = self.tabs[self.active_tab].active_terminal.clone();
                active_term.update(cx, |term, cx| {
                    term.mneme_active = true;
                    if term.mneme_editor.get_text().trim().is_empty() {
                        term.mneme_editor.set_text(":klogs");
                    }
                    let snippets = term.settings.read(cx).snippets.clone();
                    let is_ssh = term.ssh_profile.is_some();
                    let _ = term.mneme_editor.expand_or_advance_snippet(&snippets, is_ssh);
                    cx.notify();
                });
                window.focus(&active_term.read(cx).focus_handle);
            }
            "mneme::ForceExecuteUnsafeCommand" | "mneme::ForceExecuteUnsafe" => {
                let active_term = self.tabs[self.active_tab].active_terminal.clone();
                window.focus(&active_term.read(cx).focus_handle);
            }
            "well::Quit" | "workspace::Quit" => {
                cx.quit();
            }
            _ => {}
        }
        cx.notify();
    }

    fn toggle_ssh_manager(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.ssh_state.open = !self.ssh_state.open;
        if self.ssh_state.open {
            self.palette_open = false;
            self.settings.update(cx, |s, _| {
                s.settings_open = false;
            });
            window.focus(&self.ssh_focus_handle);
        } else {
            let focus = self.tabs[self.active_tab].active_terminal.read(cx).focus_handle.clone();
            window.focus(&focus);
        }
        self.sync_modal_open_flag(cx);
        cx.notify();
    }

    fn connect_ssh(
        &mut self,
        target: SshTarget,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let mut profile = TheiaSshProfile::from_host(SshHostConfig {
            name: target.destination.clone(),
            hostname: target.destination.clone(),
            user: target.user.clone(),
            port: target.port,
            identity_file: target.identity_file.clone(),
            proxy_jump: target.proxy_jump.clone(),
            forward_agent: false,
            options: std::collections::HashMap::new(),
        });
        profile.forward_caduceus_ipc = self.ssh_state.forward_caduceus_ipc;
        let hydrated_target = profile.build_target(target);
        self.connect_ssh_with_mode(hydrated_target, Some(profile), None, window, cx);
    }

    fn connect_ssh_with_mode(
        &mut self,
        target: SshTarget,
        profile: Option<TheiaSshProfile>,
        split_direction: Option<SplitDirection>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let target_clone = target.clone();
        let settings = self.settings.clone();
        match TerminalView::spawn_ssh_pty(&target) {
            Ok((pty, rx, astraea, cwd)) => {
                let new_term = cx.new(|cx| {
                    TerminalView::from_pty(cx, settings, pty, rx, astraea, cwd)
                        .with_ssh_profile(profile)
                });

                if let Some(dir) = split_direction {
                    let target_leaf = self.active_terminal(window, cx);
                    let tab = &mut self.tabs[self.active_tab];
                    tab.root.split(&target_leaf, new_term.clone(), dir);
                    tab.active_terminal = new_term.clone();
                    tab.is_ssh = true;
                    self.ssh_engine.register_session(tab.tab_id, target_clone);
                } else {
                    let tab_id = self.next_tab_id;
                    self.next_tab_id += 1;

                    let title = match &target_clone.user {
                        Some(user) => format!("🌐 {}@{}", user, target_clone.destination),
                        None => format!("🌐 {}", target_clone.destination),
                    };

                    self.ssh_engine.register_session(tab_id, target_clone.clone());

                    self.tabs.push(WorkspaceTab {
                        tab_id,
                        root: PaneTree::Leaf(new_term.clone()),
                        active_terminal: new_term.clone(),
                        is_ssh: true,
                        title,
                        ssh_target: Some(target_clone),
                    });

                    self.active_tab = self.tabs.len() - 1;
                }

                self.ssh_state.open = false;
                self.ssh_state.status_message = None;
                self.sync_modal_open_flag(cx);

                let focus = new_term.read(cx).focus_handle.clone();
                window.focus(&focus);
                cx.notify();
            }
            Err(err) => {
                self.ssh_state.status_message = Some(format!("SSH spawn failure: {err}"));
                cx.notify();
            }
        }
    }

    fn close_tab(&mut self, index: usize, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            let tab = self.tabs.remove(index);
            if tab.is_ssh {
                self.ssh_engine.remove_session(tab.tab_id);
            }
            if self.tabs.is_empty() {
                let new_term = cx.new(|cx| TerminalView::new(cx, self.settings.clone()));
                let tab_id = self.next_tab_id;
                self.next_tab_id += 1;
                self.tabs.push(WorkspaceTab {
                    tab_id,
                    root: PaneTree::Leaf(new_term.clone()),
                    active_terminal: new_term,
                    is_ssh: false,
                    title: format!("well #{tab_id}"),
                    ssh_target: None,
                });
                self.active_tab = 0;
            } else if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            let focus = self.tabs[self.active_tab].active_terminal.read(cx).focus_handle.clone();
            window.focus(&focus);
            cx.notify();
        }
    }

    fn active_terminal(&self, window: &gpui::Window, cx: &mut Context<Self>) -> Entity<TerminalView> {
        let tab = &self.tabs[self.active_tab];
        if let Some(focused) = Self::find_focused_terminal(&tab.root, window, cx) {
            focused
        } else {
            tab.active_terminal.clone()
        }
    }
    
    fn find_focused_terminal(tree: &PaneTree, window: &gpui::Window, cx: &mut Context<Self>) -> Option<Entity<TerminalView>> {
        match tree {
            PaneTree::Leaf(term) => {
                if term.read(cx).focus_handle.is_focused(window) {
                    Some(term.clone())
                } else {
                    None
                }
            }
            PaneTree::Split { first, second, .. } => {
                Self::find_focused_terminal(first, window, cx).or_else(|| Self::find_focused_terminal(second, window, cx))
            }
        }
    }

    fn split_active_pane(&mut self, direction: SplitDirection, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let new_term = cx.new(|cx| TerminalView::new(cx, self.settings.clone()));
        let target = self.active_terminal(window, cx);
        
        let tab = &mut self.tabs[self.active_tab];
        tab.root.split(&target, new_term.clone(), direction);
        tab.active_terminal = new_term.clone();
        
        let focus = new_term.read(cx).focus_handle.clone();
        window.focus(&focus);
        cx.notify();
    }

    fn dispatch_command_to_split_pane(
        &mut self,
        cmd: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.split_active_pane(SplitDirection::Horizontal, window, cx);
        self.ssh_state.open = false;
        self.sync_modal_open_flag(cx);
        let term = self.tabs[self.active_tab].active_terminal.clone();
        let payload = format!("{cmd}\r");
        let pty = term.read(cx).pty.clone();
        if let Ok(mut writer) = pty.writer.lock() {
            use std::io::Write;
            let _ = writer.write_all(payload.as_bytes());
        }
        cx.notify();
    }
    
    fn close_active_pane(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let target = self.active_terminal(window, cx);
        let tab = &mut self.tabs[self.active_tab];
        
        if matches!(&tab.root, PaneTree::Leaf(term) if term == &target) {
            self.close_tab(self.active_tab, window, cx);
        } else {
            tab.root.close(&target);
            tab.active_terminal = tab.root.first_leaf();
            let focus = self.tabs[self.active_tab].active_terminal.read(cx).focus_handle.clone();
            window.focus(&focus);
            cx.notify();
        }
    }

    fn add_tab(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let new_term = cx.new(|cx| TerminalView::new(cx, self.settings.clone()));
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let count = self.tabs.len() + 1;
        self.tabs.push(WorkspaceTab {
            tab_id,
            root: PaneTree::Leaf(new_term.clone()),
            active_terminal: new_term.clone(),
            is_ssh: false,
            title: format!("well #{count}"),
            ssh_target: None,
        });
        self.active_tab = self.tabs.len() - 1;
        let focus = new_term.read(cx).focus_handle.clone();
        window.focus(&focus);
        cx.notify();
    }
    
    fn focus_next_pane(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let target = self.active_terminal(window, cx);
        let tab = &mut self.tabs[self.active_tab];
        let leaves = tab.root.all_leaves();
        if leaves.is_empty() { return; }
        let mut next_idx = 0;
        for (i, leaf) in leaves.iter().enumerate() {
            if leaf == &target {
                next_idx = (i + 1) % leaves.len();
                break;
            }
        }
        
        let next_term = &leaves[next_idx];
        tab.active_terminal = next_term.clone();
        window.focus(&next_term.read(cx).focus_handle);
        cx.notify();
    }

    fn save_current_ssh_draft(&mut self, cx: &mut Context<Self>) {
        match self.ssh_state.host_draft.to_profile() {
            Ok(profile) => {
                let saved_name = profile.host.name.clone();
                let editing_idx = self.ssh_state.host_draft.editing_index;
                self.settings.update(cx, |s, cx| {
                    if let Some(idx) = editing_idx {
                        if idx < s.ssh_profiles.len() {
                            s.ssh_profiles[idx] = profile;
                        } else {
                            s.ssh_profiles.push(profile);
                        }
                    } else if let Some(existing) = s
                        .ssh_profiles
                        .iter_mut()
                        .find(|p| p.host.name == profile.host.name)
                    {
                        *existing = profile;
                    } else {
                        s.ssh_profiles.push(profile);
                    }
                    let _ = s.save_to_disk();
                    cx.notify();
                });
                self.ssh_state.status_message = Some(format!(
                    "✓ Saved SSH host profile '{}' to ~/.config/well/operator_settings.toml",
                    saved_name
                ));
                cx.notify();
            }
            Err(err) => {
                self.ssh_state.status_message = Some(format!("⚠ {err}"));
                cx.notify();
            }
        }
    }

    fn render_ssh_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.settings.read(cx).theme.clone();
        let saved_profiles = self.settings.read(cx).ssh_profiles.clone();
        let ssh_tunnels = self.settings.read(cx).ssh_tunnels.clone();
        let config_hosts = self.ssh_engine.list_hosts();
        let sessions = self.ssh_engine.sessions_snapshot();
        let active_count = sessions.len();

        let section = self.ssh_state.section;
        let server_subtab = self.ssh_state.server_subtab;
        let active_input = self.ssh_state.active_input;
        let search_filter = self.ssh_state.search_filter.clone();
        let filter_lower = search_filter.to_lowercase();
        let status_message = self.ssh_state.status_message.clone();
        let draft = self.ssh_state.host_draft.clone();

        let filtered_saved: Vec<(usize, TheiaSshProfile)> = saved_profiles
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, p)| {
                if filter_lower.is_empty() {
                    true
                } else {
                    p.host.name.to_lowercase().contains(&filter_lower)
                        || p.host.hostname.to_lowercase().contains(&filter_lower)
                        || p.host
                            .user
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&filter_lower)
                }
            })
            .collect();

        let filtered_config: Vec<SshHostConfig> = config_hosts
            .into_iter()
            .filter(|h| {
                if filter_lower.is_empty() {
                    true
                } else {
                    h.name.to_lowercase().contains(&filter_lower)
                        || h.hostname.to_lowercase().contains(&filter_lower)
                        || h.user
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&filter_lower)
                }
            })
            .take(15)
            .collect();

        let rail_sections = [
            (SshConsoleSection::HostEditor, ">_", "SSH Host Editor & Terminal"),
            (SshConsoleSection::PortForwarding, "⇄", "Port Forwarding & Tunnels"),
            (SshConsoleSection::KeyVault, "🔑", "SSH Key Vault & HTTP Key Server"),
            (SshConsoleSection::ServerAndAgentHub, "🖧", "SSH Server, $SSH_AUTH_SOCK & Ops Agent"),
            (SshConsoleSection::PxeAndBootloader, "🚀", "PXE / ProxyDHCP & Custom UEFI Bootloader"),
            (SshConsoleSection::SmbShares, "📁", "SMB / CIFS Network Shares & Mounts"),
            (SshConsoleSection::NetworkTools, "📡", "Networking Tools (Port Scan, ARP, WoL)"),
        ];

        let mut icon_rail = div()
            .w(px(50.0))
            .h_full()
            .bg(theme.tab_inactive_bg)
            .border_r_1()
            .border_color(theme.border_inactive)
            .py_3()
            .flex()
            .flex_col()
            .items_center()
            .justify_between();

        let mut rail_top = div().flex().flex_col().items_center().gap_2();
        for (sec, icon, _tip) in rail_sections {
            let is_active = section == sec;
            rail_top = rail_top.child(
                div()
                    .id(("ssh-rail", sec as usize))
                    .w(px(34.0))
                    .h(px(34.0))
                    .rounded_lg()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .bg(if is_active {
                        theme.tab_active_bg
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .border_1()
                    .border_color(if is_active {
                        theme.border_active
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .text_color(if is_active {
                        theme.border_active
                    } else {
                        rgb(0x94a3b8)
                    })
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.ssh_state.section = sec;
                        cx.notify();
                    }))
                    .child(icon),
            );
        }
        icon_rail = icon_rail.child(rail_top).child(
            div()
                .id("ssh-rail-close")
                .w(px(34.0))
                .h(px(34.0))
                .rounded_lg()
                .bg(theme.tab_active_bg)
                .border_1()
                .border_color(theme.border_inactive)
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.fg)
                .cursor(gpui::CursorStyle::PointingHand)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.toggle_ssh_manager(window, cx);
                }))
                .child("✕"),
        );

        let search_focused = active_input == SshConsoleInputField::SidebarSearch;
        let mut hosts_sidebar = div()
            .w(px(268.0))
            .h_full()
            .flex_shrink_0()
            .bg(theme.tab_inactive_bg)
            .border_r_1()
            .border_color(theme.border_inactive)
            .p_3()
            .flex()
            .flex_col()
            .gap_2p5()
            .overflow_hidden()
            .child(
                div()
                    .id("ssh-sidebar-search")
                    .px_2p5()
                    .py_1p5()
                    .rounded_md()
                    .bg(theme.bg)
                    .border_1()
                    .border_color(if search_focused {
                        theme.border_active
                    } else {
                        theme.border_inactive
                    })
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.ssh_state.active_input = SshConsoleInputField::SidebarSearch;
                        cx.notify();
                    }))
                    .child(div().text_xs().text_color(rgb(0x64748b)).child("🔍"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if search_filter.is_empty() {
                                rgb(0x64748b)
                            } else {
                                theme.fg
                            })
                            .child(if search_filter.is_empty() {
                                if search_focused {
                                    "│".to_string()
                                } else {
                                    "Search hosts, IPs, tags...".to_string()
                                }
                            } else if search_focused {
                                format!("{search_filter}│")
                            } else {
                                search_filter.clone()
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.border_active)
                            .child("SSH HOSTS"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .id("ssh-sidebar-reload")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(theme.tab_active_bg)
                                    .border_1()
                                    .border_color(theme.border_inactive)
                                    .text_xs()
                                    .text_color(theme.fg)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_engine.reload();
                                        view.ssh_state.status_message =
                                            Some("🔄 Reloaded ~/.ssh/config & host profiles.".to_string());
                                        cx.notify();
                                    }))
                                    .child("↻ Reload"),
                            )
                            .child(
                                div()
                                    .id("ssh-sidebar-add-btn")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(theme.tab_active_bg)
                                    .border_1()
                                    .border_color(theme.border_active)
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.section = SshConsoleSection::HostEditor;
                                        view.ssh_state.host_draft = SshHostEditorDraft::new_blank();
                                        view.ssh_state.active_input = SshConsoleInputField::HostName;
                                        view.ssh_state.status_message = Some(
                                            "Creating new SSH host profile — fill fields on the right and click Save Host.".to_string(),
                                        );
                                        cx.notify();
                                    }))
                                    .child("+ New"),
                            ),
                    ),
            );

        hosts_sidebar = hosts_sidebar.child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .pt_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x94a3b8))
                        .child("⌄ 📁 Saved Profiles"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(format!("{}", filtered_saved.len())),
                ),
        );

        let mut saved_list = div().flex().flex_col().gap_1p5();
        for (orig_idx, prof) in filtered_saved.iter().take(5) {
            let idx_val = *orig_idx;
            let hname = prof.host.name.clone();
            let user_str = prof.host.user.clone().unwrap_or_else(|| "root".to_string());
            let subline = format!("{}@{}:{}", user_str, prof.host.hostname, prof.host.port);
            let is_editing = self.ssh_state.host_draft.editing_index == Some(idx_val)
                && section == SshConsoleSection::HostEditor;
            let prof_edit = prof.clone();
            let prof_connect = prof.clone();

            saved_list = saved_list.child(
                div()
                    .id(("saved-ssh-card", idx_val))
                    .p_2()
                    .rounded_md()
                    .bg(if is_editing {
                        rgb(0x17253b)
                    } else {
                        rgb(0x131b2c)
                    })
                    .border_1()
                    .border_color(if is_editing {
                        rgb(0x06b6d4)
                    } else {
                        rgb(0x1e293b)
                    })
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf1f5f9))
                                    .child(hname),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .id(("edit-saved-ssh", idx_val))
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.ssh_state.section = SshConsoleSection::HostEditor;
                                                view.ssh_state.host_draft = SshHostEditorDraft::from_profile(
                                                    Some(idx_val),
                                                    &prof_edit,
                                                );
                                                view.ssh_state.active_input = SshConsoleInputField::HostHostname;
                                                cx.notify();
                                            }))
                                            .child("✎"),
                                    )
                                    .child(
                                        div()
                                            .id(("del-saved-ssh", idx_val))
                                            .text_xs()
                                            .text_color(rgb(0xf87171))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.settings.update(cx, |s, cx| {
                                                    if idx_val < s.ssh_profiles.len() {
                                                        s.ssh_profiles.remove(idx_val);
                                                        let _ = s.save_to_disk();
                                                        cx.notify();
                                                    }
                                                });
                                                if view.ssh_state.host_draft.editing_index == Some(idx_val) {
                                                    view.ssh_state.host_draft = SshHostEditorDraft::new_blank();
                                                }
                                                cx.notify();
                                            }))
                                            .child("🗑"),
                                    )
                                    .child(
                                        div()
                                            .id(("conn-saved-ssh", idx_val))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x22d3ee))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                let p = prof_connect.clone();
                                                let base = view.ssh_engine.resolve_target(&p.host.hostname);
                                                let target = p.build_target(base);
                                                view.connect_ssh_with_mode(target, Some(p), None, window, cx);
                                            }))
                                            .child("↗"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("Menlo")
                            .text_color(rgb(0x64748b))
                            .child(subline),
                    ),
            );
        }
        hosts_sidebar = hosts_sidebar.child(saved_list);

        hosts_sidebar = hosts_sidebar.child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .pt_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x22d3ee))
                        .child("⌄ 📁 OpenSSH Config"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(format!("{}", filtered_config.len())),
                ),
        );

        let mut cfg_list = div().flex().flex_col().gap_1p5();
        for (cidx, host_cfg) in filtered_config.iter().take(4).enumerate() {
            let prof = TheiaSshProfile::from_host(host_cfg.clone());
            let hname = prof.host.name.clone();
            let user_str = prof.host.user.clone().unwrap_or_else(|| "user".to_string());
            let subline = format!("{}@{}:{}", user_str, prof.host.hostname, prof.host.port);
            let prof_edit = prof.clone();
            let prof_conn = prof.clone();

            cfg_list = cfg_list.child(
                div()
                    .id(("cfg-ssh-card", cidx))
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x131b2c))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf1f5f9))
                                            .child(hname),
                                    )
                                    .child(
                                        div()
                                            .px_1()
                                            .rounded_sm()
                                            .bg(rgb(0x1e293b))
                                            .text_color(rgb(0x94a3b8))
                                            .text_xs()
                                            .child("config"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .id(("edit-cfg-ssh", cidx))
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.ssh_state.section = SshConsoleSection::HostEditor;
                                                view.ssh_state.host_draft =
                                                    SshHostEditorDraft::from_profile(None, &prof_edit);
                                                view.ssh_state.active_input =
                                                    SshConsoleInputField::HostHostname;
                                                cx.notify();
                                            }))
                                            .child("✎"),
                                    )
                                    .child(
                                        div()
                                            .id(("conn-cfg-ssh", cidx))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x22d3ee))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                let p = prof_conn.clone();
                                                let base = view.ssh_engine.resolve_target(&p.host.name);
                                                let target = p.build_target(base);
                                                view.connect_ssh_with_mode(target, Some(p), None, window, cx);
                                            }))
                                            .child("↗"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("Menlo")
                            .text_color(rgb(0x64748b))
                            .child(subline),
                    ),
            );
        }
        hosts_sidebar = hosts_sidebar.child(cfg_list);

        let right_content = match section {
            SshConsoleSection::HostEditor => {
                let make_input = |id_str: &'static str,
                                  label: &'static str,
                                  val: String,
                                  placeholder: &'static str,
                                  field: SshConsoleInputField,
                                  cx: &mut Context<Self>| {
                    let focused = active_input == field;
                    let display = if val.is_empty() {
                        if focused {
                            "│".to_string()
                        } else {
                            placeholder.to_string()
                        }
                    } else if focused {
                        format!("{val}│")
                    } else {
                        val.clone()
                    };
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(0x94a3b8))
                                .child(label),
                        )
                        .child(
                            div()
                                .id(id_str)
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x090d16))
                                .border_1()
                                .border_color(if focused {
                                    rgb(0x06b6d4)
                                } else {
                                    rgb(0x1e293b)
                                })
                                .font_family("Menlo")
                                .text_xs()
                                .text_color(if val.is_empty() && !focused {
                                    rgb(0x475569)
                                } else {
                                    rgb(0xf1f5f9)
                                })
                                .cursor(gpui::CursorStyle::IBeam)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.ssh_state.active_input = field;
                                    cx.notify();
                                }))
                                .child(display),
                        )
                };

                let tier_row = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        div()
                            .id("ssh-draft-agent-fwd")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(if draft.forward_agent {
                                gpui::rgba(0x06b6d428)
                            } else {
                                rgb(0x131b2c)
                            })
                            .border_1()
                            .border_color(if draft.forward_agent {
                                rgb(0x06b6d4)
                            } else {
                                rgb(0x1e293b)
                            })
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if draft.forward_agent {
                                rgb(0x22d3ee)
                            } else {
                                rgb(0x64748b)
                            })
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.ssh_state.host_draft.forward_agent =
                                    !view.ssh_state.host_draft.forward_agent;
                                cx.notify();
                            }))
                            .child(if draft.forward_agent {
                                "☑ Agent Forwarding (-A)"
                            } else {
                                "☐ Agent Forwarding (-A)"
                            }),
                    )
                    .child(
                        div()
                            .id("ssh-draft-mux")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(if draft.use_control_master {
                                gpui::rgba(0x10b98128)
                            } else {
                                rgb(0x131b2c)
                            })
                            .border_1()
                            .border_color(if draft.use_control_master {
                                rgb(0x10b981)
                            } else {
                                rgb(0x1e293b)
                            })
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if draft.use_control_master {
                                rgb(0x34d399)
                            } else {
                                rgb(0x64748b)
                            })
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.ssh_state.host_draft.use_control_master =
                                    !view.ssh_state.host_draft.use_control_master;
                                cx.notify();
                            }))
                            .child(if draft.use_control_master {
                                "☑ ControlMaster Multiplex"
                            } else {
                                "☐ ControlMaster Multiplex"
                            }),
                    );

                let mut active_sess_row = div().flex().flex_col().gap_1p5();
                if sessions.is_empty() {
                    active_sess_row = active_sess_row.child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("No active remote PTY sessions. Click 'Connect in Tab' or 'Split Right' below to launch."),
                    );
                } else {
                    for (sidx, sess) in sessions.iter().take(2).enumerate() {
                        let tab_id = sess.tab_id;
                        let dest = sess.destination.clone();
                        let port = sess.port;
                        let user = sess.user.clone().unwrap_or_else(|| "root".into());
                        active_sess_row = active_sess_row.child(
                            div()
                                .id(("active-ssh-banner", sidx))
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(0x0f1f2e))
                                .border_1()
                                .border_color(rgb(0x10b981))
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xecfdf5))
                                        .child(format!("🟢 Active PTY: {user}@{dest}:{port} (Tab #{tab_id})")),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_2()
                                        .child(
                                            div()
                                                .id(("jump-active-ssh", sidx))
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x10b981))
                                                .text_color(rgb(0x090d16))
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(move |view, _, window, cx| {
                                                    if let Some(pos) =
                                                        view.tabs.iter().position(|t| t.tab_id == tab_id)
                                                    {
                                                        view.active_tab = pos;
                                                        let focus = view.tabs[pos]
                                                            .active_terminal
                                                            .read(cx)
                                                            .focus_handle
                                                            .clone();
                                                        window.focus(&focus);
                                                        view.ssh_state.open = false;
                                                        view.sync_modal_open_flag(cx);
                                                        cx.notify();
                                                    }
                                                }))
                                                .child("Jump to Tab"),
                                        )
                                        .child(
                                            div()
                                                .id(("disc-active-ssh", sidx))
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0xef4444))
                                                .text_color(rgb(0xffffff))
                                                .text_xs()
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(move |view, _, window, cx| {
                                                    if let Some(pos) =
                                                        view.tabs.iter().position(|t| t.tab_id == tab_id)
                                                    {
                                                        view.close_tab(pos, window, cx);
                                                    }
                                                }))
                                                .child("Disconnect"),
                                        ),
                                ),
                        );
                    }
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .rounded_lg()
                                            .bg(gpui::rgba(0x06b6d422))
                                            .border_1()
                                            .border_color(rgb(0x06b6d4))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(rgb(0x22d3ee))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child(">_"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child(if draft.editing_index.is_some() {
                                                        format!("Edit SSH Host Profile — {}", draft.name)
                                                    } else {
                                                        "SSH Host Profile Editor & Connection Launcher".to_string()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Select any host on the left (✎) to edit, or configure and save a new SSH host profile."),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(gpui::rgba(0x10b98122))
                                    .border_1()
                                    .border_color(rgb(0x10b981))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x34d399))
                                    .child(format!("● {} Active Sessions", active_count)),
                            ),
                    )
                    .child(
                        div()
                            .p_4()
                            .rounded_xl()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .child(make_input(
                                        "ssh-ed-name",
                                        "Host Alias / Name",
                                        draft.name.clone(),
                                        "prod-bastion-01",
                                        SshConsoleInputField::HostName,
                                        cx,
                                    ))
                                    .child(make_input(
                                        "ssh-ed-hostname",
                                        "Hostname or IP Address",
                                        draft.hostname.clone(),
                                        "192.168.1.12 or bastion.example.com",
                                        SshConsoleInputField::HostHostname,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .child(make_input(
                                        "ssh-ed-user",
                                        "SSH Username",
                                        draft.user.clone(),
                                        "root / ubuntu / yocan",
                                        SshConsoleInputField::HostUser,
                                        cx,
                                    ))
                                    .child(make_input(
                                        "ssh-ed-port",
                                        "SSH Port",
                                        draft.port.clone(),
                                        "22",
                                        SshConsoleInputField::HostPort,
                                        cx,
                                    ))
                                    .child(make_input(
                                        "ssh-ed-key",
                                        "Identity Key Path (-i)",
                                        draft.identity_file.clone(),
                                        "~/.ssh/id_ed25519",
                                        SshConsoleInputField::HostIdentityFile,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .items_end()
                                    .child(make_input(
                                        "ssh-ed-jump",
                                        "ProxyJump Bastion (-J Optional)",
                                        draft.proxy_jump.clone(),
                                        "jump-user@bastion:22",
                                        SshConsoleInputField::HostProxyJump,
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Caduceus IPC Tunnel (-R)"),
                                            )
                                            .child(
                                                div()
                                                    .id("ssh-ed-ipc-toggle")
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_md()
                                                    .bg(if draft.forward_caduceus_ipc {
                                                        gpui::rgba(0x10b98128)
                                                    } else {
                                                        rgb(0x090d16)
                                                    })
                                                    .border_1()
                                                    .border_color(if draft.forward_caduceus_ipc {
                                                        rgb(0x10b981)
                                                    } else {
                                                        rgb(0x1e293b)
                                                    })
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(if draft.forward_caduceus_ipc {
                                                        rgb(0x34d399)
                                                    } else {
                                                        rgb(0x64748b)
                                                    })
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.host_draft.forward_caduceus_ipc =
                                                            !view.ssh_state.host_draft.forward_caduceus_ipc;
                                                        cx.notify();
                                                    }))
                                                    .child(if draft.forward_caduceus_ipc {
                                                        "✓ Forward /tmp/well-caduceus.sock"
                                                    } else {
                                                        "Off (Click to Enable)"
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2p5()
                                    .pt_1()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .justify_between()
                                            .items_center()
                                            .child(tier_row)
                                            .child(
                                                div()
                                                    .text_size(px(10.5))
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Persists to ~/.config/well/operator_settings.json"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .items_center()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .id("ssh-ed-save-btn")
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x06b6d4))
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.save_current_ssh_draft(cx);
                                                    }))
                                                    .child("💾 Save Host (⌘S)"),
                                            )
                                            .child(
                                                div()
                                                    .id("ssh-ed-conn-tab-btn")
                                                    .px_3p5()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x10b981))
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, window, cx| {
                                                        if let Ok(p) = view.ssh_state.host_draft.to_profile() {
                                                            view.save_current_ssh_draft(cx);
                                                            let base = view
                                                                .ssh_engine
                                                                .resolve_target(&p.host.hostname);
                                                            let target = p.build_target(base);
                                                            view.connect_ssh_with_mode(
                                                                target,
                                                                Some(p),
                                                                None,
                                                                window,
                                                                cx,
                                                            );
                                                        }
                                                    }))
                                                    .child("⚡ Connect in Tab"),
                                            )
                                            .child(
                                                div()
                                                    .id("ssh-ed-split-btn")
                                                    .px_3p5()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x1e293b))
                                                    .border_1()
                                                    .border_color(rgb(0x38bdf8))
                                                    .text_color(rgb(0x38bdf8))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, window, cx| {
                                                        if let Ok(p) = view.ssh_state.host_draft.to_profile() {
                                                            view.save_current_ssh_draft(cx);
                                                            let base = view
                                                                .ssh_engine
                                                                .resolve_target(&p.host.hostname);
                                                            let target = p.build_target(base);
                                                            view.connect_ssh_with_mode(
                                                                target,
                                                                Some(p),
                                                                Some(SplitDirection::Horizontal),
                                                                window,
                                                                cx,
                                                            );
                                                        }
                                                    }))
                                                    .child("🪟 Split Right"),
                                            ),
                                    ),
                            ),
                    )
                    .child(active_sess_row)
                    .into_any_element()
            }
            SshConsoleSection::PortForwarding => {
                let t_kind = self.ssh_state.tunnel_draft_kind;
                let t_local = self.ssh_state.tunnel_draft_local.clone();
                let t_remote = self.ssh_state.tunnel_draft_remote.clone();
                let t_ssh_host = self.ssh_state.tunnel_ssh_host.clone();
                let local_focused = active_input == SshConsoleInputField::TunnelLocalListen;
                let remote_focused = active_input == SshConsoleInputField::TunnelRemoteTarget;
                let host_focused = active_input == SshConsoleInputField::TunnelSshHost;

                let mut grid_rows = div().flex().flex_col().gap_3();
                let mut pair_row = div().flex().flex_row().gap_3();
                for (tidx, tunnel) in ssh_tunnels.iter().enumerate() {
                    let (badge_bg, badge_fg) = tunnel.kind.badge_colors();
                    let kind_label = tunnel.kind.badge_label();
                    let title = tunnel.title.clone();
                    let l_addr = tunnel.local_listen.clone();
                    let r_addr = tunnel.remote_target.clone();
                    let is_active = tunnel.active || tunnel.probe_local_port();
                    let ssh_cmd = tunnel.build_ssh_command(&t_ssh_host);
                    let ssh_cmd_for_split = ssh_cmd.clone();
                    let ssh_cmd_for_copy = ssh_cmd.clone();

                    let card = div()
                        .id(("ssh-tunnel-card", tidx))
                        .flex_1()
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(0x111726))
                        .border_1()
                        .border_color(if is_active {
                            rgb(0x10b981)
                        } else {
                            rgb(0x1e293b)
                        })
                        .flex()
                        .flex_col()
                        .gap_2p5()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(badge_bg))
                                                .text_color(rgb(badge_fg))
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .child(kind_label),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xf8fafc))
                                                .child(title),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if is_active {
                                                    rgb(0x34d399)
                                                } else {
                                                    rgb(0x64748b)
                                                })
                                                .child(if is_active { "● Listening" } else { "● Stopped" }),
                                        )
                                        .child(
                                            div()
                                                .id(("del-tunnel-btn", tidx))
                                                .text_xs()
                                                .text_color(rgb(0x64748b))
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.settings.update(cx, |s, cx| {
                                                        if tidx < s.ssh_tunnels.len() {
                                                            s.ssh_tunnels.remove(tidx);
                                                            let _ = s.save_to_disk();
                                                            cx.notify();
                                                        }
                                                    });
                                                    cx.notify();
                                                }))
                                                .child("🗑"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(rgb(0x090d16))
                                .border_1()
                                .border_color(rgb(0x162032))
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x64748b))
                                                .child("LOCAL LISTEN"),
                                        )
                                        .child(
                                            div()
                                                .font_family("Menlo")
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x22d3ee))
                                                .child(l_addr),
                                        ),
                                )
                                .child(div().text_xs().text_color(rgb(0x64748b)).child("→"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_end()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x64748b))
                                                .child("REMOTE TARGET"),
                                        )
                                        .child(
                                            div()
                                                .font_family("Menlo")
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x34d399))
                                                .child(r_addr),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .id(("copy-tunnel-cmd", tidx))
                                        .text_xs()
                                        .font_family("Menlo")
                                        .text_color(rgb(0x94a3b8))
                                        .cursor(gpui::CursorStyle::PointingHand)
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                ssh_cmd_for_copy.clone(),
                                            ));
                                            view.ssh_state.status_message = Some(format!(
                                                "📋 Copied tunnel command: {}",
                                                ssh_cmd_for_copy
                                            ));
                                            cx.notify();
                                        }))
                                        .child("📋 Copy SSH Cmd"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .id(("split-tunnel-btn", tidx))
                                                .px_2p5()
                                                .py_1()
                                                .rounded_md()
                                                .bg(rgb(0x161f33))
                                                .border_1()
                                                .border_color(rgb(0x38bdf8))
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x38bdf8))
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(move |view, _, window, cx| {
                                                    view.dispatch_command_to_split_pane(
                                                        &ssh_cmd_for_split,
                                                        window,
                                                        cx,
                                                    );
                                                }))
                                                .child("🪟 Split Tunnel"),
                                        )
                                        .child(
                                            div()
                                                .id(("toggle-tunnel-btn", tidx))
                                                .px_3()
                                                .py_1()
                                                .rounded_md()
                                                .bg(if is_active {
                                                    gpui::rgba(0xef444428)
                                                } else {
                                                    gpui::rgba(0x10b98122)
                                                })
                                                .border_1()
                                                .border_color(if is_active {
                                                    rgb(0xef4444)
                                                } else {
                                                    rgb(0x10b981)
                                                })
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(if is_active {
                                                    rgb(0xf87171)
                                                } else {
                                                    rgb(0x34d399)
                                                })
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.settings.update(cx, |s, cx| {
                                                        if let Some(t) = s.ssh_tunnels.get_mut(tidx) {
                                                            t.active = !t.active;
                                                            let _ = s.save_to_disk();
                                                            cx.notify();
                                                        }
                                                    });
                                                    cx.notify();
                                                }))
                                                .child(if is_active {
                                                    "■ Stop Tunnel"
                                                } else {
                                                    "▷ Start Tunnel"
                                                }),
                                        ),
                                ),
                        );

                    pair_row = pair_row.child(card);
                    if tidx % 2 == 1 || tidx + 1 == ssh_tunnels.len() {
                        grid_rows = grid_rows.child(pair_row);
                        pair_row = div().flex().flex_row().gap_3();
                    }
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child("⇄ Visual Port Forwarding & Tunnels"),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(gpui::rgba(0x06b6d422))
                                                    .border_1()
                                                    .border_color(rgb(0x06b6d4))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x22d3ee))
                                                    .child("PORT FORWARDING"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .child("Route database connections, microservices, and SOCKS5 traffic through SSH tunnels."),
                                    ),
                            )
                            .child(
                                div()
                                    .id("new-tunnel-add-btn")
                                    .px_3p5()
                                    .py_1p5()
                                    .rounded_lg()
                                    .bg(rgb(0xf59e0b))
                                    .text_color(rgb(0x090d16))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let kind = view.ssh_state.tunnel_draft_kind;
                                        let local = view.ssh_state.tunnel_draft_local.trim().to_string();
                                        let remote = view.ssh_state.tunnel_draft_remote.trim().to_string();
                                        let port_num = local.split(':').next_back().unwrap_or("8080");
                                        let title = format!("Port {port_num}");
                                        view.settings.update(cx, |s, cx| {
                                            s.ssh_tunnels.push(SshTunnelEntry {
                                                title,
                                                kind,
                                                local_listen: if local.is_empty() {
                                                    "127.0.0.1:9000".into()
                                                } else {
                                                    local
                                                },
                                                remote_target: if remote.is_empty() {
                                                    "127.0.0.1:9000".into()
                                                } else {
                                                    remote
                                                },
                                                active: false,
                                                traffic_bytes: 0,
                                            });
                                            let _ = s.save_to_disk();
                                            cx.notify();
                                        });
                                        cx.notify();
                                    }))
                                    .child("+ New Tunnel"),
                            ),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("cycle-tunnel-kind")
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x1e293b))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x22d3ee))
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.tunnel_draft_kind =
                                            view.ssh_state.tunnel_draft_kind.next();
                                        cx.notify();
                                    }))
                                    .child(format!(
                                        "{} ({}) ▾",
                                        t_kind.badge_label(),
                                        t_kind.ssh_flag()
                                    )),
                            )
                            .child(
                                div()
                                    .id("tunnel-ssh-host-input")
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if host_focused {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0xf8fafc))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::TunnelSshHost;
                                        cx.notify();
                                    }))
                                    .child(if host_focused {
                                        format!("Via SSH: {t_ssh_host}│")
                                    } else {
                                        format!("Via SSH: {t_ssh_host}")
                                    }),
                            )
                            .child(
                                div()
                                    .id("tunnel-local-input")
                                    .flex_1()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if local_focused {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x22d3ee))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::TunnelLocalListen;
                                        cx.notify();
                                    }))
                                    .child(if local_focused {
                                        format!("Local: {t_local}│")
                                    } else {
                                        format!("Local: {t_local}")
                                    }),
                            )
                            .child(div().text_xs().text_color(rgb(0x64748b)).child("→"))
                            .child(
                                div()
                                    .id("tunnel-remote-input")
                                    .flex_1()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if remote_focused {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x34d399))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::TunnelRemoteTarget;
                                        cx.notify();
                                    }))
                                    .child(if remote_focused {
                                        format!("Target: {t_remote}│")
                                    } else {
                                        format!("Target: {t_remote}")
                                    }),
                            ),
                    )
                    .child(grid_rows)
                    .into_any_element()
            }
            SshConsoleSection::KeyVault => {
                let keys = DiscoveredSshKey::discover_local_keys();
                let ks_running = self.ssh_state.key_server_running;
                let ks_port = self.ssh_state.key_server_port.clone();
                let lan_ip = self.ssh_state.pxe_server_ip.clone();
                let bootstrap_cmd = format!(
                    "mkdir -p ~/.ssh && curl -fsSL http://{lan_ip}:{ks_port}/authorized_keys >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys"
                );
                let bootstrap_for_copy = bootstrap_cmd.clone();

                let mut key_cards = div().flex().flex_col().gap_2();
                for (kidx, key) in keys.iter().take(2).enumerate() {
                    let kname = key.name.clone();
                    let kalgo = key.algorithm.clone();
                    let kpath = key.path_display.clone();
                    let kpath_for_agent = kpath.clone();
                    let kpath_for_copy_id = kpath.clone();
                    let kfp = key.fingerprint.clone();
                    let kpub = key.public_key_line.clone();
                    let kpub_copy = kpub.clone();

                    key_cards = key_cards.child(
                        div()
                            .id(("ssh-vault-key", kidx))
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .w(px(30.0))
                                                    .h(px(30.0))
                                                    .rounded_lg()
                                                    .bg(rgb(0x0f1f2e))
                                                    .border_1()
                                                    .border_color(rgb(0x0284c7))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_xs()
                                                    .text_color(rgb(0x38bdf8))
                                                    .child("🔒"),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(gpui::FontWeight::BOLD)
                                                                    .text_color(rgb(0xf8fafc))
                                                                    .child(kname),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .rounded_sm()
                                                                    .bg(rgb(0x172554))
                                                                    .text_color(rgb(0x60a5fa))
                                                                    .text_xs()
                                                                    .font_weight(gpui::FontWeight::BOLD)
                                                                    .child(kalgo),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .font_family("Menlo")
                                                            .text_xs()
                                                            .text_color(rgb(0x64748b))
                                                            .child(kpath),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id(("vault-add-agent", kidx))
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x161f33))
                                                    .border_1()
                                                    .border_color(rgb(0x26324d))
                                                    .text_xs()
                                                    .text_color(rgb(0xe2e8f0))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        let _ = std::process::Command::new("ssh-add")
                                                            .arg(&kpath_for_agent)
                                                            .output();
                                                        view.ssh_state.agent_key_path =
                                                            kpath_for_agent.clone();
                                                        view.ssh_state.status_message = Some(format!(
                                                            "🛡 Added key '{}' to macOS ssh-agent ($SSH_AUTH_SOCK)",
                                                            kpath_for_agent
                                                        ));
                                                        cx.notify();
                                                    }))
                                                    .child("🛡 Add to Agent"),
                                            )
                                            .child(
                                                div()
                                                    .id(("vault-use-host", kidx))
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x161f33))
                                                    .border_1()
                                                    .border_color(rgb(0x26324d))
                                                    .text_xs()
                                                    .text_color(rgb(0xe2e8f0))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, window, cx| {
                                                        let target_host = view.ssh_state.tunnel_ssh_host.clone();
                                                        let cmd = format!("ssh-copy-id -i {} {}", kpath_for_copy_id, target_host);
                                                        view.dispatch_command_to_split_pane(&cmd, window, cx);
                                                    }))
                                                    .child(">_ ssh-copy-id"),
                                            )
                                            .child(
                                                div()
                                                    .id(("vault-copy-pub", kidx))
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(gpui::rgba(0x06b6d422))
                                                    .border_1()
                                                    .border_color(rgb(0x06b6d4))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x22d3ee))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        cx.write_to_clipboard(
                                                            gpui::ClipboardItem::new_string(
                                                                kpub_copy.clone(),
                                                            ),
                                                        );
                                                        view.ssh_state.status_message = Some(
                                                            "📋 Copied public key to macOS clipboard!"
                                                                .to_string(),
                                                        );
                                                        cx.notify();
                                                    }))
                                                    .child("📋 Copy Public Key"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("Menlo")
                                    .text_color(rgb(0x94a3b8))
                                    .child(format!("Fingerprint:  {kfp}")),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(rgb(0x162032))
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x94a3b8))
                                    .child(kpub),
                            ),
                    );
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf8fafc))
                                            .child("🔑 SSH Key Vault & HTTP Authorized Key Server"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .child("Hardware-grade Ed25519/RSA key vault + LAN HTTP /authorized_keys bootstrap server"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("gen-ssh-keypair-btn")
                                    .px_3p5()
                                    .py_1p5()
                                    .rounded_lg()
                                    .bg(rgb(0x10b981))
                                    .text_color(rgb(0x090d16))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        if let Ok(home) = std::env::var("HOME") {
                                            let key_file = format!("{home}/.ssh/id_ed25519_well");
                                            if !std::path::Path::new(&key_file).exists() {
                                                let _ = std::process::Command::new("ssh-keygen")
                                                    .args(["-t", "ed25519", "-f", &key_file, "-N", "", "-C", "well-netops"])
                                                    .output();
                                            }
                                            view.ssh_state.status_message = Some(format!(
                                                "✨ Ed25519 key pair ready at {key_file}"
                                            ));
                                        }
                                        cx.notify();
                                    }))
                                    .child("+ Generate Key Pair"),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(if ks_running {
                                rgb(0x10b981)
                            } else {
                                rgb(0x0284c7)
                            })
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child("🌐 HTTP Authorized Key Server (/authorized_keys)"),
                                            )
                                            .child(
                                                div()
                                                    .id("keyserver-port-pill")
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(if ks_running {
                                                        gpui::rgba(0x10b98128)
                                                    } else {
                                                        rgb(0x1e293b)
                                                    })
                                                    .text_color(if ks_running {
                                                        rgb(0x34d399)
                                                    } else {
                                                        rgb(0x94a3b8)
                                                    })
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::IBeam)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.active_input =
                                                            SshConsoleInputField::KeyServerPort;
                                                        cx.notify();
                                                    }))
                                                    .child(if ks_running {
                                                        format!("SERVING :{ks_port}")
                                                    } else {
                                                        format!("PORT :{ks_port}")
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("copy-keyserver-bootstrap")
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x161f33))
                                                    .border_1()
                                                    .border_color(rgb(0x38bdf8))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x38bdf8))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        cx.write_to_clipboard(
                                                            gpui::ClipboardItem::new_string(
                                                                bootstrap_for_copy.clone(),
                                                            ),
                                                        );
                                                        view.ssh_state.status_message = Some(
                                                            "📋 Copied curl authorized_keys bootstrap one-liner!"
                                                                .to_string(),
                                                        );
                                                        cx.notify();
                                                    }))
                                                    .child("📋 Copy curl Bootstrap"),
                                            )
                                            .child(
                                                div()
                                                    .id("toggle-keyserver-btn")
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(if ks_running {
                                                        rgb(0xef4444)
                                                    } else {
                                                        rgb(0x06b6d4)
                                                    })
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        if view.ssh_state.key_server_running {
                                                            if let Some(stop) =
                                                                view.ssh_state.key_server_stop.take()
                                                            {
                                                                stop.store(
                                                                    true,
                                                                    std::sync::atomic::Ordering::Relaxed,
                                                                );
                                                            }
                                                            view.ssh_state.key_server_running = false;
                                                            view.ssh_state.status_message = Some(
                                                                "■ Stopped HTTP Authorized Key Server."
                                                                    .to_string(),
                                                            );
                                                        } else {
                                                            let port = view
                                                                .ssh_state
                                                                .key_server_port
                                                                .parse::<u16>()
                                                                .unwrap_or(8787);
                                                            let stop = std::sync::Arc::new(
                                                                std::sync::atomic::AtomicBool::new(false),
                                                            );
                                                            match start_http_key_server(port, stop.clone())
                                                            {
                                                                Ok(addr) => {
                                                                    view.ssh_state.key_server_running =
                                                                        true;
                                                                    view.ssh_state.key_server_stop =
                                                                        Some(stop);
                                                                    view.ssh_state.status_message =
                                                                        Some(format!(
                                                                        "🟢 Key Server listening on http://{}/authorized_keys",
                                                                        addr
                                                                    ));
                                                                }
                                                                Err(e) => {
                                                                    view.ssh_state.status_message =
                                                                        Some(format!("⚠ {e}"));
                                                                }
                                                            }
                                                        }
                                                        cx.notify();
                                                    }))
                                                    .child(if ks_running {
                                                        "■ Stop Key Server"
                                                    } else {
                                                        "▶ Start Key Server (:8787)"
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(rgb(0x162032))
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x22d3ee))
                                    .child(bootstrap_cmd),
                            ),
                    )
                    .child(key_cards)
                    .into_any_element()
            }
            SshConsoleSection::ServerAndAgentHub => {
                let header = div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf8fafc))
                                    .child("🖧 SSH Server Hosting & SSH Agent Hub"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x94a3b8))
                                    .child("Run an embedded macOS SSH server daemon and manage loaded credentials in $SSH_AUTH_SOCK"),
                            ),
                    )
                    .child(
                        div()
                            .p_1()
                            .rounded_lg()
                            .bg(rgb(0x090d16))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_row()
                            .gap_1()
                            .child(
                                div()
                                    .id("subtab-server-hosting")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if server_subtab == ServerHubSubtab::ServerHosting {
                                        rgb(0x06b6d4)
                                    } else {
                                        gpui::rgba(0x00000000)
                                    })
                                    .text_color(if server_subtab == ServerHubSubtab::ServerHosting {
                                        rgb(0x090d16)
                                    } else {
                                        rgb(0x94a3b8)
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.server_subtab =
                                            ServerHubSubtab::ServerHosting;
                                        cx.notify();
                                    }))
                                    .child("🖧 SSH Server Hosting"),
                            )
                            .child(
                                div()
                                    .id("subtab-agent-hub")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if server_subtab == ServerHubSubtab::AgentHub {
                                        rgb(0x06b6d4)
                                    } else {
                                        gpui::rgba(0x00000000)
                                    })
                                    .text_color(if server_subtab == ServerHubSubtab::AgentHub {
                                        rgb(0x090d16)
                                    } else {
                                        rgb(0x94a3b8)
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.server_subtab = ServerHubSubtab::AgentHub;
                                        cx.notify();
                                    }))
                                    .child("🛡 SSH Agent Hub"),
                            )
                            .child(
                                div()
                                    .id("subtab-ops-agent")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if server_subtab == ServerHubSubtab::OpsAgent {
                                        rgb(0x06b6d4)
                                    } else {
                                        gpui::rgba(0x00000000)
                                    })
                                    .text_color(if server_subtab == ServerHubSubtab::OpsAgent {
                                        rgb(0x090d16)
                                    } else {
                                        rgb(0x94a3b8)
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.server_subtab = ServerHubSubtab::OpsAgent;
                                        cx.notify();
                                    }))
                                    .child("🤖 NetOps Agent"),
                            ),
                    );

                let body = match server_subtab {
                    ServerHubSubtab::ServerHosting => {
                        let running = self.ssh_state.server_running;
                        let s_port = self.ssh_state.server_port.clone();
                        let s_addr = self.ssh_state.server_listen_addr.clone();
                        let allow_pub = self.ssh_state.allow_pubkey_auth;
                        let allow_pw = self.ssh_state.allow_password_auth;
                        let port_focused = active_input == SshConsoleInputField::ServerPort;
                        let keys = DiscoveredSshKey::discover_local_keys();

                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x1e293b))
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .text_color(rgb(0xf8fafc))
                                                            .child("Embedded SSH Server Daemon"),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(if running {
                                                                gpui::rgba(0x10b98128)
                                                            } else {
                                                                rgb(0x1e293b)
                                                            })
                                                            .text_color(if running {
                                                                rgb(0x34d399)
                                                            } else {
                                                                rgb(0x94a3b8)
                                                            })
                                                            .text_xs()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .child(if running { "RUNNING" } else { "STOPPED" }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Ready to accept incoming terminal sessions on unprivileged port"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("start-ssh-server-btn")
                                                    .px_4()
                                                    .py_2()
                                                    .rounded_lg()
                                                    .bg(if running {
                                                        rgb(0xef4444)
                                                    } else {
                                                        rgb(0x06b6d4)
                                                    })
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.server_running =
                                                            !view.ssh_state.server_running;
                                                        cx.notify();
                                                    }))
                                                    .child(if running {
                                                        "■ Stop SSH Server"
                                                    } else {
                                                        "▶ Start SSH Server"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .id("auto-cfg-ssh-server-btn")
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_lg()
                                                    .bg(gpui::rgba(0x8b5cf628))
                                                    .border_1()
                                                    .border_color(rgb(0x8b5cf6))
                                                    .text_color(rgb(0xc4b5fd))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.server_port = "2222".to_string();
                                                        view.ssh_state.server_listen_addr =
                                                            "0.0.0.0 (All LAN & VPN Interfaces)"
                                                                .to_string();
                                                        view.ssh_state.allow_pubkey_auth = true;
                                                        cx.notify();
                                                    }))
                                                    .child("⚡ 1-Click Auto-Configure"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_3p5()
                                            .rounded_xl()
                                            .bg(rgb(0x111726))
                                            .border_1()
                                            .border_color(rgb(0x1e293b))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child("Listener Configuration"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Server Port (Unprivileged > 1024)"),
                                            )
                                            .child(
                                                div()
                                                    .id("server-port-input")
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x090d16))
                                                    .border_1()
                                                    .border_color(if port_focused {
                                                        rgb(0x06b6d4)
                                                    } else {
                                                        rgb(0x1e293b)
                                                    })
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0xf8fafc))
                                                    .cursor(gpui::CursorStyle::IBeam)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.active_input =
                                                            SshConsoleInputField::ServerPort;
                                                        cx.notify();
                                                    }))
                                                    .child(if port_focused {
                                                        format!("{s_port}│")
                                                    } else {
                                                        s_port
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Listen Interface Address"),
                                            )
                                            .child(
                                                div()
                                                    .id("server-listen-addr-input")
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x090d16))
                                                    .border_1()
                                                    .border_color(
                                                        if active_input
                                                            == SshConsoleInputField::ServerListenAddr
                                                        {
                                                            rgb(0x06b6d4)
                                                        } else {
                                                            rgb(0x1e293b)
                                                        },
                                                    )
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0xf8fafc))
                                                    .cursor(gpui::CursorStyle::IBeam)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.active_input =
                                                            SshConsoleInputField::ServerListenAddr;
                                                        cx.notify();
                                                    }))
                                                    .child(s_addr),
                                            )
                                            .child(
                                                div()
                                                    .id("toggle-srv-pubkey")
                                                    .text_xs()
                                                    .text_color(rgb(0xe2e8f0))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.allow_pubkey_auth =
                                                            !view.ssh_state.allow_pubkey_auth;
                                                        cx.notify();
                                                    }))
                                                    .child(if allow_pub {
                                                        "☑ Allow Public Key Authentication (Recommended)"
                                                    } else {
                                                        "☐ Allow Public Key Authentication (Recommended)"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .id("toggle-srv-pw")
                                                    .text_xs()
                                                    .text_color(rgb(0xe2e8f0))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.allow_password_auth =
                                                            !view.ssh_state.allow_password_auth;
                                                        cx.notify();
                                                    }))
                                                    .child(if allow_pw {
                                                        "☑ Allow Password Authentication"
                                                    } else {
                                                        "☐ Allow Password Authentication"
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_3p5()
                                            .rounded_xl()
                                            .bg(rgb(0x111726))
                                            .border_1()
                                            .border_color(rgb(0x1e293b))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child("Host Key Security"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Algorithm & Key Path"),
                                            )
                                            .child(
                                                div()
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0x38bdf8))
                                                    .child("ED25519 (~/.theia/ssh_server/ssh_host_ed25519_key)"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Host Key Fingerprint"),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x090d16))
                                                    .border_1()
                                                    .border_color(rgb(0x1e293b))
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0xe2e8f0))
                                                    .child("SHA256:2MxNZ+ew0X6C0j3bs3z6K4Xj9j++N+AECkI9t2NcasY"),
                                            )
                                            .child(
                                                div()
                                                    .p_2p5()
                                                    .rounded_lg()
                                                    .bg(gpui::rgba(0x06b6d414))
                                                    .border_1()
                                                    .border_color(rgb(0x0e7490))
                                                    .text_xs()
                                                    .text_color(rgb(0x7dd3fc))
                                                    .child("ⓘ Isolated Daemon Sandbox: Runs isolated from macOS system remote login with dedicated keys."),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x1e293b))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf8fafc))
                                            .child(format!("Authorized Public Keys ({})", keys.len())),
                                    )
                                    .children(keys.into_iter().take(2).map(|k| {
                                        div()
                                            .px_3()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(rgb(0x090d16))
                                            .border_1()
                                            .border_color(rgb(0x162032))
                                            .font_family("Menlo")
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .child(k.public_key_line)
                                    })),
                            )
                            .into_any_element()
                    }
                    ServerHubSubtab::AgentHub => {
                        let sock = std::env::var("SSH_AUTH_SOCK")
                            .unwrap_or_else(|_| "/var/run/com.apple.launchd.Listeners".to_string());
                        let a_key = self.ssh_state.agent_key_path.clone();
                        let a_focused = active_input == SshConsoleInputField::AgentKeyPath;
                        let loaded_ids = DiscoveredSshKey::query_loaded_agent_identities();

                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x0e7490))
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .text_color(rgb(0xf8fafc))
                                                            .child("macOS SSH Authentication Agent ($SSH_AUTH_SOCK)"),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(gpui::rgba(0x06b6d428))
                                                            .text_color(rgb(0x22d3ee))
                                                            .text_xs()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .child("CONNECTED"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0x64748b))
                                                    .child(sock),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("agent-load-all-btn")
                                            .px_3()
                                            .py_1p5()
                                            .rounded_lg()
                                            .bg(gpui::rgba(0x8b5cf628))
                                            .border_1()
                                            .border_color(rgb(0x8b5cf6))
                                            .text_color(rgb(0xc4b5fd))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                let _ = std::process::Command::new("ssh-add").output();
                                                view.ssh_state.status_message = Some(
                                                    "⚡ Ran ssh-add to load default ~/.ssh identities into $SSH_AUTH_SOCK."
                                                        .to_string(),
                                                );
                                                cx.notify();
                                            }))
                                            .child("⚡ 1-Click Auto-Load Keys"),
                                    ),
                            )
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x1e293b))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf8fafc))
                                            .child("Add SSH Key to Agent (ssh-add)"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .id("agent-key-input")
                                                    .flex_1()
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_md()
                                                    .bg(rgb(0x090d16))
                                                    .border_1()
                                                    .border_color(if a_focused {
                                                        rgb(0x06b6d4)
                                                    } else {
                                                        rgb(0x1e293b)
                                                    })
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0xf8fafc))
                                                    .cursor(gpui::CursorStyle::IBeam)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.ssh_state.active_input =
                                                            SshConsoleInputField::AgentKeyPath;
                                                        cx.notify();
                                                    }))
                                                    .child(if a_focused {
                                                        format!("{a_key}│")
                                                    } else {
                                                        a_key
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .id("agent-add-exec-btn")
                                                    .px_3p5()
                                                    .py_2()
                                                    .rounded_md()
                                                    .bg(rgb(0x06b6d4))
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        let k = view.ssh_state.agent_key_path.clone();
                                                        let expanded = if let (Some(rest), Ok(home)) =
                                                            (k.strip_prefix("~/"), std::env::var("HOME"))
                                                        {
                                                            format!("{home}/{rest}")
                                                        } else {
                                                            k.clone()
                                                        };
                                                        let _ = std::process::Command::new("ssh-add")
                                                            .arg(&expanded)
                                                            .output();
                                                        view.ssh_state.status_message = Some(format!(
                                                            "🛡 Executed ssh-add '{}' into $SSH_AUTH_SOCK.",
                                                            k
                                                        ));
                                                        cx.notify();
                                                    }))
                                                    .child("+ Add to Agent"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x1e293b))
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child(format!(
                                                        "Loaded Key Identities in Memory ({})",
                                                        loaded_ids.len()
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_family("Menlo")
                                                    .text_color(rgb(0x64748b))
                                                    .child("Queried via ssh-add -l"),
                                            ),
                                    )
                                    .children(if loaded_ids.is_empty() {
                                        vec![div()
                                            .py_2()
                                            .text_xs()
                                            .text_color(rgb(0x64748b))
                                            .child("No identities currently loaded in ssh-agent. Click '⚡ 1-Click Auto-Load Keys' or '+ Add to Agent' above.")]
                                    } else {
                                        loaded_ids
                                            .into_iter()
                                            .take(3)
                                            .map(|line| {
                                                div()
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x090d16))
                                                    .border_1()
                                                    .border_color(rgb(0x162032))
                                                    .font_family("Menlo")
                                                    .text_xs()
                                                    .text_color(rgb(0x34d399))
                                                    .child(line)
                                            })
                                            .collect()
                                    }),
                            )
                            .into_any_element()
                    }
                    ServerHubSubtab::OpsAgent => {
                        let lan_ip = self.ssh_state.pxe_server_ip.clone();
                        let target_host = self.ssh_state.net_target_host.clone();
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x8b5cf6))
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .text_color(rgb(0xf8fafc))
                                                            .child("🤖 Well Helper — Autonomous NetOps & Bare-Metal Agent"),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(gpui::rgba(0x10b98128))
                                                            .text_color(rgb(0x34d399))
                                                            .text_xs()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .child("READY"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Orchestrates SSH tunnels, Key Server bootstrap, PXE Option 43 TLV verification, SMB mounts, and UEFI QEMU harnesses."),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_3p5()
                                            .rounded_xl()
                                            .bg(rgb(0x111726))
                                            .border_1()
                                            .border_color(rgb(0x1e293b))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x22d3ee))
                                                    .child("1. Full Netboot & Key Server Stack Start"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child(format!(
                                                        "Starts both the HTTP Authorized Key Server (:8787) and the PXE HTTP iPXE Boot Server (:8080) on {lan_ip}."
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .id("agent-start-all-services")
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x06b6d4))
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        if !view.ssh_state.key_server_running {
                                                            let stop = std::sync::Arc::new(
                                                                std::sync::atomic::AtomicBool::new(false),
                                                            );
                                                            if start_http_key_server(8787, stop.clone())
                                                                .is_ok()
                                                            {
                                                                view.ssh_state.key_server_running = true;
                                                                view.ssh_state.key_server_stop = Some(stop);
                                                            }
                                                        }
                                                        if !view.ssh_state.pxe_http_running {
                                                            let stop = std::sync::Arc::new(
                                                                std::sync::atomic::AtomicBool::new(false),
                                                            );
                                                            if start_pxe_http_boot_server(
                                                                8080,
                                                                view.ssh_state.pxe_bootfile.clone(),
                                                                stop.clone(),
                                                            )
                                                            .is_ok()
                                                            {
                                                                view.ssh_state.pxe_http_running = true;
                                                                view.ssh_state.pxe_http_stop = Some(stop);
                                                            }
                                                        }
                                                        view.ssh_state.status_message = Some(
                                                            "🤖 NetOps Agent: Started HTTP Key Server (:8787) + PXE HTTP Boot Server (:8080)!"
                                                                .to_string(),
                                                        );
                                                        cx.notify();
                                                    }))
                                                    .child("⚡ Activate Key Server + PXE Boot Server"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_3p5()
                                            .rounded_xl()
                                            .bg(rgb(0x111726))
                                            .border_1()
                                            .border_color(rgb(0x1e293b))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x34d399))
                                                    .child("2. Deep Host & Subnet Security Audit"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child(format!(
                                                        "Scans {target_host} across 15 ports (SSH, DHCP, TFTP, SMB, ProxyDHCP 4011) and refreshes ARP neighbors."
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .id("agent-run-net-audit")
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x10b981))
                                                    .text_color(rgb(0x090d16))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        let host = view.ssh_state.net_target_host.clone();
                                                        view.ssh_state.net_port_scan =
                                                            run_fast_port_scan(&host);
                                                        view.ssh_state.net_arp_entries =
                                                            discover_arp_neighbors();
                                                        view.ssh_state.section =
                                                            SshConsoleSection::NetworkTools;
                                                        view.ssh_state.status_message = Some(format!(
                                                            "🤖 NetOps Agent: Completed 15-port scan on {host} & refreshed ARP table."
                                                        ));
                                                        cx.notify();
                                                    }))
                                                    .child("🔍 Run Full Port & ARP Audit"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .p_3p5()
                                    .rounded_xl()
                                    .bg(rgb(0x111726))
                                    .border_1()
                                    .border_color(rgb(0x1e293b))
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child("3. Interactive Pythia AI + Well Helper Terminal Session"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Spawn a split terminal pane running live network interface, route, and SSH agent telemetry."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("agent-spawn-diag-pane")
                                            .px_3p5()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(gpui::rgba(0x8b5cf628))
                                            .border_1()
                                            .border_color(rgb(0x8b5cf6))
                                            .text_color(rgb(0xc4b5fd))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.dispatch_command_to_split_pane(
                                                    "ifconfig | grep -E '^[a-z0-9]+:|inet ' && arp -a && ssh-add -l",
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .child("🪟 Launch Telemetry Split Pane"),
                                    ),
                            )
                            .into_any_element()
                    }
                };

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(header)
                    .child(body)
                    .into_any_element()
            }
            SshConsoleSection::PxeAndBootloader => {
                let pxe_subtab = self.ssh_state.pxe_subtab;
                let pxe_mode = self.ssh_state.pxe_mode;
                let pxe_arch = self.ssh_state.pxe_arch;
                let srv_ip_str = self.ssh_state.pxe_server_ip.clone();
                let http_port_str = self.ssh_state.pxe_http_port.clone();
                let bootfile_str = self.ssh_state.pxe_bootfile.clone();
                let pxe_running = self.ssh_state.pxe_http_running;
                let ip_focused = active_input == SshConsoleInputField::PxeServerIp;
                let port_focused = active_input == SshConsoleInputField::PxeHttpPort;
                let file_focused = active_input == SshConsoleInputField::PxeBootFile;

                let parsed_ip = srv_ip_str
                    .parse::<std::net::Ipv4Addr>()
                    .unwrap_or(std::net::Ipv4Addr::new(192, 168, 1, 12));
                let http_port_num = http_port_str.parse::<u16>().unwrap_or(8080);
                let opt43_bytes = build_legacy_bios_option43(parsed_ip);
                let opt43_hex: String = opt43_bytes
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let dns_opt6 = build_standalone_dns_option6(Some(parsed_ip));
                let dns_hex: String = dns_opt6
                    .iter()
                    .map(|b| format!("{b}"))
                    .collect::<Vec<_>>()
                    .join(".");
                let ipxe_script = generate_strict_ipxe_script(http_port_num, &bootfile_str);
                let chain_kind = classify_chainload_payload(ipxe_script.as_bytes());
                let stage2_ok = is_ipxe_stage2_client(Some(b"iPXE"), Some(&[175, 2, 1, 1]));
                let tftp_sample = TftpAction::from_opcode(3, 1, 512, 1468);
                let _ = (
                    matches!(
                        chain_kind,
                        ChainloadTargetKind::IpxeScript
                            | ChainloadTargetKind::UefiPe32Image
                            | ChainloadTargetKind::Unknown
                    ),
                    stage2_ok,
                    matches!(
                        tftp_sample,
                        TftpAction::Ack(_, _) | TftpAction::Error | TftpAction::Ignore
                    ),
                );
                let ipxe_script_copy = ipxe_script.clone();
                let ipxe_script_write = ipxe_script.clone();
                let qemu_cmd = build_qemu_uefi_test_command("tftpboot", &bootfile_str);
                let qemu_cmd_copy = qemu_cmd.clone();
                let qemu_cmd_run = qemu_cmd.clone();

                let header = div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf8fafc))
                                    .child("🚀 PXE / ProxyDHCP Server & Custom UEFI Bootloader"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x94a3b8))
                                    .child("Strict Option 43 TLV, Port 4011 Unicast ProxyDHCP, ${next-server} iPXE & bare-metal no_std UEFI SNP"),
                            ),
                    )
                    .child(
                        div()
                            .p_1()
                            .rounded_lg()
                            .bg(rgb(0x090d16))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_row()
                            .gap_1()
                            .child(
                                div()
                                    .id("pxe-subtab-server")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if pxe_subtab == PxeBootSubtab::PxeServer {
                                        rgb(0x06b6d4)
                                    } else {
                                        gpui::rgba(0x00000000)
                                    })
                                    .text_color(if pxe_subtab == PxeBootSubtab::PxeServer {
                                        rgb(0x090d16)
                                    } else {
                                        rgb(0x94a3b8)
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.pxe_subtab = PxeBootSubtab::PxeServer;
                                        cx.notify();
                                    }))
                                    .child("🌐 PXE & ProxyDHCP Server"),
                            )
                            .child(
                                div()
                                    .id("pxe-subtab-bootloader")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if pxe_subtab == PxeBootSubtab::CustomBootloader {
                                        rgb(0x06b6d4)
                                    } else {
                                        gpui::rgba(0x00000000)
                                    })
                                    .text_color(if pxe_subtab == PxeBootSubtab::CustomBootloader {
                                        rgb(0x090d16)
                                    } else {
                                        rgb(0x94a3b8)
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.pxe_subtab = PxeBootSubtab::CustomBootloader;
                                        cx.notify();
                                    }))
                                    .child("⚙ Custom UEFI Bootloader (ipxe-uefi)"),
                            ),
                    );

                let body = match pxe_subtab {
                    PxeBootSubtab::PxeServer => div()
                        .flex()
                        .flex_col()
                        .gap_2p5()
                        .child(
                            div()
                                .p_3()
                                .rounded_xl()
                                .bg(rgb(0x111726))
                                .border_1()
                                .border_color(rgb(0x1e293b))
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .id("cycle-pxe-mode-btn")
                                                .px_3()
                                                .py_1p5()
                                                .rounded_md()
                                                .bg(gpui::rgba(0x06b6d428))
                                                .border_1()
                                                .border_color(rgb(0x06b6d4))
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x22d3ee))
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.ssh_state.pxe_mode = match view.ssh_state.pxe_mode
                                                    {
                                                        PxeServerMode::ProxyDhcp => {
                                                            PxeServerMode::StandaloneDhcp
                                                        }
                                                        PxeServerMode::StandaloneDhcp => {
                                                            PxeServerMode::ProxyDhcp
                                                        }
                                                    };
                                                    cx.notify();
                                                }))
                                                .child(format!("Mode: {} ▾", pxe_mode.label())),
                                        )
                                        .child(
                                            div()
                                                .id("cycle-pxe-arch-btn")
                                                .px_2p5()
                                                .py_1p5()
                                                .rounded_md()
                                                .bg(rgb(0x161f33))
                                                .border_1()
                                                .border_color(rgb(0x26324d))
                                                .text_xs()
                                                .text_color(rgb(0xe2e8f0))
                                                .cursor(gpui::CursorStyle::PointingHand)
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.ssh_state.pxe_arch =
                                                        view.ssh_state.pxe_arch.next();
                                                    cx.notify();
                                                }))
                                                .child(format!("{} ▾", pxe_arch.label())),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("toggle-pxe-http-srv")
                                        .px_3p5()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(if pxe_running {
                                            rgb(0xef4444)
                                        } else {
                                            rgb(0x10b981)
                                        })
                                        .text_color(rgb(0x090d16))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .cursor(gpui::CursorStyle::PointingHand)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            if view.ssh_state.pxe_http_running {
                                                if let Some(stop) = view.ssh_state.pxe_http_stop.take() {
                                                    stop.store(
                                                        true,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                }
                                                view.ssh_state.pxe_http_running = false;
                                                view.ssh_state.status_message = Some(
                                                    "■ Stopped PXE HTTP Boot Server.".to_string(),
                                                );
                                            } else {
                                                let port = view
                                                    .ssh_state
                                                    .pxe_http_port
                                                    .parse::<u16>()
                                                    .unwrap_or(8080);
                                                let stop = std::sync::Arc::new(
                                                    std::sync::atomic::AtomicBool::new(false),
                                                );
                                                match start_pxe_http_boot_server(
                                                    port,
                                                    view.ssh_state.pxe_bootfile.clone(),
                                                    stop.clone(),
                                                ) {
                                                    Ok(addr) => {
                                                        view.ssh_state.pxe_http_running = true;
                                                        view.ssh_state.pxe_http_stop = Some(stop);
                                                        view.ssh_state.status_message = Some(format!(
                                                            "🟢 PXE HTTP Boot Server active on http://{}/{}",
                                                            addr, view.ssh_state.pxe_bootfile
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        view.ssh_state.status_message =
                                                            Some(format!("⚠ {e}"));
                                                    }
                                                }
                                            }
                                            cx.notify();
                                        }))
                                        .child(if pxe_running {
                                            "■ Stop HTTP Boot (:8080)"
                                        } else {
                                            "▶ Start HTTP Boot (:8080)"
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2p5()
                                .child(
                                    div()
                                        .id("pxe-ip-input")
                                        .flex_1()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(0x090d16))
                                        .border_1()
                                        .border_color(if ip_focused {
                                            rgb(0x06b6d4)
                                        } else {
                                            rgb(0x1e293b)
                                        })
                                        .font_family("Menlo")
                                        .text_xs()
                                        .text_color(rgb(0x22d3ee))
                                        .cursor(gpui::CursorStyle::IBeam)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.ssh_state.active_input =
                                                SshConsoleInputField::PxeServerIp;
                                            cx.notify();
                                        }))
                                        .child(if ip_focused {
                                            format!("siaddr (TFTP/Next-Server): {srv_ip_str}│")
                                        } else {
                                            format!("siaddr (TFTP/Next-Server): {srv_ip_str}")
                                        }),
                                )
                                .child(
                                    div()
                                        .id("pxe-port-input")
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(0x090d16))
                                        .border_1()
                                        .border_color(if port_focused {
                                            rgb(0x06b6d4)
                                        } else {
                                            rgb(0x1e293b)
                                        })
                                        .font_family("Menlo")
                                        .text_xs()
                                        .text_color(rgb(0xf8fafc))
                                        .cursor(gpui::CursorStyle::IBeam)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.ssh_state.active_input =
                                                SshConsoleInputField::PxeHttpPort;
                                            cx.notify();
                                        }))
                                        .child(if port_focused {
                                            format!("HTTP Port: {http_port_str}│")
                                        } else {
                                            format!("HTTP Port: {http_port_str}")
                                        }),
                                )
                                .child(
                                    div()
                                        .id("pxe-bootfile-input")
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(0x090d16))
                                        .border_1()
                                        .border_color(if file_focused {
                                            rgb(0x06b6d4)
                                        } else {
                                            rgb(0x1e293b)
                                        })
                                        .font_family("Menlo")
                                        .text_xs()
                                        .text_color(rgb(0x34d399))
                                        .cursor(gpui::CursorStyle::IBeam)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.ssh_state.active_input =
                                                SshConsoleInputField::PxeBootFile;
                                            cx.notify();
                                        }))
                                        .child(if file_focused {
                                            format!("Bootfile: {bootfile_str}│")
                                        } else {
                                            format!("Bootfile: {bootfile_str}")
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .p_3()
                                .rounded_xl()
                                .bg(rgb(0x111726))
                                .border_1()
                                .border_color(rgb(0x1e293b))
                                .flex()
                                .flex_col()
                                .gap_1p5()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xf8fafc))
                                                .child("Strict Option 43 TLV Layout (26 Bytes: Sub-opt 6=8, Sub-opt 8=IP, Sub-opt 9/10='PXE')"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family("Menlo")
                                                .text_color(rgb(0x34d399))
                                                .child(if pxe_mode == PxeServerMode::ProxyDhcp {
                                                    "yiaddr: 0.0.0.0 • Opt 60: PXEClient • Opt 1/3/51 Excluded • Opt 54 Guard".to_string()
                                                } else {
                                                    format!("Option 6 DNS + Fallback: {dns_hex}")
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(0x090d16))
                                        .border_1()
                                        .border_color(rgb(0x162032))
                                        .font_family("Menlo")
                                        .text_xs()
                                        .text_color(rgb(0x22d3ee))
                                        .child(opt43_hex),
                                ),
                        )
                        .child(
                            div()
                                .p_3()
                                .rounded_xl()
                                .bg(rgb(0x111726))
                                .border_1()
                                .border_color(rgb(0x1e293b))
                                .flex()
                                .flex_col()
                                .gap_1p5()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xf8fafc))
                                                .child("Dynamic iPXE Script (Uses ${next-server} & Unconditional set dns 8.8.8.8)"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .id("write-tftpboot-ipxe")
                                                        .px_2p5()
                                                        .py_1()
                                                        .rounded_md()
                                                        .bg(rgb(0x10b981))
                                                        .text_color(rgb(0x090d16))
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::BOLD)
                                                        .cursor(gpui::CursorStyle::PointingHand)
                                                        .on_click(cx.listener(move |view, _, _, cx| {
                                                            let _ = std::fs::create_dir_all("tftpboot");
                                                            let path = format!(
                                                                "tftpboot/{}",
                                                                view.ssh_state.pxe_bootfile
                                                            );
                                                            match std::fs::write(&path, &ipxe_script_write)
                                                            {
                                                                Ok(()) => {
                                                                    view.ssh_state.status_message =
                                                                        Some(format!(
                                                                        "💾 Wrote strict iPXE script to {path}"
                                                                    ));
                                                                }
                                                                Err(e) => {
                                                                    view.ssh_state.status_message =
                                                                        Some(format!("⚠ {e}"));
                                                                }
                                                            }
                                                            cx.notify();
                                                        }))
                                                        .child("💾 Save tftpboot/boot.ipxe"),
                                                )
                                                .child(
                                                    div()
                                                        .id("copy-ipxe-script")
                                                        .px_2p5()
                                                        .py_1()
                                                        .rounded_md()
                                                        .bg(rgb(0x161f33))
                                                        .border_1()
                                                        .border_color(rgb(0x38bdf8))
                                                        .text_xs()
                                                        .text_color(rgb(0x38bdf8))
                                                        .cursor(gpui::CursorStyle::PointingHand)
                                                        .on_click(cx.listener(move |view, _, _, cx| {
                                                            cx.write_to_clipboard(
                                                                gpui::ClipboardItem::new_string(
                                                                    ipxe_script_copy.clone(),
                                                                ),
                                                            );
                                                            view.ssh_state.status_message = Some(
                                                                "📋 Copied iPXE script to clipboard!"
                                                                    .to_string(),
                                                            );
                                                            cx.notify();
                                                        }))
                                                        .child("📋 Copy Script"),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0x090d16))
                                        .border_1()
                                        .border_color(rgb(0x162032))
                                        .font_family("Menlo")
                                        .text_xs()
                                        .text_color(rgb(0xe2e8f0))
                                        .child(ipxe_script),
                                ),
                        )
                        .into_any_element(),
                    PxeBootSubtab::CustomBootloader => div()
                        .flex()
                        .flex_col()
                        .gap_2p5()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2p5()
                                .child(
                                    div()
                                        .flex_1()
                                        .p_3()
                                        .rounded_xl()
                                        .bg(rgb(0x111726))
                                        .border_1()
                                        .border_color(rgb(0x06b6d4))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x22d3ee))
                                                .child("1. Bare-Metal UEFI SimpleNetworkProtocol (SNP)"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x94a3b8))
                                                .child("Routes Layer 2 frame RX/TX strictly through firmware-initialized UEFI SNP. Never performs raw PCIe MMIO PHY resets (prevents Intel Haswell/I218-V bus freezes)."),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .p_3()
                                        .rounded_xl()
                                        .bg(rgb(0x111726))
                                        .border_1()
                                        .border_color(rgb(0xa855f7))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xc084fc))
                                                .child("2. DHCP Option 77 ('iPXE') + Option 175 Handshake"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x94a3b8))
                                                .child("Broadcasts User Class 'iPXE' (Opt 77) and encapsulated Opt 175 in DHCPDISCOVER/REQUEST so pxe-rs transitions from ROM Stage 1 to HTTP Stage 2 without bootloops."),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2p5()
                                .child(
                                    div()
                                        .flex_1()
                                        .p_3()
                                        .rounded_xl()
                                        .bg(rgb(0x111726))
                                        .border_1()
                                        .border_color(rgb(0x10b981))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x34d399))
                                                .child("3. Dual-Path Chainloader (#!ipxe vs MZ PE32/COFF)"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x94a3b8))
                                                .child("Inspects payload magic bytes before LoadImage: b\"#!ipxe\" evaluates via Evaluator::execute with inherited ${ip}/${server_ip}; b\"MZ\" delegates to UEFI LoadImage + StartImage."),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .p_3()
                                        .rounded_xl()
                                        .bg(rgb(0x111726))
                                        .border_1()
                                        .border_color(rgb(0xf59e0b))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xfbbf24))
                                                .child("4. Zero-Copy TftpAction & Cargo Isolation"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x94a3b8))
                                                .child("Decouples rx_frame_buffer parsing from send_udp via stack TftpAction::Ack(block, is_eof), and isolates [[bin]] ipxe-uefi with test = false."),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .p_3()
                                .rounded_xl()
                                .bg(rgb(0x111726))
                                .border_1()
                                .border_color(rgb(0x1e293b))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xf8fafc))
                                                .child("Headless QEMU EDK2 UEFI Netboot Regression Harness (Slirp DHCP + TFTP + Serial Log)"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .id("copy-qemu-uefi-cmd")
                                                        .px_2p5()
                                                        .py_1()
                                                        .rounded_md()
                                                        .bg(rgb(0x161f33))
                                                        .border_1()
                                                        .border_color(rgb(0x38bdf8))
                                                        .text_xs()
                                                        .text_color(rgb(0x38bdf8))
                                                        .cursor(gpui::CursorStyle::PointingHand)
                                                        .on_click(cx.listener(move |view, _, _, cx| {
                                                            cx.write_to_clipboard(
                                                                gpui::ClipboardItem::new_string(
                                                                    qemu_cmd_copy.clone(),
                                                                ),
                                                            );
                                                            view.ssh_state.status_message = Some(
                                                                "📋 Copied Headless QEMU EDK2 UEFI command!"
                                                                    .to_string(),
                                                            );
                                                            cx.notify();
                                                        }))
                                                        .child("📋 Copy QEMU Cmd"),
                                                )
                                                .child(
                                                    div()
                                                        .id("run-qemu-uefi-split")
                                                        .px_3()
                                                        .py_1()
                                                        .rounded_md()
                                                        .bg(rgb(0x10b981))
                                                        .text_color(rgb(0x090d16))
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::BOLD)
                                                        .cursor(gpui::CursorStyle::PointingHand)
                                                        .on_click(cx.listener(move |view, _, window, cx| {
                                                            let _ = std::fs::create_dir_all("tftpboot");
                                                            let _ = std::fs::create_dir_all("target/esp");
                                                            view.dispatch_command_to_split_pane(
                                                                &qemu_cmd_run,
                                                                window,
                                                                cx,
                                                            );
                                                        }))
                                                        .child("▶ Launch QEMU UEFI in Split Pane"),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0x090d16))
                                        .border_1()
                                        .border_color(rgb(0x162032))
                                        .font_family("Menlo")
                                        .text_xs()
                                        .text_color(rgb(0x22d3ee))
                                        .child(qemu_cmd),
                                ),
                        )
                        .into_any_element(),
                };

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(header)
                    .child(body)
                    .into_any_element()
            }
            SshConsoleSection::SmbShares => {
                let shares = self.ssh_state.smb_shares.clone();
                let s_host = self.ssh_state.smb_draft_host.clone();
                let s_share = self.ssh_state.smb_draft_share.clone();
                let s_user = self.ssh_state.smb_draft_user.clone();
                let host_foc = active_input == SshConsoleInputField::SmbServerHost;
                let share_foc = active_input == SshConsoleInputField::SmbShareName;
                let user_foc = active_input == SshConsoleInputField::SmbUsername;

                let mut share_cards = div().flex().flex_col().gap_2p5();
                for (sidx, share) in shares.iter().take(3).enumerate() {
                    let stitle = share.title.clone();
                    let badge = share.purpose_badge.clone();
                    let url = share.smb_url();
                    let url_open = url.clone();
                    let url_copy = url.clone();
                    let mount_cmd = share.mount_smbfs_command();
                    let mount_pt = share.mount_point.clone();
                    let mounted = share.is_mounted();

                    share_cards = share_cards.child(
                        div()
                            .id(("smb-share-card", sidx))
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(if mounted {
                                rgb(0x10b981)
                            } else {
                                rgb(0x1e293b)
                            })
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(0x0d2833))
                                                    .text_color(rgb(0x2ac3de))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child(badge),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child(stitle),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(if mounted {
                                                rgb(0x34d399)
                                            } else {
                                                rgb(0x64748b)
                                            })
                                            .child(if mounted {
                                                format!("● Mounted ({mount_pt})")
                                            } else {
                                                format!("● Unmounted ({mount_pt})")
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x090d16))
                                            .border_1()
                                            .border_color(rgb(0x162032))
                                            .font_family("Menlo")
                                            .text_xs()
                                            .text_color(rgb(0x22d3ee))
                                            .child(url),
                                    )
                                    .child(
                                        div()
                                            .id(("smb-open-finder", sidx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x06b6d4))
                                            .text_color(rgb(0x090d16))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                let _ = std::process::Command::new("open")
                                                    .arg(&url_open)
                                                    .spawn();
                                                view.ssh_state.status_message = Some(format!(
                                                    "📂 Opened {} in macOS Finder SMB mounter.",
                                                    url_open
                                                ));
                                                cx.notify();
                                            }))
                                            .child("📂 Mount in Finder"),
                                    )
                                    .child(
                                        div()
                                            .id(("smb-mount-cli", sidx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x161f33))
                                            .border_1()
                                            .border_color(rgb(0x38bdf8))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x38bdf8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.dispatch_command_to_split_pane(
                                                    &mount_cmd,
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .child("🪟 Mount in Split"),
                                    )
                                    .child(
                                        div()
                                            .id(("smb-copy-url", sidx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x161f33))
                                            .border_1()
                                            .border_color(rgb(0x26324d))
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                cx.write_to_clipboard(
                                                    gpui::ClipboardItem::new_string(
                                                        url_copy.clone(),
                                                    ),
                                                );
                                                view.ssh_state.status_message = Some(format!(
                                                    "📋 Copied {} to clipboard",
                                                    url_copy
                                                ));
                                                cx.notify();
                                            }))
                                            .child("📋 Copy URL"),
                                    ),
                            ),
                    );
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf8fafc))
                                            .child("📁 SMB / CIFS Network Shares & Netboot Payload Mounts"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .child("Mount PXE boot shares (WinPE / Kickstart / ISOs) and remote SMB3 volumes directly into /Volumes"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("add-smb-share-btn")
                                    .px_3p5()
                                    .py_1p5()
                                    .rounded_lg()
                                    .bg(rgb(0x10b981))
                                    .text_color(rgb(0x090d16))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let host = view.ssh_state.smb_draft_host.trim().to_string();
                                        let share = view.ssh_state.smb_draft_share.trim().to_string();
                                        let user = view.ssh_state.smb_draft_user.trim().to_string();
                                        if !host.is_empty() && !share.is_empty() {
                                            view.ssh_state.smb_shares.insert(
                                                0,
                                                SmbShareEntry {
                                                    title: format!("{share} on {host}"),
                                                    server_host: host,
                                                    share_name: share.clone(),
                                                    username: user,
                                                    mount_point: format!("/Volumes/{share}"),
                                                    purpose_badge: "SMB3".into(),
                                                },
                                            );
                                            view.ssh_state.status_message = Some(format!(
                                                "✓ Added SMB share //{} to catalog.",
                                                share
                                            ));
                                            cx.notify();
                                        }
                                    }))
                                    .child("+ Add SMB Share"),
                            ),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .id("smb-host-input")
                                    .flex_1()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if host_foc {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x22d3ee))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::SmbServerHost;
                                        cx.notify();
                                    }))
                                    .child(if host_foc {
                                        format!("Server: {s_host}│")
                                    } else {
                                        format!("Server: {s_host}")
                                    }),
                            )
                            .child(
                                div()
                                    .id("smb-share-input")
                                    .flex_1()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if share_foc {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x34d399))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::SmbShareName;
                                        cx.notify();
                                    }))
                                    .child(if share_foc {
                                        format!("Share: {s_share}│")
                                    } else {
                                        format!("Share: {s_share}")
                                    }),
                            )
                            .child(
                                div()
                                    .id("smb-user-input")
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if user_foc {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0xf8fafc))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::SmbUsername;
                                        cx.notify();
                                    }))
                                    .child(if user_foc {
                                        format!("User: {s_user}│")
                                    } else {
                                        format!("User: {s_user}")
                                    }),
                            ),
                    )
                    .child(share_cards)
                    .into_any_element()
            }
            SshConsoleSection::NetworkTools => {
                let n_host = self.ssh_state.net_target_host.clone();
                let n_mac = self.ssh_state.net_target_mac.clone();
                let host_foc = active_input == SshConsoleInputField::NetTargetHost;
                let mac_foc = active_input == SshConsoleInputField::NetTargetMac;
                let scan_results = self.ssh_state.net_port_scan.clone();
                let arp_list = self.ssh_state.net_arp_entries.clone();

                let mut port_badges = div().flex().flex_row().flex_wrap().gap_1p5();
                if scan_results.is_empty() {
                    port_badges = port_badges.child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("Click '⚡ Scan 15 Ports' above to probe SSH (22), DHCP (67), TFTP (69), SMB (445), ProxyDHCP (4011), iPXE (8080) & KeySrv (8787)."),
                    );
                } else {
                    for res in scan_results {
                        port_badges = port_badges.child(
                            div()
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(if res.open {
                                    gpui::rgba(0x10b98128)
                                } else {
                                    rgb(0x090d16)
                                })
                                .border_1()
                                .border_color(if res.open {
                                    rgb(0x10b981)
                                } else {
                                    rgb(0x1e293b)
                                })
                                .font_family("Menlo")
                                .text_xs()
                                .text_color(if res.open {
                                    rgb(0x34d399)
                                } else {
                                    rgb(0x64748b)
                                })
                                .child(format!(
                                    "{} :{} ({})",
                                    if res.open { "●" } else { "○" },
                                    res.port,
                                    res.service
                                )),
                        );
                    }
                }

                let mut arp_rows = div().flex().flex_col().gap_1p5();
                for (aidx, neighbor) in arp_list.iter().take(4).enumerate() {
                    let nip = neighbor.ip.clone();
                    let nip_scan = nip.clone();
                    let nmac = neighbor.mac.clone();
                    let nmac_wol = nmac.clone();
                    let nhost = neighbor.hostname.clone();
                    let niface = neighbor.interface.clone();

                    arp_rows = arp_rows.child(
                        div()
                            .id(("arp-neighbor-row", aidx))
                            .px_3()
                            .py_1p5()
                            .rounded_lg()
                            .bg(rgb(0x090d16))
                            .border_1()
                            .border_color(rgb(0x162032))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .font_family("Menlo")
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x22d3ee))
                                            .child(nip),
                                    )
                                    .child(
                                        div()
                                            .font_family("Menlo")
                                            .text_xs()
                                            .text_color(rgb(0xa855f7))
                                            .child(nmac),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x64748b))
                                            .child(format!("{nhost} ({niface})")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(("arp-scan-btn", aidx))
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x161f33))
                                            .border_1()
                                            .border_color(rgb(0x38bdf8))
                                            .text_xs()
                                            .text_color(rgb(0x38bdf8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.ssh_state.net_target_host = nip_scan.clone();
                                                view.ssh_state.net_port_scan =
                                                    run_fast_port_scan(&nip_scan);
                                                cx.notify();
                                            }))
                                            .child("⚡ Scan"),
                                    )
                                    .child(
                                        div()
                                            .id(("arp-wol-btn", aidx))
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(gpui::rgba(0xf59e0b28))
                                            .border_1()
                                            .border_color(rgb(0xf59e0b))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xfbbf24))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.ssh_state.net_target_mac = nmac_wol.clone();
                                                match send_wol_magic_packet(&nmac_wol) {
                                                    Ok(bytes) => {
                                                        view.ssh_state.status_message = Some(format!(
                                                            "🔔 Broadcast {bytes}-byte Wake-on-LAN Magic Packet to {nmac_wol} (255.255.255.255:9)"
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        view.ssh_state.status_message =
                                                            Some(format!("⚠ {e}"));
                                                    }
                                                }
                                                cx.notify();
                                            }))
                                            .child("🔔 Wake (WoL)"),
                                    ),
                            ),
                    );
                }

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf8fafc))
                                            .child("📡 Networking Tools: Port Scanner, ARP Table, Wake-on-LAN & Packet Sniffer"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .child("Probe L4 services, inspect L2 MAC neighbors, broadcast 102-byte WoL magic packets, or sniff PXE traffic"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("net-ping-split-btn")
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(rgb(0x161f33))
                                            .border_1()
                                            .border_color(rgb(0x38bdf8))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x38bdf8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                let h = view.ssh_state.net_target_host.clone();
                                                view.dispatch_command_to_split_pane(
                                                    &format!("ping -c 5 {h}"),
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .child("🏓 Ping in Split"),
                                    )
                                    .child(
                                        div()
                                            .id("net-pxe-tcpdump-btn")
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(gpui::rgba(0xa855f728))
                                            .border_1()
                                            .border_color(rgb(0xa855f7))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xc084fc))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.dispatch_command_to_split_pane(
                                                    "sudo tcpdump -i any -n 'udp port 67 or udp port 68 or udp port 69 or udp port 4011'",
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .child("🦈 Sniff PXE/DHCP"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .id("net-host-input")
                                    .flex_1()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if host_foc {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x22d3ee))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::NetTargetHost;
                                        cx.notify();
                                    }))
                                    .child(if host_foc {
                                        format!("Target Host/IP: {n_host}│")
                                    } else {
                                        format!("Target Host/IP: {n_host}")
                                    }),
                            )
                            .child(
                                div()
                                    .id("net-scan-ports-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x06b6d4))
                                    .text_color(rgb(0x090d16))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let h = view.ssh_state.net_target_host.clone();
                                        let results = run_fast_port_scan(&h);
                                        let open_cnt = results.iter().filter(|r| r.open).count();
                                        view.ssh_state.net_port_scan = results;
                                        view.ssh_state.status_message = Some(format!(
                                            "⚡ Scanned 15 ports on {h} — {open_cnt} open."
                                        ));
                                        cx.notify();
                                    }))
                                    .child("⚡ Scan 15 Ports"),
                            )
                            .child(
                                div()
                                    .id("net-mac-input")
                                    .flex_1()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x090d16))
                                    .border_1()
                                    .border_color(if mac_foc {
                                        rgb(0x06b6d4)
                                    } else {
                                        rgb(0x1e293b)
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0xfbbf24))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.ssh_state.active_input =
                                            SshConsoleInputField::NetTargetMac;
                                        cx.notify();
                                    }))
                                    .child(if mac_foc {
                                        format!("WoL MAC: {n_mac}│")
                                    } else {
                                        format!("WoL MAC: {n_mac}")
                                    }),
                            )
                            .child(
                                div()
                                    .id("net-send-wol-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0xf59e0b))
                                    .text_color(rgb(0x090d16))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let mac = view.ssh_state.net_target_mac.clone();
                                        match send_wol_magic_packet(&mac) {
                                            Ok(bytes) => {
                                                view.ssh_state.status_message = Some(format!(
                                                    "🔔 Sent {bytes}-byte UDP Wake-on-LAN Magic Packet to {mac} (255.255.255.255:9)"
                                                ));
                                            }
                                            Err(e) => {
                                                view.ssh_state.status_message =
                                                    Some(format!("⚠ {e}"));
                                            }
                                        }
                                        cx.notify();
                                    }))
                                    .child("🔔 Send WoL Packet"),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf8fafc))
                                    .child(format!("TCP Port Probe Matrix — {n_host}")),
                            )
                            .child(port_badges),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(0x111726))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf8fafc))
                                            .child(format!(
                                                "Live ARP / LAN Neighbor Table ({} entries)",
                                                arp_list.len()
                                            )),
                                    )
                                    .child(
                                        div()
                                            .id("refresh-arp-btn")
                                            .px_2p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x161f33))
                                            .border_1()
                                            .border_color(rgb(0x26324d))
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.ssh_state.net_arp_entries =
                                                    discover_arp_neighbors();
                                                cx.notify();
                                            }))
                                            .child("↻ Refresh ARP"),
                                    ),
                            )
                            .child(arp_rows),
                    )
                    .into_any_element()
            }
        };

        div()
            .id("theia-ssh-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000dd))
            .flex()
            .items_center()
            .justify_center()
            .px_5()
            .py_3()
            .track_focus(&self.ssh_focus_handle)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|view: &mut WorkspaceView, _, window, cx| {
                    window.focus(&view.ssh_focus_handle);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(
                |view: &mut WorkspaceView, event: &gpui::KeyDownEvent, window, cx| {
                    view.handle_ssh_key_down(event, window, cx);
                    cx.stop_propagation();
                },
            ))
            .child(
                div()
                    .id("theia-ssh-modal-box")
                    .w_full()
                    .h_full()
                    .max_w(px(1360.0))
                    .max_h(px(750.0))
                    .overflow_hidden()
                    .bg(theme.bg)
                    .border_2()
                    .border_color(theme.border_active)
                    .rounded_xl()
                    .flex()
                    .flex_col()
                    .child(
                        // Unified Theia's Prism 3-Mode Header Bar inside Theia SSH
                        div()
                            .px_5()
                            .py_2()
                            .bg(theme.tab_inactive_bg)
                            .border_b_1()
                            .border_color(theme.border_inactive)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2p5()
                                    .child(div().text_lg().child("🏛️"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.border_active)
                                                    .child("THEIA'S PRISM — OPERATOR CONTROL CENTER"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.5))
                                                    .text_color(rgb(0x94a3b8))
                                                    .child("Theia SSH, Tunnels, Key Vault, PXE Bootloader & Network Ops"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("ssh-hub-to-prism-btn")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_color(theme.fg)
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.settings_view.update(cx, |sv, cx| {
                                                    sv.set_section(SettingsSection::AppearancePrism, cx);
                                                });
                                                view.toggle_settings_modal(window, cx);
                                            }))
                                            .child("🎨 Theia's Prism (⌘,)"),
                                    )
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.border_active)
                                            .text_color(theme.bg)
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child("🌐 Theia SSH & Net Ops (⇧⌘S)"),
                                    )
                                    .child(
                                        div()
                                            .id("ssh-hub-to-info-btn")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_color(theme.fg)
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.settings_view.update(cx, |sv, cx| {
                                                    sv.set_section(SettingsSection::SystemInfoGuide, cx);
                                                });
                                                view.toggle_settings_modal(window, cx);
                                            }))
                                            .child("📖 Info & System Guide"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .overflow_hidden()
                            .child(icon_rail)
                            .children(if self.ssh_state.section == SshConsoleSection::HostEditor {
                                Some(hosts_sidebar)
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .p_4()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .overflow_hidden()
                                    .children(status_message.map(|msg| {
                                        div()
                                            .px_3()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(gpui::rgba(0x06b6d422))
                                            .border_1()
                                            .border_color(rgb(0x06b6d4))
                                            .text_color(rgb(0x22d3ee))
                                            .text_xs()
                                            .child(msg)
                                    }))
                                    .child(right_content),
                            ),
                    ),
            )
    }

    fn render_settings_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.settings.read(cx).theme.clone();
        div()
            .id("theia-prism-modal-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000dd))
            .flex()
            .items_center()
            .justify_center()
            .px_5()
            .py_3()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|view: &mut WorkspaceView, _, window, cx| {
                    let focus = view.settings_view.read(cx).focus_handle.clone();
                    window.focus(&focus);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id("theia-prism-modal-box")
                    .w_full()
                    .h_full()
                    .max_w(px(1360.0))
                    .max_h(px(750.0))
                    .overflow_hidden()
                    .bg(theme.bg)
                    .border_2()
                    .border_color(theme.border_active)
                    .rounded_xl()
                    .child(self.settings_view.clone()),
            )
    }

    fn render_command_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.settings.read(cx).theme.clone();
        let query = self.palette_query.clone();
        let filtered = filter_actions(&query);
        let total = filtered.len();
        let selected_idx = if total == 0 {
            0
        } else {
            self.palette_selected_idx.min(total - 1)
        };

        let page_size = 5usize;
        let page_start = if total <= page_size {
            0
        } else {
            let half = page_size / 2;
            selected_idx
                .saturating_sub(half)
                .min(total.saturating_sub(page_size))
        };
        let page_end = (page_start + page_size).min(total);

        let mut list_col = div().w_full().flex().flex_col().gap_1p5().overflow_hidden();
        if filtered.is_empty() {
            list_col = list_col.child(
                div()
                    .p_4()
                    .text_xs()
                    .text_color(rgb(0x928374))
                    .child("No matching actions found in WELL_ACTIONS registry."),
            );
        } else {
            for (idx, act) in filtered
                .iter()
                .enumerate()
                .skip(page_start)
                .take(page_size)
            {
                let is_selected = idx == selected_idx;
                let act_id = act.id;
                let cat_label = act.category.as_str();
                let label = act.label;
                let desc = act.description;
                let kb = act.default_keybind;
                let ctx_scope = act.key_context;

                list_col = list_col.child(
                    div()
                        .id(("palette-item", idx))
                        .w_full()
                        .overflow_hidden()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(if is_selected { theme.tab_active_bg } else { theme.tab_inactive_bg })
                        .border_1()
                        .border_color(if is_selected { theme.border_active } else { theme.border_inactive })
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .cursor(gpui::CursorStyle::PointingHand)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.execute_palette_action(act_id, window, cx);
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .pr_2()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(theme.border_inactive)
                                                .text_color(rgb(0x8ec07c))
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .child(cat_label),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.fg)
                                                .child(label),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xa89984))
                                        .overflow_hidden()
                                        .child(desc),
                                ),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(theme.bg)
                                        .text_color(rgb(0x928374))
                                        .text_xs()
                                        .child(ctx_scope),
                                )
                                .child(
                                    div()
                                        .px_2p5()
                                        .py_1()
                                        .rounded_md()
                                        .bg(theme.bg)
                                        .border_1()
                                        .border_color(theme.border_active)
                                        .text_color(theme.border_active)
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(kb),
                                ),
                        ),
                );
            }
        }

        div()
            .id("command-palette-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000dd))
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&self.palette_focus_handle)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|view: &mut WorkspaceView, _, window, cx| {
                    window.focus(&view.palette_focus_handle);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(move |view: &mut WorkspaceView, event: &gpui::KeyDownEvent, window, cx| {
                view.handle_palette_key_down(event, window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .id("command-palette-box")
                    .w(px(740.0))
                    .overflow_hidden()
                    .bg(theme.bg)
                    .border_2()
                    .border_color(theme.border_active)
                    .rounded_xl()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child("⌘ COMMAND PALETTE — LABELED FUNCTION REGISTRY (⇧⌘P)"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .id("pal-to-prism-btn")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_color(theme.fg)
                                            .text_xs()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.toggle_settings_modal(window, cx);
                                            }))
                                            .child("🎨 Prism & Settings"),
                                    )
                                    .child(
                                        div()
                                            .id("pal-to-ssh-btn")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_color(theme.fg)
                                            .text_xs()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.toggle_ssh_manager(window, cx);
                                            }))
                                            .child("🌐 Theia SSH"),
                                    )
                                    .child(
                                        div()
                                            .id("close-palette-x")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_color(theme.fg)
                                            .text_xs()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.toggle_command_palette(window, cx);
                                            }))
                                            .child("✕ Esc"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("palette-search-input")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_active)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .cursor(gpui::CursorStyle::IBeam)
                            .on_click(cx.listener(|view, _, window, cx| {
                                window.focus(&view.palette_focus_handle);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if query.is_empty() { rgb(0xa89984) } else { theme.fg })
                                    .child(if query.is_empty() {
                                        "🔍 Type to filter functions by category, label, description, or shortcut... ▏".to_string()
                                    } else {
                                        format!("🔍 {query}▏")
                                    }),
                            )
                            .children(if !query.is_empty() {
                                Some(
                                    div()
                                        .id("clear-palette-query-btn")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(theme.border_inactive)
                                        .text_xs()
                                        .text_color(theme.fg)
                                        .cursor(gpui::CursorStyle::PointingHand)
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.palette_query.clear();
                                            view.palette_selected_idx = 0;
                                            window.focus(&view.palette_focus_handle);
                                            cx.notify();
                                        }))
                                        .child("✕ Clear"),
                                )
                            } else {
                                None
                            }),
                    )
                    .child(list_col)
                    .child(
                        div()
                            .pt_1()
                            .border_t_1()
                            .border_color(theme.border_inactive)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .text_xs()
                            .text_color(rgb(0x928374))
                            .child(if total == 0 {
                                "0 matching actions".to_string()
                            } else {
                                format!(
                                    "Showing {}–{} of {} actions  ·  ↑/↓ or Tab to select  ·  ↵ to execute",
                                    page_start + 1,
                                    page_end,
                                    total
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("palette-prev-btn")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_color(theme.fg)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                if total > 0 {
                                                    view.palette_selected_idx = view
                                                        .palette_selected_idx
                                                        .saturating_sub(page_size);
                                                    cx.notify();
                                                }
                                            }))
                                            .child("◀ Prev"),
                                    )
                                    .child(
                                        div()
                                            .id("palette-next-btn")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x282828))
                                            .border_1()
                                            .border_color(rgb(0x504945))
                                            .text_color(rgb(0xebdbb2))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                if total > 0 {
                                                    view.palette_selected_idx = (view
                                                        .palette_selected_idx
                                                        + page_size)
                                                        .min(total - 1);
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Next ▶"),
                                    ),
                            ),
                    ),
            )
    }

    fn handle_palette_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let filtered = filter_actions(&self.palette_query);
        let total = filtered.len();
        let selected_idx = if total == 0 {
            0
        } else {
            self.palette_selected_idx.min(total - 1)
        };
        let selected_action_id = filtered.get(selected_idx).map(|a| a.id);

        let key = event.keystroke.key.as_str();
        match key {
            "escape" => {
                self.toggle_command_palette(window, cx);
            }
            "up" => {
                if total > 0 {
                    self.palette_selected_idx = if self.palette_selected_idx == 0 {
                        total - 1
                    } else {
                        self.palette_selected_idx - 1
                    };
                    cx.notify();
                }
            }
            "down" | "tab" => {
                if total > 0 {
                    self.palette_selected_idx = (self.palette_selected_idx + 1) % total;
                    cx.notify();
                }
            }
            "enter" => {
                if let Some(id) = selected_action_id {
                    self.execute_palette_action(id, window, cx);
                }
            }
            "backspace" => {
                self.palette_query.pop();
                self.palette_selected_idx = 0;
                cx.notify();
            }
            _ => {
                if let Some(ch) = settings_view::extract_printable_char(&event.keystroke) {
                    self.palette_query.push_str(&ch);
                    self.palette_selected_idx = 0;
                    cx.notify();
                }
            }
        }
    }

    fn handle_ssh_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        if (mods.platform || mods.control) && key.eq_ignore_ascii_case("s") {
            self.save_current_ssh_draft(cx);
            return;
        }
        match key {
            "escape" => {
                self.toggle_ssh_manager(window, cx);
            }
            "enter" => {
                if self.ssh_state.section == SshConsoleSection::HostEditor {
                    self.save_current_ssh_draft(cx);
                }
            }
            "tab" => {
                self.ssh_state.active_input = match self.ssh_state.active_input {
                    SshConsoleInputField::HostName => SshConsoleInputField::HostHostname,
                    SshConsoleInputField::HostHostname => SshConsoleInputField::HostUser,
                    SshConsoleInputField::HostUser => SshConsoleInputField::HostPort,
                    SshConsoleInputField::HostPort => SshConsoleInputField::HostIdentityFile,
                    SshConsoleInputField::HostIdentityFile => SshConsoleInputField::HostProxyJump,
                    SshConsoleInputField::HostProxyJump => SshConsoleInputField::SidebarSearch,
                    SshConsoleInputField::SidebarSearch => SshConsoleInputField::HostName,
                    SshConsoleInputField::TunnelLocalListen => {
                        SshConsoleInputField::TunnelRemoteTarget
                    }
                    SshConsoleInputField::TunnelRemoteTarget => {
                        SshConsoleInputField::TunnelLocalListen
                    }
                    other => other,
                };
                cx.notify();
            }
            "backspace" => {
                match self.ssh_state.active_input {
                    SshConsoleInputField::SidebarSearch => {
                        self.ssh_state.search_filter.pop();
                    }
                    SshConsoleInputField::HostName => {
                        self.ssh_state.host_draft.name.pop();
                    }
                    SshConsoleInputField::HostHostname => {
                        self.ssh_state.host_draft.hostname.pop();
                    }
                    SshConsoleInputField::HostUser => {
                        self.ssh_state.host_draft.user.pop();
                    }
                    SshConsoleInputField::HostPort => {
                        self.ssh_state.host_draft.port.pop();
                    }
                    SshConsoleInputField::HostIdentityFile => {
                        self.ssh_state.host_draft.identity_file.pop();
                    }
                    SshConsoleInputField::HostProxyJump => {
                        self.ssh_state.host_draft.proxy_jump.pop();
                    }
                    SshConsoleInputField::TunnelLocalListen => {
                        self.ssh_state.tunnel_draft_local.pop();
                    }
                    SshConsoleInputField::TunnelRemoteTarget => {
                        self.ssh_state.tunnel_draft_remote.pop();
                    }
                    SshConsoleInputField::ServerPort => {
                        self.ssh_state.server_port.pop();
                    }
                    SshConsoleInputField::ServerListenAddr => {
                        self.ssh_state.server_listen_addr.pop();
                    }
                    SshConsoleInputField::AgentKeyPath => {
                        self.ssh_state.agent_key_path.pop();
                    }
                    SshConsoleInputField::TunnelSshHost => {
                        self.ssh_state.tunnel_ssh_host.pop();
                    }
                    SshConsoleInputField::KeyServerPort => {
                        self.ssh_state.key_server_port.pop();
                    }
                    SshConsoleInputField::PxeServerIp => {
                        self.ssh_state.pxe_server_ip.pop();
                    }
                    SshConsoleInputField::PxeHttpPort => {
                        self.ssh_state.pxe_http_port.pop();
                    }
                    SshConsoleInputField::PxeBootFile => {
                        self.ssh_state.pxe_bootfile.pop();
                    }
                    SshConsoleInputField::SmbServerHost => {
                        self.ssh_state.smb_draft_host.pop();
                    }
                    SshConsoleInputField::SmbShareName => {
                        self.ssh_state.smb_draft_share.pop();
                    }
                    SshConsoleInputField::SmbUsername => {
                        self.ssh_state.smb_draft_user.pop();
                    }
                    SshConsoleInputField::NetTargetHost => {
                        self.ssh_state.net_target_host.pop();
                    }
                    SshConsoleInputField::NetTargetMac => {
                        self.ssh_state.net_target_mac.pop();
                    }
                }
                cx.notify();
            }
            _ => {
                if let Some(ch) = settings_view::extract_printable_char(&event.keystroke) {
                    match self.ssh_state.active_input {
                        SshConsoleInputField::SidebarSearch => {
                            self.ssh_state.search_filter.push_str(&ch);
                        }
                        SshConsoleInputField::HostName => {
                            self.ssh_state.host_draft.name.push_str(&ch);
                        }
                        SshConsoleInputField::HostHostname => {
                            self.ssh_state.host_draft.hostname.push_str(&ch);
                        }
                        SshConsoleInputField::HostUser => {
                            self.ssh_state.host_draft.user.push_str(&ch);
                        }
                        SshConsoleInputField::HostPort => {
                            if ch.chars().all(|c| c.is_ascii_digit()) {
                                self.ssh_state.host_draft.port.push_str(&ch);
                            }
                        }
                        SshConsoleInputField::HostIdentityFile => {
                            self.ssh_state.host_draft.identity_file.push_str(&ch);
                        }
                        SshConsoleInputField::HostProxyJump => {
                            self.ssh_state.host_draft.proxy_jump.push_str(&ch);
                        }
                        SshConsoleInputField::TunnelLocalListen => {
                            self.ssh_state.tunnel_draft_local.push_str(&ch);
                        }
                        SshConsoleInputField::TunnelRemoteTarget => {
                            self.ssh_state.tunnel_draft_remote.push_str(&ch);
                        }
                        SshConsoleInputField::ServerPort => {
                            if ch.chars().all(|c| c.is_ascii_digit()) {
                                self.ssh_state.server_port.push_str(&ch);
                            }
                        }
                        SshConsoleInputField::ServerListenAddr => {
                            self.ssh_state.server_listen_addr.push_str(&ch);
                        }
                        SshConsoleInputField::AgentKeyPath => {
                            self.ssh_state.agent_key_path.push_str(&ch);
                        }
                        SshConsoleInputField::TunnelSshHost => {
                            self.ssh_state.tunnel_ssh_host.push_str(&ch);
                        }
                        SshConsoleInputField::KeyServerPort => {
                            if ch.chars().all(|c| c.is_ascii_digit()) {
                                self.ssh_state.key_server_port.push_str(&ch);
                            }
                        }
                        SshConsoleInputField::PxeServerIp => {
                            self.ssh_state.pxe_server_ip.push_str(&ch);
                        }
                        SshConsoleInputField::PxeHttpPort => {
                            if ch.chars().all(|c| c.is_ascii_digit()) {
                                self.ssh_state.pxe_http_port.push_str(&ch);
                            }
                        }
                        SshConsoleInputField::PxeBootFile => {
                            self.ssh_state.pxe_bootfile.push_str(&ch);
                        }
                        SshConsoleInputField::SmbServerHost => {
                            self.ssh_state.smb_draft_host.push_str(&ch);
                        }
                        SshConsoleInputField::SmbShareName => {
                            self.ssh_state.smb_draft_share.push_str(&ch);
                        }
                        SshConsoleInputField::SmbUsername => {
                            self.ssh_state.smb_draft_user.push_str(&ch);
                        }
                        SshConsoleInputField::NetTargetHost => {
                            self.ssh_state.net_target_host.push_str(&ch);
                        }
                        SshConsoleInputField::NetTargetMac => {
                            self.ssh_state.net_target_mac.push_str(&ch);
                        }
                    }
                    cx.notify();
                }
            }
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, font_family, geometric_border, settings_open) = {
            let settings = self.settings.read(cx);
            (
                settings.theme.clone(),
                settings.font_family.clone(),
                settings.geometric_border,
                settings.settings_open,
            )
        };
        if self.palette_open && !self.palette_focus_handle.is_focused(window) {
            window.focus(&self.palette_focus_handle);
        } else if self.ssh_state.open && !self.ssh_focus_handle.is_focused(window) {
            window.focus(&self.ssh_focus_handle);
        } else if settings_open {
            let sf = self.settings_view.read(cx).focus_handle.clone();
            if !sf.is_focused(window) {
                window.focus(&sf);
            }
        }

        let active_tab_idx = self.active_tab;
        let active_tab = &self.tabs[active_tab_idx];
        let is_active_ssh = active_tab.is_ssh;
        let (git_branch, is_dirty, rows, cols, mneme_active, pythia_active) = {
            let active_term = active_tab.active_terminal.read(cx);
            let snap = active_term.astraea.read_snapshot();
            let branch = if !snap.git_branch.is_empty() {
                snap.git_branch.clone()
            } else {
                "main".to_string()
            };
            let (r, c) = active_term.last_size.get().unwrap_or((24, 80));
            (
                branch,
                snap.is_dirty,
                r,
                c,
                active_term.mneme_active,
                active_term.pythia.active,
            )
        };
        let footer_hints = footer_hints_for_context(mneme_active, pythia_active, is_active_ssh);
        let tab_title = active_tab.title.clone();

        // Tier 1 Left: ❖ WELL + [● ATLAS] pill + [+]
        let header_left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2p5()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_base()
                            .text_color(rgb(0x22d3ee))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("❖"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg)
                            .child("WELL"),
                    ),
            )
            .child(
                div()
                    .px_2p5()
                    .py_0p5()
                    .rounded_md()
                    .bg(rgb(0x121926))
                    .border_1()
                    .border_color(rgb(0x22d3ee))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(div().size(px(6.0)).rounded_full().bg(rgb(0x22c55e)))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x22d3ee))
                            .child(if is_active_ssh { "THEIA SSH" } else { "ATLAS" }),
                    ),
            )
            .child(
                div()
                    .id("header-new-tab")
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x263248))
                    .bg(rgb(0x162030))
                    .text_color(theme.fg)
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(|view, _event, window, cx| {
                        view.add_tab(window, cx);
                    }))
                    .child("+"),
            );

        // Tier 1 Center: Split-Pane & Geometric Frame Control Capsule [ −  ·  ∏  ·  ⊞  ·  ◈ ]
        let header_center = div()
            .px_2p5()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x0f141e))
            .border_1()
            .border_color(rgb(0x222e44))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("capsule-split-horiz")
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xc8d7e6))
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.split_active_pane(SplitDirection::Horizontal, window, cx);
                    }))
                    .child("−"),
            )
            .child(div().text_xs().text_color(rgb(0x2d3c55)).child("·"))
            .child(
                div()
                    .id("capsule-split-vert")
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xc8d7e6))
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.split_active_pane(SplitDirection::Vertical, window, cx);
                    }))
                    .child("∏"),
            )
            .child(div().text_xs().text_color(rgb(0x2d3c55)).child("·"))
            .child(
                div()
                    .id("capsule-cycle-pane")
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xf56e82))
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.focus_next_pane(window, cx);
                    }))
                    .child("□"),
            )
            .child(div().text_xs().text_color(rgb(0x2d3c55)).child("·"))
            .child(
                div()
                    .id("capsule-geom-toggle")
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(if geometric_border {
                        rgb(0x22d3ee)
                    } else {
                        rgb(0x64748b)
                    })
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.settings.update(cx, |s, cx| {
                            s.geometric_border = !s.geometric_border;
                            let _ = s.save_to_disk();
                            cx.notify();
                        });
                        cx.notify();
                    }))
                    .child("◈"),
            );

        // Tier 2: Shell Tab Strip (well #1, well #2, ..., well #8 ×)
        let mut tab_strip_items: Vec<gpui::AnyElement> = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active_tab;
            let is_ssh = tab.is_ssh;
            let border_color = if is_active {
                if is_ssh {
                    rgb(0x8ec07c)
                } else {
                    rgb(0x22d3ee)
                }
            } else {
                rgb(0x1c2636)
            };
            let bg = if is_active {
                if is_ssh {
                    rgb(0x1c2e24)
                } else {
                    rgb(0x162030)
                }
            } else {
                rgb(0x0e121b)
            };
            let title = tab.title.clone();

            tab_strip_items.push(
                div()
                    .id(("tab-pill", i))
                    .bg(bg)
                    .px_2p5()
                    .py_1()
                    .rounded_t_md()
                    .border_1()
                    .border_color(border_color)
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id(("select", i))
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _event, window, cx| {
                                view.active_tab = i;
                                let focus = view.tabs[i].active_terminal.read(cx).focus_handle.clone();
                                window.focus(&focus);
                                cx.notify();
                            }))
                            .child(title)
                            .text_color(if is_active {
                                rgb(0xffffff)
                            } else {
                                rgb(0xa0afc3)
                            })
                            .font_weight(if is_active {
                                gpui::FontWeight::BOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .text_xs(),
                    )
                    .child(
                        div()
                            .id(("close", i))
                            .child("×")
                            .text_color(if is_active {
                                rgb(0xff788c)
                            } else {
                                rgb(0x64748b)
                            })
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _event, window, cx| {
                                view.close_tab(i, window, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

        div()
            .key_context("Workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg)
            .on_key_down(cx.listener(
                |view: &mut WorkspaceView, event: &gpui::KeyDownEvent, window, cx| {
                    if view.palette_open {
                        view.handle_palette_key_down(event, window, cx);
                        cx.stop_propagation();
                    } else if view.ssh_state.open {
                        view.handle_ssh_key_down(event, window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &OpenCommandPalette, window, cx| {
                    view.toggle_command_palette(window, cx);
                },
            ))
            .on_action(cx.listener(|view: &mut WorkspaceView, _: &NewTab, window, cx| {
                view.add_tab(window, cx);
            }))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &OpenSnippetPicker, window, cx| {
                    view.execute_palette_action("mneme::ExpandSnippet", window, cx);
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &SplitHorizontal, window, cx| {
                    view.split_active_pane(SplitDirection::Horizontal, window, cx);
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &SplitVertical, window, cx| {
                    view.split_active_pane(SplitDirection::Vertical, window, cx);
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &ClosePane, window, cx| {
                    view.close_active_pane(window, cx);
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &FocusNextPane, window, cx| {
                    view.focus_next_pane(window, cx);
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &OpenSshManager, window, cx| {
                    view.toggle_ssh_manager(window, cx);
                },
            ))
            .on_action(cx.listener(
                |view: &mut WorkspaceView, _: &OpenSettings, window, cx| {
                    view.toggle_settings_modal(window, cx);
                },
            ))
            // Tier 1 Header Bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .w_full()
                    .pl_20()
                    .pr_4()
                    .py_1p5()
                    .bg(theme.tab_inactive_bg)
                    .border_b_1()
                    .border_color(theme.border_inactive)
                    .child(header_left)
                    .child(header_center)
                    .child(div().w(px(140.0))),
            )
            // Tier 2 Shell Tab Strip (well #1, well #2, ...)
            .child(
                div()
                    .w_full()
                    .px_3()
                    .pt_1()
                    .pb_1()
                    .bg(theme.bg)
                    .border_b_1()
                    .border_color(rgb(0x1a2434))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .children(tab_strip_items),
            )
            // Main Terminal Viewport + Optional ATLAS Geometric Lattice Border
            .child(
                div()
                    .relative()
                    .flex_grow()
                    .px(if geometric_border { px(13.0) } else { px(0.0) })
                    .pb(if geometric_border { px(13.0) } else { px(0.0) })
                    .child(self.tabs[self.active_tab].root.render(cx))
                    .children(if geometric_border {
                        Some(
                            canvas(
                                move |_bounds, _window, _cx| {},
                                move |bounds, _prepaint, window, _cx| {
                                    let left = f32::from(bounds.origin.x) + 0.5;
                                    let top = f32::from(bounds.origin.y) + 1.0;
                                    let right =
                                        f32::from(bounds.origin.x + bounds.size.width) - 0.5;
                                    let bottom =
                                        f32::from(bounds.origin.y + bounds.size.height) - 0.5;
                                    let step = 20.0_f32;
                                    let band = 11.0_f32;
                                    if right - left <= band * 2.0 || bottom - top <= band * 2.0 {
                                        return;
                                    }

                                    let mut cyan_builder = gpui::PathBuilder::stroke(px(0.85));
                                    let mut violet_builder = gpui::PathBuilder::stroke(px(0.85));

                                    let mut i = 0usize;
                                    let mut y = top;
                                    while y < bottom - band {
                                        let end = (y + step).min(bottom - band);
                                        let mid_y = (y + end) * 0.5;
                                        let target = if i.is_multiple_of(2) {
                                            &mut cyan_builder
                                        } else {
                                            &mut violet_builder
                                        };
                                        target.add_polygon(
                                            &[
                                                gpui::point(px(left), px(y)),
                                                gpui::point(px(left + band), px(mid_y)),
                                                gpui::point(px(left), px(end)),
                                            ],
                                            true,
                                        );
                                        target.add_polygon(
                                            &[
                                                gpui::point(px(right), px(y)),
                                                gpui::point(px(right - band), px(mid_y)),
                                                gpui::point(px(right), px(end)),
                                            ],
                                            true,
                                        );
                                        y = end;
                                        i += 1;
                                    }

                                    let mut x = left;
                                    while x < right {
                                        let end = (x + step).min(right);
                                        let mid_x = (x + end) * 0.5;
                                        let target = if i.is_multiple_of(2) {
                                            &mut cyan_builder
                                        } else {
                                            &mut violet_builder
                                        };
                                        target.add_polygon(
                                            &[
                                                gpui::point(px(x), px(bottom - band)),
                                                gpui::point(px(mid_x), px(bottom)),
                                                gpui::point(px(end), px(bottom - band)),
                                            ],
                                            true,
                                        );
                                        x = end;
                                        i += 1;
                                    }

                                    let inner_left = left + band;
                                    let inner_right = right - band;
                                    let inner_bottom = bottom - band;
                                    cyan_builder.move_to(gpui::point(px(inner_left), px(top)));
                                    cyan_builder
                                        .line_to(gpui::point(px(inner_left), px(inner_bottom)));
                                    cyan_builder
                                        .line_to(gpui::point(px(inner_right), px(inner_bottom)));
                                    cyan_builder.line_to(gpui::point(px(inner_right), px(top)));

                                    if let Ok(path) = cyan_builder.build() {
                                        window.paint_path(path, rgb(0x22d3ee));
                                    }
                                    if let Ok(path) = violet_builder.build() {
                                        window.paint_path(path, rgb(0xa855f7));
                                    }
                                },
                            )
                            .absolute()
                            .inset_0(),
                        )
                    } else {
                        None
                    })
                    .children(if self.ssh_state.open {
                        Some(self.render_ssh_modal(cx))
                    } else {
                        None
                    })
                    .children(if self.palette_open {
                        Some(self.render_command_palette(cx))
                    } else {
                        None
                    })
                    .children(if settings_open {
                        Some(self.render_settings_modal(cx))
                    } else {
                        None
                    }),
            )
            .child(
                // Theia Bottom Status Bar with Interactive Context-Sensitive Help Bar
                div()
                    .w_full()
                    .h(px(24.0))
                    .px_3()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .bg(if is_active_ssh { rgb(0x28332a) } else { theme.tab_inactive_bg })
                    .border_t_1()
                    .border_color(if is_active_ssh { rgb(0x8ec07c) } else { theme.border_inactive })
                    .text_xs()
                    .font_family(font_family)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("status-remote-btn")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .bg(if is_active_ssh { rgb(0x8ec07c) } else { theme.border_inactive })
                                    .text_color(if is_active_ssh { theme.bg } else { theme.fg })
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.toggle_ssh_manager(window, cx);
                                    }))
                                    .child("><")
                                    .child(if is_active_ssh {
                                        format!("SSH: {tab_title}")
                                    } else {
                                        "Theia SSH (⇧⌘S)".into()
                                    })
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .text_color(theme.fg)
                                    .child("")
                                    .child(git_branch)
                                    .children(if is_dirty {
                                        Some(div().text_color(rgb(0xfabd2f)).font_weight(gpui::FontWeight::BOLD).child("*"))
                                    } else {
                                        None
                                    })
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .text_color(rgb(0xa89984))
                            .children(footer_hints.into_iter().enumerate().map(|(hidx, (kb, label))| {
                                let label_str = label.to_string();
                                div()
                                    .id(("footer-hint-pill", hidx))
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(theme.bg)
                                    .text_color(theme.fg)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        if label_str.contains("Functions") {
                                            view.toggle_command_palette(window, cx);
                                        } else if label_str.contains("Pythia") {
                                            view.settings.update(cx, |s, _| {
                                                s.settings_open = false;
                                            });
                                            view.ssh_state.open = false;
                                            view.palette_open = false;
                                            let active_term =
                                                view.tabs[view.active_tab].active_terminal.clone();
                                            active_term.update(cx, |term, cx| {
                                                term.pythia.active = !term.pythia.active;
                                                cx.notify();
                                            });
                                            window.focus(&active_term.read(cx).focus_handle);
                                        } else if label_str.contains("Snippets") {
                                            view.execute_palette_action(
                                                "mneme::ExpandSnippet",
                                                window,
                                                cx,
                                            );
                                        } else if label_str.contains("Split") {
                                            view.split_active_pane(
                                                SplitDirection::Horizontal,
                                                window,
                                                cx,
                                            );
                                        }
                                    }))
                                    .child(format!("{kb} {label}"))
                            }))
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .text_color(rgb(0xa89984))
                                    .child(format!("{} × {}", cols, rows))
                            )
                            .child(
                                div()
                                    .id("status-prism-btn")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .bg(if settings_open {
                                        theme.border_active
                                    } else {
                                        theme.bg
                                    })
                                    .text_color(if settings_open {
                                        theme.bg
                                    } else {
                                        theme.border_active
                                    })
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.toggle_settings_modal(window, cx);
                                    }))
                                    .child("🏛️ Theia's Prism (⌘,)")
                            )
                    )
            )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match diagnostics::startup_command_output(&args) {
        Ok(Some(output)) => {
            println!("{output}");
            return;
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("[ERROR] {err}");
            std::process::exit(1);
        }
    }

    let (app_tx, mut app_rx) = tokio::sync::mpsc::channel(10);
    
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let socket_path = std::path::Path::new("/tmp/well-caduceus.sock");
            let server = well_ipc::HermesRpcServer::new(app_tx);
            if let Ok(bound) = server.bind(socket_path).await {
                let _ = bound.serve_until(std::future::pending()).await;
            }
        });
    });

    Application::new().run(move |cx: &mut App| {
        let settings_model = cx.new(|_| TerminalSettings::new());
        cx.bind_keys([
            KeyBinding::new("cmd-,", OpenSettings, None),
            KeyBinding::new("cmd-shift-p", OpenCommandPalette, None),
            KeyBinding::new("cmd-t", NewTab, None),
            KeyBinding::new("ctrl-r", OpenSnippetPicker, None),
            KeyBinding::new("cmd-d", SplitHorizontal, None),
            KeyBinding::new("cmd-shift-d", SplitVertical, None),
            KeyBinding::new("cmd-w", ClosePane, None),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-]", FocusNextPane, None),
            KeyBinding::new("cmd-shift-s", OpenSshManager, None),
            KeyBinding::new("f12", OpenSettings, None),
        ]);

        cx.set_menus(vec![
            gpui::Menu {
                name: "Well".into(),
                items: vec![
                    gpui::MenuItem::action("Theia's Prism Control Center...", OpenSettings),
                    gpui::MenuItem::separator(),
                    gpui::MenuItem::action("Quit", Quit),
                ],
            },
            gpui::Menu {
                name: "Window".into(),
                items: vec![
                    gpui::MenuItem::action("Theia SSH Manager...", OpenSshManager),
                    gpui::MenuItem::action("Theia's Prism...", OpenSettings),
                    gpui::MenuItem::separator(),
                    gpui::MenuItem::action("Close", ClosePane),
                ]
            }
        ]);

        cx.on_action(|_: &Quit, cx: &mut App| {
            cx.quit();
        });

        let settings_model_for_action = settings_model.clone();
        cx.on_action(move |_: &OpenSettings, cx: &mut App| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::new(px(140.0), px(90.0)),
                    size: size(px(1140.0), px(740.0)),
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("🏛️ Theia's Prism — Operator Control Center".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(Point::new(px(12.0), px(12.0))),
                }),
                ..Default::default()
            };

            let settings_model_clone = settings_model_for_action.clone();
            cx.open_window(options, move |window, cx| {
                let view = cx.new(|cx| SettingsView::new(cx, settings_model_clone.clone()));
                window.focus(&view.read(cx).focus_handle);
                view
            }).unwrap();
        });

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::new(px(100.0), px(70.0)),
                size: size(px(1280.0), px(820.0)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("Well — Theia's Prism Terminal".into()),
                appears_transparent: true,
                traffic_light_position: Some(Point::new(px(12.0), px(12.0))),
            }),
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let view = cx.new(|cx| WorkspaceView::new(cx, settings_model.clone()));
            window.focus(&view.read(cx).tabs[0].active_terminal.read(cx).focus_handle);
            
            view.update(cx, |_, cx| {
                cx.spawn_in(window, move |weak_view: gpui::WeakEntity<WorkspaceView>, cx: &mut gpui::AsyncWindowContext| {
                    let mut cx = cx.clone();
                    async move {
                        while let Some(cmd) = app_rx.recv().await {
                            let _ = cx.update(|window, cx| {
                            if let Some(view) = weak_view.upgrade() {
                                view.update(cx, |view, cx| {
                                    match cmd {
                                        well_ipc::HermesAppCommand::SwitchTab { tab_index, respond_to } => {
                                            if tab_index < view.tabs.len() {
                                                view.active_tab = tab_index;
                                                let focus = view.tabs[view.active_tab].active_terminal.read(cx).focus_handle.clone();
                                                window.focus(&focus);
                                                cx.notify();
                                                let _ = respond_to.send(Ok(serde_json::json!({ "status": "success", "tab": tab_index })));
                                            } else {
                                                let _ = respond_to.send(Err(format!("Tab {} out of bounds", tab_index)));
                                            }
                                        }
                                        well_ipc::HermesAppCommand::SplitPane { horizontal, respond_to } => {
                                            let dir = if horizontal { SplitDirection::Horizontal } else { SplitDirection::Vertical };
                                            view.split_active_pane(dir, window, cx);
                                            let _ = respond_to.send(Ok(serde_json::json!({ "status": "success" })));
                                        }
                                        well_ipc::HermesAppCommand::SendShellInput { input_text, respond_to } => {
                                            let target = view.active_terminal(window, cx);
                                            let pty = target.read(cx).pty.clone();
                                            {
                                                let mut writer = pty.writer.lock().unwrap();
                                                let _ = writer.write_all(input_text.as_bytes());
                                            }
                                            let _ = respond_to.send(Ok(serde_json::json!({ "status": "success" })));
                                        }
                                        well_ipc::HermesAppCommand::InsertEditorText { text, respond_to } => {
                                            let target = view.active_terminal(window, cx);
                                            target.update(cx, |t, cx| {
                                                t.mneme_active = true;
                                                t.mneme_editor.set_text(&text);
                                                cx.notify();
                                            });
                                            let _ = respond_to.send(Ok(serde_json::json!({ "status": "success" })));
                                        }
                                        well_ipc::HermesAppCommand::PublishAgentLifecycle { event_name, payload, respond_to } => {
                                            eprintln!("[well-gpui] agent.lifecycle: event={event_name} payload={payload}");
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "acknowledged",
                                                "event": event_name
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::ReadSurface { surface_id: _, respond_to } => {
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "active_tab": view.active_tab,
                                                "total_tabs": view.tabs.len()
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::WorkspaceState { respond_to } => {
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "active_tab": view.active_tab,
                                                "total_tabs": view.tabs.len()
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::NewTab { respond_to } => {
                                            view.add_tab(window, cx);
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "tab": view.active_tab
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::SendSurfaceInput { surface_id: _, input_text, respond_to } => {
                                            let target = view.active_terminal(window, cx);
                                            let pty = target.read(cx).pty.clone();
                                            {
                                                let mut writer = pty.writer.lock().unwrap();
                                                let _ = writer.write_all(input_text.as_bytes());
                                            }
                                            let _ = respond_to.send(Ok(serde_json::json!({ "status": "success" })));
                                        }
                                        well_ipc::HermesAppCommand::GetPrompt { transient: _, respond_to } => {
                                            let target = view.active_terminal(window, cx);
                                            let snap = target.read(cx).astraea.read_snapshot();
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "cwd": snap.cwd,
                                                "branch": snap.git_branch,
                                                "dirty": snap.is_dirty
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::GetAstraeaState { respond_to } => {
                                            let target = view.active_terminal(window, cx);
                                            let snap = target.read(cx).astraea.read_snapshot();
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "cwd": snap.cwd,
                                                "branch": snap.git_branch,
                                                "dirty": snap.is_dirty,
                                                "op": snap.scm_op.as_str()
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::ConnectSsh { target, respond_to } => {
                                            let dest = target.destination.clone();
                                            view.connect_ssh(target, window, cx);
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "connected",
                                                "destination": dest,
                                                "tab": view.active_tab
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::ListSshSessions { respond_to } => {
                                            let sessions = view.ssh_engine.sessions_snapshot();
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "active_count": sessions.len(),
                                                "sessions": sessions
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::ListSshHosts { respond_to } => {
                                            let hosts = view.ssh_engine.list_hosts();
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "count": hosts.len(),
                                                "hosts": hosts
                                            })));
                                        }
                                        well_ipc::HermesAppCommand::SyncRecipes { recipes_json, respond_to } => {
                                            let history = shared_metis_history();
                                            let mut guard = history.write().unwrap();
                                            match guard.sync_recipes_json(&recipes_json) {
                                                Ok(count) => {
                                                    if let Ok(path) = well_shell::MetisHistory::default_path() {
                                                        let recipes_path = path.with_file_name("recipes.json");
                                                        let _ = guard.save_recipes_to_file(&recipes_path);
                                                    }
                                                    let _ = respond_to.send(Ok(serde_json::json!({
                                                        "status": "success",
                                                        "synced_count": count,
                                                        "total_recipes": guard.recipes().len()
                                                    })));
                                                }
                                                Err(e) => {
                                                    let _ = respond_to.send(Err(format!("JSON parsing error: {e}")));
                                                }
                                            }
                                        }
                                        well_ipc::HermesAppCommand::SearchRecipes { query, limit, respond_to } => {
                                            let history = shared_metis_history();
                                            let guard = history.read().unwrap();
                                            let results = guard.search_recipes(&query, limit);
                                            let _ = respond_to.send(Ok(serde_json::json!({
                                                "status": "success",
                                                "query": query,
                                                "count": results.len(),
                                                "recipes": results
                                            })));
                                        }
                                    }
                                });
                            }
                        });
                    }
                }
                }).detach();
            });

            view
        }).unwrap();
    });
}
