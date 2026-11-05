//! Categorized Two-Pane Settings UI (`⌘,`) for Well GPUI.
//! Backed by `well-config` atomic TOML persistence (`ConfigStore`),
//! `~/.config/well/operator_settings.json` full operator state persistence, and `WELL_ACTIONS`.

use crate::actions::{detect_keybind_conflicts, WELL_ACTIONS};
use crate::ssh_console::{SshHostEditorDraft, SshTunnelEntry};
use crate::theme::{parse_hex_color, rgba_to_hex, CustomThemeHex, Theme};
use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use well_editor::{SnippetScope, WellSnippet};
use well_shell::ssh::{SshConfigFile, SshHostConfig};
use well_shell::theia_ssh::TheiaSshProfile;

fn default_true() -> bool {
    true
}

fn default_cursor_blink_ms() -> u64 {
    500
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CursorShape {
    #[default]
    Block,
    Beam,
    Underline,
}

impl CursorShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::Block => "▋ Block",
            Self::Beam => "▏ Beam",
            Self::Underline => "▁ Underline",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedOperatorSettings {
    pub theme_id: u32,
    #[serde(default)]
    pub custom_theme: Option<CustomThemeHex>,
    pub font_family: String,
    pub font_size: f32,
    pub per_host_ssh_theme: bool,
    pub kitty_graphics_enabled: bool,
    #[serde(default = "default_true")]
    pub geometric_border: bool,
    #[serde(default)]
    pub cursor_shape: CursorShape,
    #[serde(default = "default_true")]
    pub cursor_blink: bool,
    #[serde(default = "default_cursor_blink_ms")]
    pub cursor_blink_ms: u64,
    #[serde(default = "default_true")]
    pub cursor_use_accent: bool,
    #[serde(default)]
    pub show_line_numbers: bool,
    pub snippets: Vec<WellSnippet>,
    pub ssh_profiles: Vec<TheiaSshProfile>,
    #[serde(default)]
    pub ssh_tunnels: Vec<SshTunnelEntry>,
    pub custom_bindings: Vec<(String, String, String)>,
    pub gate_rm_root: bool,
    pub gate_kubectl_delete: bool,
    pub gate_git_force_push: bool,
    pub gate_terraform_destroy: bool,
    pub metis_offline_cache_priority: bool,
}

pub struct TerminalSettings {
    pub font_family: String,
    pub font_size: f32,
    pub theme: Theme,
    pub custom_theme: CustomThemeHex,
    pub per_host_ssh_theme: bool,
    pub kitty_graphics_enabled: bool,
    pub geometric_border: bool,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,
    pub cursor_blink_ms: u64,
    pub cursor_use_accent: bool,
    pub show_line_numbers: bool,
    pub snippets: Vec<WellSnippet>,
    pub ssh_profiles: Vec<TheiaSshProfile>,
    pub ssh_tunnels: Vec<SshTunnelEntry>,
    pub custom_bindings: Vec<(String, String, String)>,
    pub gate_rm_root: bool,
    pub gate_kubectl_delete: bool,
    pub gate_git_force_push: bool,
    pub gate_terraform_destroy: bool,
    pub metis_offline_cache_priority: bool,
    pub last_saved_status: Option<String>,
    pub modal_open: bool,
    pub settings_open: bool,
    pub requested_hub_switch: Option<&'static str>,
    pub requested_snippet_insert: Option<String>,
}

pub fn extract_printable_char(keystroke: &gpui::Keystroke) -> Option<String> {
    if keystroke.modifiers.platform || keystroke.modifiers.control {
        return None;
    }
    if let Some(ch) = &keystroke.key_char {
        if !ch.is_empty() && !ch.chars().all(|c| c.is_control()) {
            return Some(ch.clone());
        }
    }
    match keystroke.key.as_str() {
        "space" => Some(" ".to_string()),
        s if s.len() == 1 => {
            if keystroke.modifiers.shift {
                Some(s.to_ascii_uppercase())
            } else {
                Some(s.to_string())
            }
        }
        _ => None,
    }
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalSettings {
    pub fn operator_settings_path() -> PathBuf {
        if let Ok(path) = well_config::ConfigStore::default_path() {
            path.with_file_name("operator_settings.json")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home)
                .join(".config")
                .join("well")
                .join("operator_settings.json")
        }
    }

    pub fn default_bindings() -> Vec<(String, String, String)> {
        WELL_ACTIONS
            .iter()
            .map(|a| {
                (
                    a.id.to_string(),
                    a.default_keybind.to_string(),
                    a.key_context.to_string(),
                )
            })
            .collect()
    }

    pub fn new() -> Self {
        let mut font_family = "Menlo".into();
        let mut font_size = 14.0;
        if let Ok(path) = well_config::ConfigStore::default_path() {
            if let Ok((_, prefs)) = well_config::ConfigStore::load(path) {
                if !prefs.appearance.font_family.is_empty()
                    && prefs.appearance.font_family != "OpenDyslexic Nerd Font"
                {
                    font_family = prefs.appearance.font_family;
                }
                font_size = prefs.appearance.font_size;
            }
        }

        let op_path = Self::operator_settings_path();
        if let Ok(raw) = std::fs::read_to_string(&op_path) {
            if let Ok(persisted) = serde_json::from_str::<PersistedOperatorSettings>(&raw) {
                let custom_theme = persisted
                    .custom_theme
                    .unwrap_or_else(|| CustomThemeHex::from_theme(&Theme::get(persisted.theme_id)));
                let theme = if persisted.theme_id == 5 {
                    custom_theme.to_theme()
                } else {
                    Theme::get(persisted.theme_id)
                };
                return Self {
                    font_family: persisted.font_family,
                    font_size: persisted.font_size,
                    theme,
                    custom_theme,
                    per_host_ssh_theme: persisted.per_host_ssh_theme,
                    kitty_graphics_enabled: persisted.kitty_graphics_enabled,
                    geometric_border: persisted.geometric_border,
                    cursor_shape: persisted.cursor_shape,
                    cursor_blink: persisted.cursor_blink,
                    cursor_blink_ms: persisted.cursor_blink_ms.clamp(200, 1200),
                    cursor_use_accent: persisted.cursor_use_accent,
                    show_line_numbers: persisted.show_line_numbers,
                    snippets: if persisted.snippets.is_empty() {
                        WellSnippet::builtin_catalog()
                    } else {
                        persisted.snippets
                    },
                    ssh_profiles: persisted.ssh_profiles,
                    ssh_tunnels: if persisted.ssh_tunnels.is_empty() {
                        SshTunnelEntry::default_catalog()
                    } else {
                        persisted.ssh_tunnels
                    },
                    custom_bindings: if persisted.custom_bindings.len() == WELL_ACTIONS.len() {
                        persisted.custom_bindings
                    } else {
                        Self::default_bindings()
                    },
                    gate_rm_root: persisted.gate_rm_root,
                    gate_kubectl_delete: persisted.gate_kubectl_delete,
                    gate_git_force_push: persisted.gate_git_force_push,
                    gate_terraform_destroy: persisted.gate_terraform_destroy,
                    metis_offline_cache_priority: persisted.metis_offline_cache_priority,
                    last_saved_status: Some(format!("Loaded from {}", op_path.display())),
                    modal_open: false,
                    settings_open: false,
                    requested_hub_switch: None,
                    requested_snippet_insert: None,
                };
            }
        }

        let ssh_cfg = SshConfigFile::load_default();
        let mut ssh_profiles: Vec<TheiaSshProfile> = ssh_cfg
            .hosts
            .into_iter()
            .map(TheiaSshProfile::from_host)
            .collect();
        if ssh_profiles.is_empty() {
            ssh_profiles.push(TheiaSshProfile::from_host(SshHostConfig {
                name: "prod-k8s-cluster".into(),
                hostname: "10.240.0.10".into(),
                user: Some("ops".into()),
                port: 22,
                identity_file: Some(PathBuf::from("~/.ssh/id_ed25519")),
                proxy_jump: Some("bastion.internal".into()),
                forward_agent: true,
                options: Default::default(),
            }));
            ssh_profiles.push(TheiaSshProfile::from_host(SshHostConfig {
                name: "staging-worker-01".into(),
                hostname: "10.240.1.25".into(),
                user: Some("deploy".into()),
                port: 2222,
                identity_file: Some(PathBuf::from("~/.ssh/id_ed25519")),
                proxy_jump: None,
                forward_agent: false,
                options: Default::default(),
            }));
        }

        let default_theme = Theme::get(0);
        Self {
            font_family,
            font_size,
            custom_theme: CustomThemeHex::from_theme(&default_theme),
            theme: default_theme,
            per_host_ssh_theme: true,
            kitty_graphics_enabled: true,
            geometric_border: true,
            cursor_shape: CursorShape::Block,
            cursor_blink: true,
            cursor_blink_ms: 500,
            cursor_use_accent: true,
            show_line_numbers: false,
            snippets: WellSnippet::builtin_catalog(),
            ssh_profiles,
            ssh_tunnels: SshTunnelEntry::default_catalog(),
            custom_bindings: Self::default_bindings(),
            gate_rm_root: true,
            gate_kubectl_delete: true,
            gate_git_force_push: true,
            gate_terraform_destroy: true,
            metis_offline_cache_priority: true,
            last_saved_status: None,
            modal_open: false,
            settings_open: false,
            requested_hub_switch: None,
            requested_snippet_insert: None,
        }
    }

    /// Persist all operator settings atomically to `~/.config/well/operator_settings.json`
    /// and sync typography to `well-config::ConfigStore` (`settings.toml`).
    pub fn save_to_disk(&mut self) -> Result<String, String> {
        if let Ok(path) = well_config::ConfigStore::default_path() {
            if let Ok((mut store, mut prefs)) = well_config::ConfigStore::load(path) {
                prefs.appearance.font_family = self.font_family.clone();
                prefs.appearance.font_size = self.font_size;
                prefs.appearance.geometric_border = self.geometric_border;
                let _ = store.save(&prefs);
            }
        }

        let op_path = Self::operator_settings_path();
        if let Some(parent) = op_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let payload = PersistedOperatorSettings {
            theme_id: self.theme.id,
            custom_theme: Some(self.custom_theme.clone()),
            font_family: self.font_family.clone(),
            font_size: self.font_size,
            per_host_ssh_theme: self.per_host_ssh_theme,
            kitty_graphics_enabled: self.kitty_graphics_enabled,
            geometric_border: self.geometric_border,
            cursor_shape: self.cursor_shape,
            cursor_blink: self.cursor_blink,
            cursor_blink_ms: self.cursor_blink_ms,
            cursor_use_accent: self.cursor_use_accent,
            show_line_numbers: self.show_line_numbers,
            snippets: self.snippets.clone(),
            ssh_profiles: self.ssh_profiles.clone(),
            ssh_tunnels: self.ssh_tunnels.clone(),
            custom_bindings: self.custom_bindings.clone(),
            gate_rm_root: self.gate_rm_root,
            gate_kubectl_delete: self.gate_kubectl_delete,
            gate_git_force_push: self.gate_git_force_push,
            gate_terraform_destroy: self.gate_terraform_destroy,
            metis_offline_cache_priority: self.metis_offline_cache_priority,
        };

        let json = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
        let tmp_path = op_path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp_path, &op_path).map_err(|e| e.to_string())?;

        let msg = format!("Saved all changes to {}", op_path.display());
        self.last_saved_status = Some(msg.clone());
        Ok(msg)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    AppearancePrism,
    KeyboardShortcuts,
    SnippetsWorkflows,
    TheiaSshHosts,
    MnemeSafetyGates,
    PythiaTelemetry,
    SystemInfoGuide,
}

impl SettingsSection {
    pub const ALL: &'static [Self] = &[
        Self::AppearancePrism,
        Self::KeyboardShortcuts,
        Self::SnippetsWorkflows,
        Self::TheiaSshHosts,
        Self::MnemeSafetyGates,
        Self::PythiaTelemetry,
        Self::SystemInfoGuide,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::AppearancePrism => "1. Appearance & Theia's Prism",
            Self::KeyboardShortcuts => "2. Keyboard Shortcuts",
            Self::SnippetsWorkflows => "3. Snippets & Workflows",
            Self::TheiaSshHosts => "4. Theia SSH Hosts",
            Self::MnemeSafetyGates => "5. Mneme Safety Gates",
            Self::PythiaTelemetry => "6. Pythia AI & Telemetry",
            Self::SystemInfoGuide => "7. Info, Names & Guide",
        }
    }

    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::AppearancePrism => "Themes, per-host SSH palettes & typography",
            Self::KeyboardShortcuts => "All labeled functions & conflict checks",
            Self::SnippetsWorkflows => "Parameterized ${1:arg} tab-stop templates",
            Self::TheiaSshHosts => "Host profiles, redacted keys & IPC tunnel",
            Self::MnemeSafetyGates => "Tree-sitter AST destructive command gates",
            Self::PythiaTelemetry => "Sub-2ms Metis cache & Caduceus IPC health",
            Self::SystemInfoGuide => "Mythological names, subsystems & functions",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveFormInput {
    None,
    ThemeHex(usize),
    ShortcutsFilter,
    SnippetTrigger,
    SnippetLabel,
    SnippetTemplate,
    SshName,
    SshHostname,
    SshUser,
    SshPort,
    SshIdentityFile,
    SshProxyJump,
}

pub struct SettingsView {
    pub focus_handle: FocusHandle,
    pub settings: Entity<TerminalSettings>,
    pub active_section: SettingsSection,
    pub theme_hex_inputs: [String; 6],
    pub shortcuts_page: usize,
    pub shortcuts_filter: String,
    pub rebinding_index: Option<usize>,
    pub active_input: ActiveFormInput,
    pub new_snippet_trigger: String,
    pub new_snippet_label: String,
    pub new_snippet_template: String,
    pub ssh_draft: SshHostEditorDraft,
    pub status_msg: Option<String>,
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>, settings: Entity<TerminalSettings>) -> Self {
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        let (initial_status, initial_hex) = {
            let s = settings.read(cx);
            let t = &s.theme;
            (
                s.last_saved_status.clone(),
                [
                    rgba_to_hex(t.bg),
                    rgba_to_hex(t.fg),
                    rgba_to_hex(t.border_active),
                    rgba_to_hex(t.border_inactive),
                    rgba_to_hex(t.tab_active_bg),
                    rgba_to_hex(t.tab_inactive_bg),
                ],
            )
        };

        Self {
            focus_handle: cx.focus_handle(),
            settings,
            active_section: SettingsSection::AppearancePrism,
            theme_hex_inputs: initial_hex,
            shortcuts_page: 0,
            shortcuts_filter: String::new(),
            rebinding_index: None,
            active_input: ActiveFormInput::None,
            new_snippet_trigger: ":deploy".into(),
            new_snippet_label: "Rollout Restart Deployment".into(),
            new_snippet_template: "kubectl rollout restart deployment/${1:app} -n ${2:prod}".into(),
            ssh_draft: SshHostEditorDraft::default(),
            status_msg: initial_status,
        }
    }

    fn sync_hex_inputs_from_theme(&mut self, theme: &Theme) {
        self.theme_hex_inputs = [
            rgba_to_hex(theme.bg),
            rgba_to_hex(theme.fg),
            rgba_to_hex(theme.border_active),
            rgba_to_hex(theme.border_inactive),
            rgba_to_hex(theme.tab_active_bg),
            rgba_to_hex(theme.tab_inactive_bg),
        ];
    }

    fn apply_custom_theme_if_valid(&mut self, slot: usize, cx: &mut Context<Self>) {
        let candidate = self.theme_hex_inputs[slot].trim().to_string();
        if let Some(parsed) = parse_hex_color(&candidate) {
            let formatted = format!("#{parsed:06X}");
            let all_hex = self.theme_hex_inputs.clone();
            self.settings.update(cx, |s, cx| {
                s.custom_theme = CustomThemeHex {
                    name: "Custom Theme".into(),
                    bg: if slot == 0 {
                        formatted.clone()
                    } else if parse_hex_color(&all_hex[0]).is_some() {
                        all_hex[0].clone()
                    } else {
                        rgba_to_hex(s.theme.bg)
                    },
                    fg: if slot == 1 {
                        formatted.clone()
                    } else if parse_hex_color(&all_hex[1]).is_some() {
                        all_hex[1].clone()
                    } else {
                        rgba_to_hex(s.theme.fg)
                    },
                    border_active: if slot == 2 {
                        formatted.clone()
                    } else if parse_hex_color(&all_hex[2]).is_some() {
                        all_hex[2].clone()
                    } else {
                        rgba_to_hex(s.theme.border_active)
                    },
                    border_inactive: if slot == 3 {
                        formatted.clone()
                    } else if parse_hex_color(&all_hex[3]).is_some() {
                        all_hex[3].clone()
                    } else {
                        rgba_to_hex(s.theme.border_inactive)
                    },
                    tab_active_bg: if slot == 4 {
                        formatted.clone()
                    } else if parse_hex_color(&all_hex[4]).is_some() {
                        all_hex[4].clone()
                    } else {
                        rgba_to_hex(s.theme.tab_active_bg)
                    },
                    tab_inactive_bg: if slot == 5 {
                        formatted.clone()
                    } else if parse_hex_color(&all_hex[5]).is_some() {
                        all_hex[5].clone()
                    } else {
                        rgba_to_hex(s.theme.tab_inactive_bg)
                    },
                };
                s.theme = s.custom_theme.to_theme();
                let _ = s.save_to_disk();
                cx.notify();
            });
            self.status_msg = Some(format!(
                "✓ Applied & saved custom theme color {formatted} to disk"
            ));
            cx.notify();
        }
    }

    fn save_all(&mut self, cx: &mut Context<Self>) {
        let mut status = None;
        self.settings.update(cx, |s, cx| {
            match s.save_to_disk() {
                Ok(msg) => status = Some(format!("✓ {msg}")),
                Err(err) => status = Some(format!("⚠ Save error: {err}")),
            }
            cx.notify();
        });
        self.status_msg = status;
        cx.notify();
    }

    fn reset_shortcuts_to_default(&mut self, cx: &mut Context<Self>) {
        self.settings.update(cx, |s, cx| {
            s.custom_bindings = TerminalSettings::default_bindings();
            let _ = s.save_to_disk();
            cx.notify();
        });
        self.rebinding_index = None;
        self.status_msg = Some("✓ Reset & saved all keyboard shortcuts to factory defaults".into());
        cx.notify();
    }

    pub fn set_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.active_section = section;
        self.rebinding_index = None;
        self.active_input = ActiveFormInput::None;
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus_handle.is_focused(window) {
            window.focus(&self.focus_handle);
        }
        let active_section = self.active_section;
        let theme = self.settings.read(cx).theme.clone();
        let status_msg = self
            .status_msg
            .clone()
            .or_else(|| self.settings.read(cx).last_saved_status.clone())
            .unwrap_or_else(|| {
                "All changes auto-save & can be explicitly saved via ✓ Save & Close (⌘S)".into()
            });
        let settings_entity = self.settings.clone();

        // Left Sidebar Navigation (7 Clear Sections)
        let mut nav_items = div().flex().flex_col().gap_1();
        for (idx, section) in SettingsSection::ALL.iter().copied().enumerate() {
            let is_active = section == active_section;
            nav_items = nav_items.child(
                div()
                    .id(("settings-nav", idx))
                    .px_2p5()
                    .py_1()
                    .rounded_lg()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .bg(if is_active {
                        theme.border_active
                    } else {
                        theme.tab_active_bg
                    })
                    .border_1()
                    .border_color(if is_active {
                        theme.border_active
                    } else {
                        theme.border_inactive
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.active_section = section;
                        view.rebinding_index = None;
                        view.active_input = ActiveFormInput::None;
                        cx.notify();
                    }))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if is_active {
                                theme.bg
                            } else {
                                theme.fg
                            })
                            .child(section.title()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(if is_active {
                                theme.tab_inactive_bg
                            } else {
                                rgb(0x928374)
                            })
                            .child(section.subtitle()),
                    ),
            );
        }

        let sidebar = div()
            .id("settings-left-sidebar")
            .w(px(238.0))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .bg(theme.tab_inactive_bg)
            .border_r_1()
            .border_color(theme.border_inactive)
            .p_2p5()
            .flex()
            .flex_col()
            .justify_between()
            .gap_2()
            .overflow_y_scroll()
            .child(nav_items)
            .child(
                div()
                    .p_2p5()
                    .rounded_lg()
                    .bg(theme.bg)
                    .border_1()
                    .border_color(theme.border_inactive)
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
                                    .text_size(px(10.5))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child(format!("◈ {}", theme.name)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_1()
                                    .child(div().size(px(7.0)).rounded_full().bg(theme.bg))
                                    .child(div().size(px(7.0)).rounded_full().bg(theme.border_active))
                                    .child(div().size(px(7.0)).rounded_full().bg(rgb(0x22d3ee)))
                                    .child(div().size(px(7.0)).rounded_full().bg(rgb(0xa855f7)))
                                    .child(div().size(px(7.0)).rounded_full().bg(theme.fg)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x928374))
                            .child("Live sync → ~/.config/well/"),
                    ),
            );

        // Right Detail Pane
        let right_pane: AnyElement = match active_section {
            SettingsSection::AppearancePrism => {
                let s = self.settings.read(cx);
                let current_theme_id = s.theme.id;
                let custom_theme_cfg = s.custom_theme.clone();
                let font_size = s.font_size;
                let font_family = s.font_family.clone();
                let per_host_ssh_theme = s.per_host_ssh_theme;
                let kitty_enabled = s.kitty_graphics_enabled;
                let geometric_border = s.geometric_border;
                let cursor_shape = s.cursor_shape;
                let cursor_blink = s.cursor_blink;
                let cursor_blink_ms = s.cursor_blink_ms;
                let cursor_use_accent = s.cursor_use_accent;
                let show_line_numbers = s.show_line_numbers;
                let active_in = self.active_input;
                let hex_inputs = self.theme_hex_inputs.clone();

                let custom_preview = [
                    parse_hex_color(&custom_theme_cfg.bg).unwrap_or(0x1d2021),
                    parse_hex_color(&custom_theme_cfg.border_active).unwrap_or(0xfe8019),
                    parse_hex_color(&custom_theme_cfg.tab_active_bg).unwrap_or(0x282828),
                    parse_hex_color(&custom_theme_cfg.fg).unwrap_or(0xebdbb2),
                ];

                let themes = [
                    (
                        0,
                        "Theia's Prism",
                        [0x1d2021, 0xfe8019, 0x8ec07c, 0xebdbb2],
                    ),
                    (1, "Cyber-Neon", [0x0a0a0f, 0x00ffff, 0xff003c, 0x39ff14]),
                    (2, "Tokyo Night", [0x1a1b26, 0x7aa2f7, 0xf7768e, 0x9ece6a]),
                    (3, "Dracula", [0x282a36, 0xbd93f9, 0x50fa7b, 0xff5555]),
                    (
                        4,
                        "Crimson Prod",
                        [0x221112, 0xfb4934, 0xcc241d, 0xfbf1c7],
                    ),
                    (5, "✨ Custom Theme", custom_preview),
                ];

                let mut theme_cards = div().flex().flex_row().gap_1p5().flex_wrap();
                for (id, name, colors) in themes {
                    let is_sel = current_theme_id == id;
                    let sm = settings_entity.clone();
                    theme_cards = theme_cards.child(
                        div()
                            .id(("theme-card", id))
                            .px_2p5()
                            .py_1p5()
                            .rounded_lg()
                            .border_2()
                            .border_color(if is_sel { theme.border_active } else { theme.border_inactive })
                            .bg(if is_sel { theme.tab_active_bg } else { theme.bg })
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                let mut chosen = Theme::theia_prism();
                                sm.update(cx, |s, cx| {
                                    if id == 5 {
                                        s.theme = s.custom_theme.to_theme();
                                    } else {
                                        s.theme = Theme::get(id);
                                    }
                                    chosen = s.theme.clone();
                                    let _ = s.save_to_disk();
                                    cx.notify();
                                });
                                view.sync_hex_inputs_from_theme(&chosen);
                                view.active_input = ActiveFormInput::None;
                                view.status_msg = Some(format!("✓ Active theme switched to '{name}' & saved to disk"));
                                cx.notify();
                            }))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(if is_sel { theme.border_active } else { theme.fg })
                                    .child(if is_sel {
                                        format!("✓ {name}")
                                    } else {
                                        name.to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_1()
                                    .child(div().size(px(9.0)).rounded_full().bg(rgb(colors[0])))
                                    .child(div().size(px(9.0)).rounded_full().bg(rgb(colors[1])))
                                    .child(div().size(px(9.0)).rounded_full().bg(rgb(colors[2])))
                                    .child(div().size(px(9.0)).rounded_full().bg(rgb(colors[3]))),
                            ),
                    );
                }

                // Custom Theme Studio — 6 Editable Color Token Slots (2-Row Cards for Zero Clipping)
                let slot_defs: [(usize, &'static str, [u32; 6]); 6] = [
                    (
                        0,
                        "Background (bg)",
                        [0x0b0f17, 0x162030, 0x1d2021, 0x1a1b26, 0x282a36, 0x221112],
                    ),
                    (
                        1,
                        "Foreground (fg)",
                        [0xebdbb2, 0x00ffff, 0x7dcfff, 0xc0caf5, 0xf8f8f2, 0xfbf1c7],
                    ),
                    (
                        2,
                        "Accent / Border",
                        [0x22d3ee, 0xfe8019, 0x00ffff, 0x7aa2f7, 0xbd93f9, 0xfb4934],
                    ),
                    (
                        3,
                        "Muted Border",
                        [0x1e293b, 0x3c3836, 0x222233, 0x292e42, 0x44475a, 0x5c2320],
                    ),
                    (
                        4,
                        "Card / Tab Surface",
                        [0x162030, 0x282828, 0x1a1a24, 0x24283b, 0x44475a, 0x3c1818],
                    ),
                    (
                        5,
                        "Header / Sidebar",
                        [0x0b0f17, 0x1d2021, 0x0f0f15, 0x1f2335, 0x282a36, 0x221112],
                    ),
                ];

                let build_slot_card = |slot_idx: usize,
                                       slot_label: &'static str,
                                       swatches: [u32; 6],
                                       cx: &mut Context<Self>| {
                    let raw_hex = hex_inputs[slot_idx].clone();
                    let is_editing = active_in == ActiveFormInput::ThemeHex(slot_idx);
                    let preview_rgb = parse_hex_color(&raw_hex).map(rgb).unwrap_or(match slot_idx {
                        0 => theme.bg,
                        1 => theme.fg,
                        2 => theme.border_active,
                        3 => theme.border_inactive,
                        4 => theme.tab_active_bg,
                        _ => theme.tab_inactive_bg,
                    });

                    let mut swatch_row = div().flex().flex_row().items_center().gap_1p5();
                    for (sw_i, sw_hex) in swatches.into_iter().enumerate() {
                        let formatted_sw = format!("#{sw_hex:06X}");
                        let is_current_sw = raw_hex.eq_ignore_ascii_case(&formatted_sw);
                        swatch_row = swatch_row.child(
                            div()
                                .id(("custom-theme-swatch", slot_idx * 10 + sw_i))
                                .size(px(14.0))
                                .rounded_full()
                                .bg(rgb(sw_hex))
                                .border_1()
                                .border_color(if is_current_sw {
                                    rgb(0xffffff)
                                } else {
                                    rgb(0x64748b)
                                })
                                .cursor(gpui::CursorStyle::PointingHand)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.theme_hex_inputs[slot_idx] = formatted_sw.clone();
                                    view.apply_custom_theme_if_valid(slot_idx, cx);
                                })),
                        );
                    }

                    div()
                        .flex_1()
                        .min_w_0()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .bg(theme.bg)
                        .border_1()
                        .border_color(if is_editing {
                            theme.border_active
                        } else {
                            theme.border_inactive
                        })
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .size(px(14.0))
                                                .rounded_sm()
                                                .bg(preview_rgb)
                                                .border_1()
                                                .border_color(theme.border_active),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.fg)
                                                .child(slot_label),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(("theme-hex-input", slot_idx))
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(theme.tab_active_bg)
                                        .border_1()
                                        .border_color(if is_editing {
                                            theme.border_active
                                        } else {
                                            theme.border_inactive
                                        })
                                        .cursor(gpui::CursorStyle::IBeam)
                                        .font_family("Menlo")
                                        .text_size(px(10.5))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(if is_editing {
                                            theme.border_active
                                        } else {
                                            theme.fg
                                        })
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            view.active_input = ActiveFormInput::ThemeHex(slot_idx);
                                            window.focus(&view.focus_handle);
                                            cx.notify();
                                        }))
                                        .child(if is_editing {
                                            format!("{raw_hex}▏")
                                        } else {
                                            raw_hex
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
                                        .text_size(px(9.5))
                                        .text_color(rgb(0x928374))
                                        .child("Quick swatches:"),
                                )
                                .child(swatch_row),
                        )
                };

                let row_a = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(build_slot_card(slot_defs[0].0, slot_defs[0].1, slot_defs[0].2, cx))
                    .child(build_slot_card(slot_defs[1].0, slot_defs[1].1, slot_defs[1].2, cx));
                let row_b = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(build_slot_card(slot_defs[2].0, slot_defs[2].1, slot_defs[2].2, cx))
                    .child(build_slot_card(slot_defs[3].0, slot_defs[3].1, slot_defs[3].2, cx));
                let row_c = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(build_slot_card(slot_defs[4].0, slot_defs[4].1, slot_defs[4].2, cx))
                    .child(build_slot_card(slot_defs[5].0, slot_defs[5].1, slot_defs[5].2, cx));

                let sm_dec = settings_entity.clone();
                let sm_inc = settings_entity.clone();
                let sm_ssh_theme = settings_entity.clone();
                let sm_kitty = settings_entity.clone();
                let sm_geom = settings_entity.clone();
                let sm_cursor_blink = settings_entity.clone();
                let sm_cursor_color = settings_entity.clone();
                let sm_line_numbers = settings_entity.clone();

                let font_families = ["Menlo", "SF Mono", "JetBrains Mono", "Monaco"];
                let mut font_pills = div().flex().flex_row().items_center().gap_1p5();
                for (f_idx, f_name) in font_families.into_iter().enumerate() {
                    let is_font_sel = font_family == f_name;
                    let sm_font = settings_entity.clone();
                    font_pills = font_pills.child(
                        div()
                            .id(("font-family-pill", f_idx))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(if is_font_sel {
                                theme.border_active
                            } else {
                                theme.border_inactive
                            })
                            .bg(if is_font_sel {
                                theme.border_active
                            } else {
                                theme.bg
                            })
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if is_font_sel { theme.bg } else { theme.fg })
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                sm_font.update(cx, |s, cx| {
                                    s.font_family = f_name.to_string();
                                    let _ = s.save_to_disk();
                                    cx.notify();
                                });
                                view.status_msg = Some(format!("✓ Switched font family to '{f_name}' & saved"));
                                cx.notify();
                            }))
                            .child(f_name),
                    );
                }

                let cursor_shapes = [
                    CursorShape::Block,
                    CursorShape::Beam,
                    CursorShape::Underline,
                ];
                let mut cursor_shape_pills = div().flex().flex_row().items_center().gap_1();
                for (c_idx, shape_opt) in cursor_shapes.into_iter().enumerate() {
                    let is_shape_sel = cursor_shape == shape_opt;
                    let sm_shape = settings_entity.clone();
                    cursor_shape_pills = cursor_shape_pills.child(
                        div()
                            .id(("cursor-shape-pill", c_idx))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(if is_shape_sel {
                                theme.border_active
                            } else {
                                theme.border_inactive
                            })
                            .bg(if is_shape_sel {
                                theme.border_active
                            } else {
                                theme.bg
                            })
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if is_shape_sel { theme.bg } else { theme.fg })
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                sm_shape.update(cx, |s, cx| {
                                    s.cursor_shape = shape_opt;
                                    let _ = s.save_to_disk();
                                    cx.notify();
                                });
                                view.status_msg = Some(format!(
                                    "✓ Set terminal cursor style to '{}' & saved",
                                    shape_opt.label()
                                ));
                                cx.notify();
                            }))
                            .child(shape_opt.label()),
                    );
                }

                let blink_speeds: [(u64, &'static str); 3] = [
                    (350, "Fast 350ms"),
                    (500, "500ms"),
                    (750, "Calm 750ms"),
                ];
                let mut blink_speed_pills = div().flex().flex_row().items_center().gap_1();
                for (b_idx, (ms_val, ms_label)) in blink_speeds.into_iter().enumerate() {
                    let is_speed_sel = cursor_blink_ms == ms_val;
                    let sm_speed = settings_entity.clone();
                    blink_speed_pills = blink_speed_pills.child(
                        div()
                            .id(("cursor-blink-speed-pill", b_idx))
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(if is_speed_sel {
                                theme.border_active
                            } else {
                                theme.border_inactive
                            })
                            .bg(if is_speed_sel {
                                theme.tab_active_bg
                            } else {
                                theme.bg
                            })
                            .text_size(px(10.0))
                            .font_weight(if is_speed_sel {
                                gpui::FontWeight::BOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .text_color(if is_speed_sel {
                                theme.border_active
                            } else {
                                rgb(0x928374)
                            })
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                sm_speed.update(cx, |s, cx| {
                                    s.cursor_blink_ms = ms_val;
                                    s.cursor_blink = true;
                                    let _ = s.save_to_disk();
                                    cx.notify();
                                });
                                view.status_msg = Some(format!(
                                    "✓ Set cursor blink interval to {ms_val}ms & saved"
                                ));
                                cx.notify();
                            }))
                            .child(ms_label),
                    );
                }

                let preview_cursor_color = if cursor_use_accent {
                    theme.border_active
                } else {
                    theme.fg
                };
                let preview_cursor_glyph = match cursor_shape {
                    CursorShape::Block => "▋",
                    CursorShape::Beam => "▏",
                    CursorShape::Underline => "▁",
                };

                // Live Refractive Spectrum Bar & Mini-Terminal Preview
                let prism_preview_card = div()
                    .bg(theme.bg)
                    .border_1()
                    .border_color(theme.border_active)
                    .rounded_lg()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .w_full()
                            .h(px(4.0))
                            .rounded_full()
                            .overflow_hidden()
                            .flex()
                            .flex_row()
                            .child(div().flex_1().h_full().bg(theme.border_active))
                            .child(div().flex_1().h_full().bg(rgb(0x22d3ee)))
                            .child(div().flex_1().h_full().bg(rgb(0xa855f7)))
                            .child(div().flex_1().h_full().bg(rgb(0x8ec07c)))
                            .child(div().flex_1().h_full().bg(rgb(0xfabd2f)))
                            .child(div().flex_1().h_full().bg(theme.fg)),
                    )
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
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_active)
                                            .text_size(px(10.0))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.border_active)
                                            .child(format!("◈ PRISM LIVE: {}", theme.name)),
                                    )
                                    .children(if show_line_numbers {
                                        Some(
                                            div()
                                                .font_family(font_family.clone())
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.border_active)
                                                .child("  1 │ "),
                                        )
                                    } else {
                                        None
                                    })
                                    .child(
                                        div()
                                            .font_family(font_family.clone())
                                            .text_xs()
                                            .text_color(theme.fg)
                                            .child("yocan@trismagistic ~ % "),
                                    )
                                    .child(
                                        div()
                                            .font_family(font_family.clone())
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x8ec07c))
                                            .child("well --prism-refract --live"),
                                    )
                                    .child(
                                        div()
                                            .font_family(font_family.clone())
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(preview_cursor_color)
                                            .child(preview_cursor_glyph),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0x928374))
                                    .child(format!(
                                        "{font_family} · {font_size:.0}px · {} {}{}",
                                        cursor_shape.label(),
                                        if cursor_blink {
                                            format!("({cursor_blink_ms}ms blink)")
                                        } else {
                                            "(steady)".into()
                                        },
                                        if show_line_numbers { " · 1│ln" } else { "" }
                                    )),
                            ),
                    );

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(prism_preview_card)
                    .child(
                        div()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .rounded_lg()
                            .p_2()
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
                                            .text_color(theme.border_active)
                                            .child("🎨 Theme Presets & Custom Theme Slot"),
                                    )
                                    .child(
                                        div()
                                            .id("toggle-per-host-ssh-theme")
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_ssh_theme.update(cx, |s, cx| {
                                                    s.per_host_ssh_theme = !s.per_host_ssh_theme;
                                                    let _ = s.save_to_disk();
                                                    cx.notify();
                                                });
                                                view.status_msg = Some("✓ Saved per-host SSH theme preference".into());
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(if per_host_ssh_theme {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .child(if per_host_ssh_theme { "☑" } else { "☐" }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.fg)
                                                    .child("Per-Host SSH Tier Auto-Tint"),
                                            ),
                                    ),
                            )
                            .child(theme_cards),
                    )
                    .child(
                        div()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .rounded_lg()
                            .p_2()
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
                                            .text_color(theme.border_active)
                                            .child("🖌️ Custom Theme Studio — Click Any Swatch or Type #RRGGBB"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme.bg)
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x8ec07c))
                                            .child("● Live Auto-Save"),
                                    ),
                            )
                            .child(row_a)
                            .child(row_b)
                            .child(row_c),
                    )
                    .child(
                        div()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .rounded_lg()
                            .p_2()
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
                                            .text_color(theme.border_active)
                                            .child("🔤 Typography, Cursor & Kitty Graphics Protocol"),
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
                                                    .text_color(theme.fg)
                                                    .child(format!("Font ({font_size:.0}px):")),
                                            )
                                            .child(font_pills)
                                            .child(
                                                div()
                                                    .id("font-dec-btn")
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme.border_inactive)
                                                    .text_xs()
                                                    .text_color(theme.fg)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        sm_dec.update(cx, |s, cx| {
                                                            s.font_size = (s.font_size - 1.0).max(9.0);
                                                            let _ = s.save_to_disk();
                                                            cx.notify();
                                                        });
                                                        view.status_msg = Some("✓ Saved font size to settings.toml".into());
                                                        cx.notify();
                                                    }))
                                                    .child("A-"),
                                            )
                                            .child(
                                                div()
                                                    .id("font-inc-btn")
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme.border_inactive)
                                                    .text_xs()
                                                    .text_color(theme.fg)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        sm_inc.update(cx, |s, cx| {
                                                            s.font_size = (s.font_size + 1.0).min(32.0);
                                                            let _ = s.save_to_disk();
                                                            cx.notify();
                                                        });
                                                        view.status_msg = Some("✓ Saved font size to settings.toml".into());
                                                        cx.notify();
                                                    }))
                                                    .child("A+"),
                                            ),
                                    ),
                            )
                            .child(
                                // Interactive Terminal Cursor Shape, Blink Toggle, Speed & Color Row
                                div()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme.bg)
                                    .border_1()
                                    .border_color(theme.border_inactive)
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.border_active)
                                                    .child("▋ Cursor:"),
                                            )
                                            .child(cursor_shape_pills),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .id("toggle-cursor-blink")
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(if cursor_blink {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        theme.border_inactive
                                                    })
                                                    .bg(if cursor_blink {
                                                        theme.tab_active_bg
                                                    } else {
                                                        theme.bg
                                                    })
                                                    .text_size(px(10.0))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(if cursor_blink {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        sm_cursor_blink.update(cx, |s, cx| {
                                                            s.cursor_blink = !s.cursor_blink;
                                                            let _ = s.save_to_disk();
                                                            cx.notify();
                                                        });
                                                        view.status_msg = Some(
                                                            "✓ Toggled terminal cursor blinking & saved".into(),
                                                        );
                                                        cx.notify();
                                                    }))
                                                    .child(if cursor_blink {
                                                        "☑ Blink ON"
                                                    } else {
                                                        "☐ Steady"
                                                    }),
                                            )
                                            .child(blink_speed_pills),
                                    )
                                    .child(
                                        div()
                                            .id("toggle-cursor-color-mode")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(theme.border_active)
                                            .bg(theme.tab_active_bg)
                                            .text_size(px(10.0))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(if cursor_use_accent {
                                                theme.border_active
                                            } else {
                                                theme.fg
                                            })
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_cursor_color.update(cx, |s, cx| {
                                                    s.cursor_use_accent = !s.cursor_use_accent;
                                                    let _ = s.save_to_disk();
                                                    cx.notify();
                                                });
                                                view.status_msg = Some(
                                                    "✓ Switched cursor color mode & saved".into(),
                                                );
                                                cx.notify();
                                            }))
                                            .child(if cursor_use_accent {
                                                "🎨 Color: Accent"
                                            } else {
                                                "⚪ Color: Text FG"
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("toggle-kitty-gfx")
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_kitty.update(cx, |s, cx| {
                                                    s.kitty_graphics_enabled =
                                                        !s.kitty_graphics_enabled;
                                                    let _ = s.save_to_disk();
                                                    cx.notify();
                                                });
                                                view.status_msg =
                                                    Some("✓ Saved Kitty graphics preference".into());
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_color(if kitty_enabled {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .child(if kitty_enabled { "☑" } else { "☐" }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.fg)
                                                    .child("Inline Kitty Graphics (\\e_Gf=100...)"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("toggle-geometric-border")
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_geom.update(cx, |s, cx| {
                                                    s.geometric_border = !s.geometric_border;
                                                    let _ = s.save_to_disk();
                                                    cx.notify();
                                                });
                                                view.status_msg = Some(
                                                    "✓ Saved ATLAS Geometric Lattice Border preference"
                                                        .into(),
                                                );
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_color(if geometric_border {
                                                        rgb(0x22d3ee)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .child(if geometric_border {
                                                        "☑"
                                                    } else {
                                                        "☐"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.fg)
                                                    .child("ATLAS Geometric Frame"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("toggle-line-numbers")
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_line_numbers.update(cx, |s, cx| {
                                                    s.show_line_numbers = !s.show_line_numbers;
                                                    let _ = s.save_to_disk();
                                                    cx.notify();
                                                });
                                                view.status_msg = Some(
                                                    "✓ Saved Line Numbers gutter preference"
                                                        .into(),
                                                );
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_color(if show_line_numbers {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .child(if show_line_numbers {
                                                        "☑"
                                                    } else {
                                                        "☐"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.fg)
                                                    .child("Line Numbers (1 │ ln)"),
                                            ),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
            SettingsSection::KeyboardShortcuts => {
                let bindings = self.settings.read(cx).custom_bindings.clone();
                let conflicts = detect_keybind_conflicts(&bindings);
                let filter_query = self.shortcuts_filter.clone();
                let filter_lower = filter_query.trim().to_ascii_lowercase();

                let matching_actions: Vec<(usize, &'static crate::actions::ActionDescriptor)> =
                    WELL_ACTIONS
                        .iter()
                        .enumerate()
                        .filter(|(idx, desc)| {
                            if filter_lower.is_empty() {
                                return true;
                            }
                            let current_key = bindings
                                .get(*idx)
                                .map(|(_, k, _)| k.as_str())
                                .unwrap_or(desc.default_keybind);
                            let haystack = format!(
                                "{} {} {} {} {}",
                                desc.id,
                                desc.label,
                                desc.category.as_str(),
                                desc.description,
                                current_key
                            )
                            .to_ascii_lowercase();
                            filter_lower
                                .split_whitespace()
                                .all(|tok| haystack.contains(tok))
                        })
                        .collect();

                let page_size = 5usize;
                let total_actions = matching_actions.len();
                let total_pages = total_actions.div_ceil(page_size).max(1);
                let current_page = self.shortcuts_page.min(total_pages - 1);
                let start_idx = current_page * page_size;

                let mut rows = div().w_full().flex().flex_col().gap_1p5().overflow_hidden();

                if matching_actions.is_empty() {
                    rows = rows.child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(theme.tab_inactive_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .text_xs()
                            .text_color(rgb(0x928374))
                            .child("No matching functions found. Press Backspace or Clear to reset filter."),
                    );
                } else {
                    for (idx, desc) in matching_actions
                        .into_iter()
                        .skip(start_idx)
                        .take(page_size)
                    {
                        let current_key = bindings
                            .get(idx)
                            .map(|(_, k, _)| k.clone())
                            .unwrap_or_else(|| desc.default_keybind.to_string());
                        let is_rebinding = self.rebinding_index == Some(idx);
                        let conflict_msg = conflicts
                            .iter()
                            .find(|(c_idx, _)| *c_idx == idx)
                            .map(|(_, m)| m.clone());

                        rows = rows.child(
                            div()
                                .id(("action-bind-row", idx))
                                .w_full()
                                .overflow_hidden()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(theme.tab_inactive_bg)
                                .border_1()
                                .border_color(if conflict_msg.is_some() {
                                    rgb(0xfb4934)
                                } else if is_rebinding {
                                    theme.border_active
                                } else {
                                    theme.border_inactive
                                })
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .gap_3()
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
                                                        .px_1p5()
                                                        .py_0p5()
                                                        .rounded_sm()
                                                        .bg(theme.tab_active_bg)
                                                        .text_size(px(10.0))
                                                        .text_color(rgb(0x8ec07c))
                                                        .child(format!(
                                                            "{} {}",
                                                            desc.category.badge_icon(),
                                                            desc.category.as_str()
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::BOLD)
                                                        .text_color(theme.fg)
                                                        .child(desc.label),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_size(px(10.0))
                                                        .text_color(rgb(0x928374))
                                                        .child(format!("[{}]", desc.key_context)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .overflow_hidden()
                                                .text_color(if conflict_msg.is_some() {
                                                    rgb(0xfb4934)
                                                } else {
                                                    rgb(0xa89984)
                                                })
                                                .child(conflict_msg.unwrap_or_else(|| {
                                                    desc.description.to_string()
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(("rebind-pill", idx))
                                        .flex_shrink_0()
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_md()
                                        .cursor(gpui::CursorStyle::PointingHand)
                                        .bg(if is_rebinding {
                                            theme.border_active
                                        } else {
                                            theme.tab_active_bg
                                        })
                                        .border_1()
                                        .border_color(theme.border_inactive)
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(if is_rebinding {
                                            theme.bg
                                        } else {
                                            theme.border_active
                                        })
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            if view.rebinding_index == Some(idx) {
                                                view.rebinding_index = None;
                                            } else {
                                                view.rebinding_index = Some(idx);
                                            }
                                            cx.notify();
                                        }))
                                        .child(if is_rebinding {
                                            "Press key...".to_string()
                                        } else {
                                            current_key
                                        }),
                                ),
                        );
                    }
                }

                div()
                    .w_full()
                    .overflow_hidden()
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
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child(format!(
                                        "⌨️ Labeled Function Catalog (Page {}/{} — Click Pill to Rebind)",
                                        current_page + 1,
                                        total_pages
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("shortcuts-prev-page")
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_xs()
                                            .text_color(theme.fg)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.shortcuts_page = view.shortcuts_page.saturating_sub(1);
                                                cx.notify();
                                            }))
                                            .child("◀ Prev"),
                                    )
                                    .child(
                                        div()
                                            .id("shortcuts-next-page")
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_xs()
                                            .text_color(theme.fg)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                if view.shortcuts_page + 1 < total_pages {
                                                    view.shortcuts_page += 1;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Next ▶"),
                                    )
                                    .child(
                                        div()
                                            .id("reset-shortcuts-btn")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.border_inactive)
                                            .text_xs()
                                            .text_color(theme.fg)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.reset_shortcuts_to_default(cx);
                                            }))
                                            .child("↺ Reset Defaults"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("shortcuts-filter-input")
                            .px_3()
                            .py_1p5()
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
                                view.active_input = ActiveFormInput::ShortcutsFilter;
                                window.focus(&view.focus_handle);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if filter_query.is_empty() {
                                        rgb(0xa89984)
                                    } else {
                                        theme.fg
                                    })
                                    .child(if filter_query.is_empty() {
                                        "🔍 Type here to filter functions by name, category, or keybind... ▏".to_string()
                                    } else {
                                        format!("🔍 {filter_query}▏")
                                    }),
                            )
                            .children(if !filter_query.is_empty() {
                                Some(
                                    div()
                                        .id("clear-shortcuts-filter")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x3c3836))
                                        .text_xs()
                                        .text_color(rgb(0xebdbb2))
                                        .cursor(gpui::CursorStyle::PointingHand)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.shortcuts_filter.clear();
                                            view.shortcuts_page = 0;
                                            cx.notify();
                                        }))
                                        .child("✕ Clear"),
                                )
                            } else {
                                None
                            }),
                    )
                    .child(rows)
                    .into_any_element()
            }
            SettingsSection::SnippetsWorkflows => {
                let snippets = self.settings.read(cx).snippets.clone();
                let trig_in = self.new_snippet_trigger.clone();
                let lbl_in = self.new_snippet_label.clone();
                let tpl_in = self.new_snippet_template.clone();
                let active_in = self.active_input;

                let mut list = div().w_full().flex().flex_col().gap_1p5();

                for (idx, snip) in snippets.iter().enumerate() {
                    let trigger = snip.trigger.clone();
                    let trigger_for_edit = snip.trigger.clone();
                    let label_for_edit = snip.label.clone();
                    let template_for_edit = snip.template.clone();
                    let trigger_for_row = snip.trigger.clone();
                    let label_for_row = snip.label.clone();
                    let template_for_row = snip.template.clone();
                    let trigger_for_run = snip.trigger.clone();
                    let template_for_copy = snip.template.clone();
                    let sm_del = settings_entity.clone();
                    let sm_run = settings_entity.clone();
                    let is_editing_this = trig_in == snip.trigger;

                    list = list.child(
                        div()
                            .id(("snippet-card-row", idx))
                            .w_full()
                            .overflow_hidden()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(if is_editing_this {
                                theme.tab_active_bg
                            } else {
                                theme.bg
                            })
                            .border_1()
                            .border_color(if is_editing_this {
                                theme.border_active
                            } else {
                                theme.border_inactive
                            })
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.new_snippet_trigger = trigger_for_row.clone();
                                view.new_snippet_label = label_for_row.clone();
                                view.new_snippet_template = template_for_row.clone();
                                view.active_input = ActiveFormInput::SnippetTemplate;
                                window.focus(&view.focus_handle);
                                view.status_msg = Some(format!(
                                    "✎ Loaded snippet ':{trigger_for_row}' into editor above — edit & click 💾 Save Snippet"
                                ));
                                cx.notify();
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
                                                    .bg(theme.tab_active_bg)
                                                    .border_1()
                                                    .border_color(theme.border_active)
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.border_active)
                                                    .child(format!(":{}", snip.trigger)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.fg)
                                                    .child(snip.label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(0x28332a))
                                                    .text_size(px(10.0))
                                                    .text_color(rgb(0x8ec07c))
                                                    .child(snip.scope.as_str()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family("Menlo")
                                            .text_xs()
                                            .overflow_hidden()
                                            .text_color(rgb(0x8ec07c))
                                            .child(snip.template.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .id(("insert-snippet-term-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x8ec07c))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x1d2021))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                let trig = trigger_for_run.clone();
                                                sm_run.update(cx, |s, cx| {
                                                    s.requested_snippet_insert = Some(trig);
                                                    s.settings_open = false;
                                                    s.modal_open = false;
                                                    s.requested_hub_switch = Some("close");
                                                    cx.notify();
                                                });
                                            }))
                                            .child("▶ Insert in Terminal"),
                                    )
                                    .child(
                                        div()
                                            .id(("edit-snippet-btn", idx))
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_active)
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.border_active)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.new_snippet_trigger = trigger_for_edit.clone();
                                                view.new_snippet_label = label_for_edit.clone();
                                                view.new_snippet_template = template_for_edit.clone();
                                                view.active_input = ActiveFormInput::SnippetTemplate;
                                                window.focus(&view.focus_handle);
                                                view.status_msg = Some(format!(
                                                    "✎ Editing snippet ':{trigger_for_edit}' — modify fields above & click 💾 Save Snippet"
                                                ));
                                                cx.notify();
                                            }))
                                            .child("✎ Edit"),
                                    )
                                    .child(
                                        div()
                                            .id(("copy-snippet-btn", idx))
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.tab_active_bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .text_xs()
                                            .text_color(theme.fg)
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                cx.write_to_clipboard(
                                                    gpui::ClipboardItem::new_string(
                                                        template_for_copy.clone(),
                                                    ),
                                                );
                                                view.status_msg = Some(
                                                    "📋 Copied snippet template to clipboard".into(),
                                                );
                                                cx.notify();
                                            }))
                                            .child("📋"),
                                    )
                                    .child(
                                        div()
                                            .id(("del-snippet", idx))
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgb(0x3c1f1e))
                                            .text_xs()
                                            .text_color(rgb(0xfb4934))
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                let trig = trigger.clone();
                                                sm_del.update(cx, |s, cx| {
                                                    s.snippets.retain(|item| item.trigger != trig);
                                                    let _ = s.save_to_disk();
                                                    cx.notify();
                                                });
                                                view.status_msg = Some(format!(
                                                    "✓ Removed snippet ':{trigger}' & saved"
                                                ));
                                                cx.notify();
                                            }))
                                            .child("✕"),
                                    ),
                            ),
                    );
                }

                let sm_add = settings_entity.clone();
                let is_trig_sel = active_in == ActiveFormInput::SnippetTrigger;
                let is_lbl_sel = active_in == ActiveFormInput::SnippetLabel;
                let is_tpl_sel = active_in == ActiveFormInput::SnippetTemplate;

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        // Custom Snippet Creator & Saver Box
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_active)
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
                                            .text_color(theme.border_active)
                                            .child("➕ Create or Edit Parameterized Snippet (Click any field to type · Tab to switch field)"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("clear-snippet-draft-btn")
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(theme.bg)
                                                    .border_1()
                                                    .border_color(theme.border_inactive)
                                                    .text_xs()
                                                    .text_color(theme.fg)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, window, cx| {
                                                        view.new_snippet_trigger.clear();
                                                        view.new_snippet_label.clear();
                                                        view.new_snippet_template.clear();
                                                        view.active_input =
                                                            ActiveFormInput::SnippetTrigger;
                                                        window.focus(&view.focus_handle);
                                                        view.status_msg = Some(
                                                            "✨ Cleared fields — type a new Trigger, Label & Template"
                                                                .into(),
                                                        );
                                                        cx.notify();
                                                    }))
                                                    .child("🧹 Clear / New"),
                                            )
                                            .child(
                                                div()
                                                    .id("save-new-snippet-btn")
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x8ec07c))
                                                    .text_color(rgb(0x1d2021))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        let trig = view
                                                            .new_snippet_trigger
                                                            .trim()
                                                            .trim_start_matches(':')
                                                            .to_string();
                                                        let lbl = view
                                                            .new_snippet_label
                                                            .trim()
                                                            .to_string();
                                                        let tpl = view
                                                            .new_snippet_template
                                                            .trim()
                                                            .to_string();
                                                        if !trig.is_empty() && !tpl.is_empty() {
                                                            sm_add.update(cx, |s, cx| {
                                                                s.snippets
                                                                    .retain(|x| x.trigger != trig);
                                                                s.snippets.insert(
                                                                    0,
                                                                    WellSnippet {
                                                                        trigger: trig.clone(),
                                                                        label: if lbl.is_empty() {
                                                                            trig.clone()
                                                                        } else {
                                                                            lbl
                                                                        },
                                                                        template: tpl,
                                                                        scope: SnippetScope::Global,
                                                                    },
                                                                );
                                                                let _ = s.save_to_disk();
                                                                cx.notify();
                                                            });
                                                            view.status_msg = Some(format!(
                                                                "✓ Saved snippet ':{trig}' to operator_settings.json"
                                                            ));
                                                            cx.notify();
                                                        } else {
                                                            view.status_msg = Some(
                                                                "⚠ Trigger and Template cannot be empty"
                                                                    .into(),
                                                            );
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child("💾 Save Snippet (↵)"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("snip-trig-field")
                                            .w(px(170.0))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.bg)
                                            .border_1()
                                            .border_color(if is_trig_sel {
                                                theme.border_active
                                            } else {
                                                theme.border_inactive
                                            })
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.border_active)
                                            .cursor(gpui::CursorStyle::IBeam)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.active_input = ActiveFormInput::SnippetTrigger;
                                                window.focus(&view.focus_handle);
                                                cx.notify();
                                            }))
                                            .child(if is_trig_sel {
                                                format!("Trigger: :{trig_in}▏")
                                            } else if trig_in.is_empty() {
                                                "Trigger: (e.g. klogs)".to_string()
                                            } else {
                                                format!("Trigger: :{trig_in}")
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("snip-lbl-field")
                                            .flex_grow()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.bg)
                                            .border_1()
                                            .border_color(if is_lbl_sel {
                                                theme.border_active
                                            } else {
                                                theme.border_inactive
                                            })
                                            .text_xs()
                                            .text_color(theme.fg)
                                            .cursor(gpui::CursorStyle::IBeam)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.active_input = ActiveFormInput::SnippetLabel;
                                                window.focus(&view.focus_handle);
                                                cx.notify();
                                            }))
                                            .child(if is_lbl_sel {
                                                format!("Label: {lbl_in}▏")
                                            } else if lbl_in.is_empty() {
                                                "Label: (description)".to_string()
                                            } else {
                                                format!("Label: {lbl_in}")
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("snip-tpl-field")
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(theme.bg)
                                    .border_1()
                                    .border_color(if is_tpl_sel {
                                        theme.border_active
                                    } else {
                                        theme.border_inactive
                                    })
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(rgb(0x8ec07c))
                                    .cursor(gpui::CursorStyle::IBeam)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.active_input = ActiveFormInput::SnippetTemplate;
                                        window.focus(&view.focus_handle);
                                        cx.notify();
                                    }))
                                    .child(if is_tpl_sel {
                                        format!("Template: {tpl_in}▏")
                                    } else if tpl_in.is_empty() {
                                        "Template: (e.g. kubectl logs -f ${1:pod} -n ${2:default})"
                                            .to_string()
                                    } else {
                                        format!("Template: {tpl_in}")
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
                                    .child(format!(
                                        "📚 Saved Parameterized Snippets ({} total — Click any card to edit or ▶ Insert in Terminal)",
                                        snippets.len()
                                    )),
                            ),
                    )
                    .child(list)
                    .into_any_element()
            }
            SettingsSection::TheiaSshHosts => {
                let profiles = self.settings.read(cx).ssh_profiles.clone();
                let draft = self.ssh_draft.clone();
                let active_in = self.active_input;
                let mut cards = div().flex().flex_col().gap_1p5();

                for (idx, prof) in profiles.iter().take(4).enumerate() {
                    let (border_hex, _, badge_hex) = prof.tier.prism_palette();
                    let sm_ipc = settings_entity.clone();
                    let sm_cm = settings_entity.clone();
                    let sm_del = settings_entity.clone();
                    let prof_for_edit = prof.clone();
                    let host_name_del = prof.host.name.clone();
                    let ipc_on = prof.forward_caduceus_ipc;
                    let cm_on = prof.use_control_master;
                    let redacted_key = prof
                        .redacted_identity_display()
                        .unwrap_or_else(|| "Default Agent Key".into());

                    cards = cards.child(
                        div()
                            .p_2p5()
                            .rounded_lg()
                            .bg(theme.bg)
                            .border_1()
                            .border_color(if draft.editing_index == Some(idx) {
                                theme.border_active
                            } else {
                                rgb(border_hex)
                            })
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
                                            .gap_2()
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(badge_hex))
                                                    .text_size(px(10.0))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x1d2021))
                                                    .child(prof.tier.badge_label()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.fg)
                                                    .child(prof.host.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0xa89984))
                                                    .child(format!(
                                                        "{}@{}:{}",
                                                        prof.host.user.as_deref().unwrap_or("root"),
                                                        prof.host.hostname,
                                                        prof.host.port
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .text_color(rgb(0x83a598))
                                                    .child(format!("🔑 {redacted_key}")),
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
                                                    .id(("edit-ssh-host-btn", idx))
                                                    .px_2p5()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme.tab_active_bg)
                                                    .border_1()
                                                    .border_color(theme.border_active)
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.border_active)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, window, cx| {
                                                        view.ssh_draft = SshHostEditorDraft::from_profile(
                                                            Some(idx),
                                                            &prof_for_edit,
                                                        );
                                                        view.active_input = ActiveFormInput::SshHostname;
                                                        window.focus(&view.focus_handle);
                                                        view.status_msg = Some(format!(
                                                            "✎ Editing SSH host '{}' — modify fields above & click Save Host",
                                                            prof_for_edit.host.name
                                                        ));
                                                        cx.notify();
                                                    }))
                                                    .child("✎ Edit"),
                                            )
                                            .child(
                                                div()
                                                    .id(("delete-ssh-host-btn", idx))
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x3c1f1e))
                                                    .text_xs()
                                                    .text_color(rgb(0xfb4934))
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        sm_del.update(cx, |s, cx| {
                                                            if idx < s.ssh_profiles.len() {
                                                                s.ssh_profiles.remove(idx);
                                                                let _ = s.save_to_disk();
                                                                cx.notify();
                                                            }
                                                        });
                                                        if view.ssh_draft.editing_index == Some(idx) {
                                                            view.ssh_draft.editing_index = None;
                                                        }
                                                        view.status_msg = Some(format!(
                                                            "✓ Deleted SSH host '{host_name_del}' & saved"
                                                        ));
                                                        cx.notify();
                                                    }))
                                                    .child("🗑"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_4()
                                    .items_center()
                                    .child(
                                        div()
                                            .id(("ssh-ipc-toggle", idx))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_ipc.update(cx, |s, cx| {
                                                    if let Some(p) = s.ssh_profiles.get_mut(idx) {
                                                        p.forward_caduceus_ipc = !p.forward_caduceus_ipc;
                                                        let _ = s.save_to_disk();
                                                        cx.notify();
                                                    }
                                                });
                                                view.status_msg = Some("✓ Saved SSH Caduceus IPC tunnel setting".into());
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_color(if ipc_on {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .child(if ipc_on { "☑" } else { "☐" }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.fg)
                                                    .child("Forward /tmp/well-caduceus.sock (-R IPC Tunnel)"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id(("ssh-cm-toggle", idx))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .cursor(gpui::CursorStyle::PointingHand)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                sm_cm.update(cx, |s, cx| {
                                                    if let Some(p) = s.ssh_profiles.get_mut(idx) {
                                                        p.use_control_master = !p.use_control_master;
                                                        let _ = s.save_to_disk();
                                                        cx.notify();
                                                    }
                                                });
                                                view.status_msg = Some("✓ Saved SSH ControlMaster setting".into());
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_color(if cm_on {
                                                        rgb(0x8ec07c)
                                                    } else {
                                                        rgb(0x928374)
                                                    })
                                                    .child(if cm_on { "☑" } else { "☐" }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.fg)
                                                    .child("ControlMaster Multiplexing"),
                                            ),
                                    ),
                            ),
                    );
                }

                let sm_save_ssh = settings_entity.clone();
                let sm_launch_ssh = settings_entity.clone();
                let build_ssh_field = |id_str: &'static str,
                                       label: &'static str,
                                       val: String,
                                       field: ActiveFormInput,
                                       cx: &mut Context<Self>| {
                    let is_sel = active_in == field;
                    div()
                        .id(id_str)
                        .flex_1()
                        .min_w_0()
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(theme.bg)
                        .border_1()
                        .border_color(if is_sel {
                            theme.border_active
                        } else {
                            theme.border_inactive
                        })
                        .cursor(gpui::CursorStyle::IBeam)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.active_input = field;
                            window.focus(&view.focus_handle);
                            cx.notify();
                        }))
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0x928374))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(if is_sel {
                                    theme.border_active
                                } else {
                                    theme.fg
                                })
                                .child(if is_sel {
                                    format!("{val}▏")
                                } else if val.is_empty() {
                                    "(none)".to_string()
                                } else {
                                    val
                                }),
                        )
                };

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("open-full-theia-ssh-console-btn")
                            .px_3()
                            .py_1p5()
                            .rounded_lg()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_active)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .cursor(gpui::CursorStyle::PointingHand)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                sm_launch_ssh.update(cx, |s, cx| {
                                    s.settings_open = false;
                                    s.requested_hub_switch = Some("ssh");
                                    cx.notify();
                                });
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child("🌐 Open Full 7-Module Theia SSH, Tunnels, Key Vault, PXE & Net Ops Console (⇧⌘S)"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(theme.border_active)
                                    .text_color(theme.bg)
                                    .text_size(px(10.5))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("Launch Console ➔"),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
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
                                            .text_color(theme.border_active)
                                            .child(if let Some(edit_i) = draft.editing_index {
                                                format!(
                                                    "✎ Editing SSH Host #{} ({}) — Click Any Field to Modify",
                                                    edit_i + 1,
                                                    draft.name
                                                )
                                            } else {
                                                "➕ Create / Edit SSH Host Profile (Click Any Field to Type)".to_string()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("new-ssh-draft-btn")
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(theme.border_inactive)
                                                    .text_xs()
                                                    .text_color(theme.fg)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(|view, _, window, cx| {
                                                        view.ssh_draft = SshHostEditorDraft::new_blank();
                                                        view.active_input = ActiveFormInput::SshName;
                                                        window.focus(&view.focus_handle);
                                                        cx.notify();
                                                    }))
                                                    .child("+ New Host"),
                                            )
                                            .child(
                                                div()
                                                    .id("save-new-ssh-host-btn")
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x8ec07c))
                                                    .text_color(rgb(0x1d2021))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .cursor(gpui::CursorStyle::PointingHand)
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        match view.ssh_draft.to_profile() {
                                                            Ok(new_prof) => {
                                                                let saved_name = new_prof.host.name.clone();
                                                                let edit_idx = view.ssh_draft.editing_index;
                                                                sm_save_ssh.update(cx, |s, cx| {
                                                                    if let Some(idx) = edit_idx {
                                                                        if idx < s.ssh_profiles.len() {
                                                                            s.ssh_profiles[idx] = new_prof;
                                                                        } else {
                                                                            s.ssh_profiles.insert(0, new_prof);
                                                                        }
                                                                    } else if let Some(existing) = s
                                                                        .ssh_profiles
                                                                        .iter_mut()
                                                                        .find(|p| p.host.name == saved_name)
                                                                    {
                                                                        *existing = new_prof;
                                                                    } else {
                                                                        s.ssh_profiles.insert(0, new_prof);
                                                                    }
                                                                    let _ = s.save_to_disk();
                                                                    cx.notify();
                                                                });
                                                                view.status_msg = Some(format!(
                                                                    "✓ Saved SSH host profile '{saved_name}' to operator_settings.json"
                                                                ));
                                                                cx.notify();
                                                            }
                                                            Err(err) => {
                                                                view.status_msg = Some(format!("⚠ {err}"));
                                                                cx.notify();
                                                            }
                                                        }
                                                    }))
                                                    .child(if draft.editing_index.is_some() {
                                                        "💾 Save Host Changes"
                                                    } else {
                                                        "💾 Save Host Profile"
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(build_ssh_field(
                                        "ssh-field-name",
                                        "Host Alias / Name",
                                        draft.name.clone(),
                                        ActiveFormInput::SshName,
                                        cx,
                                    ))
                                    .child(build_ssh_field(
                                        "ssh-field-hostname",
                                        "Hostname / IP Address",
                                        draft.hostname.clone(),
                                        ActiveFormInput::SshHostname,
                                        cx,
                                    ))
                                    .child(build_ssh_field(
                                        "ssh-field-user",
                                        "SSH Username",
                                        draft.user.clone(),
                                        ActiveFormInput::SshUser,
                                        cx,
                                    ))
                                    .child(build_ssh_field(
                                        "ssh-field-port",
                                        "Port",
                                        draft.port.clone(),
                                        ActiveFormInput::SshPort,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(build_ssh_field(
                                        "ssh-field-identity",
                                        "Private Key Path (IdentityFile)",
                                        draft.identity_file.clone(),
                                        ActiveFormInput::SshIdentityFile,
                                        cx,
                                    ))
                                    .child(build_ssh_field(
                                        "ssh-field-jump",
                                        "ProxyJump / Bastion Host",
                                        draft.proxy_jump.clone(),
                                        ActiveFormInput::SshProxyJump,
                                        cx,
                                    )),
                            ),
                    )
                    .child(cards)
                    .into_any_element()
            }
            SettingsSection::MnemeSafetyGates => {
                let s = self.settings.read(cx);
                let gate_rm = s.gate_rm_root;
                let gate_k8s = s.gate_kubectl_delete;
                let gate_git = s.gate_git_force_push;
                let gate_tf = s.gate_terraform_destroy;

                let sm1 = settings_entity.clone();
                let sm2 = settings_entity.clone();
                let sm3 = settings_entity.clone();
                let sm4 = settings_entity.clone();

                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .rounded_lg()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child("🛡️ Mneme Tree-Sitter AST Destructive Command Gates"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xa89984))
                                    .child("When enabled, matching commands block standard Enter execution and require Shift+Enter (⇧⏎) override."),
                            )
                            .child(
                                div()
                                    .id("gate-rm")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        sm1.update(cx, |s, cx| {
                                            s.gate_rm_root = !s.gate_rm_root;
                                            let _ = s.save_to_disk();
                                            cx.notify();
                                        });
                                        view.status_msg = Some("✓ Saved Mneme safety gate to disk".into());
                                        cx.notify();
                                    }))
                                    .child(div().text_color(rgb(0x8ec07c)).child(if gate_rm { "☑" } else { "☐" }))
                                    .child(div().text_xs().text_color(theme.fg).child("Block recursive root/system removal (rm -rf /, /System, /etc, mkfs, dd of=/dev/*)")),
                            )
                            .child(
                                div()
                                    .id("gate-k8s")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        sm2.update(cx, |s, cx| {
                                            s.gate_kubectl_delete = !s.gate_kubectl_delete;
                                            let _ = s.save_to_disk();
                                            cx.notify();
                                        });
                                        view.status_msg = Some("✓ Saved Mneme safety gate to disk".into());
                                        cx.notify();
                                    }))
                                    .child(div().text_color(rgb(0x8ec07c)).child(if gate_k8s { "☑" } else { "☐" }))
                                    .child(div().text_xs().text_color(theme.fg).child("Block Kubernetes bulk/critical deletions (kubectl delete --all, namespace, node)")),
                            )
                            .child(
                                div()
                                    .id("gate-git")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        sm3.update(cx, |s, cx| {
                                            s.gate_git_force_push = !s.gate_git_force_push;
                                            let _ = s.save_to_disk();
                                            cx.notify();
                                        });
                                        view.status_msg = Some("✓ Saved Mneme safety gate to disk".into());
                                        cx.notify();
                                    }))
                                    .child(div().text_color(rgb(0x8ec07c)).child(if gate_git { "☑" } else { "☐" }))
                                    .child(div().text_xs().text_color(theme.fg).child("Block Git history destruction (git push --force main/master/prod, git reset --hard)")),
                            )
                            .child(
                                div()
                                    .id("gate-tf")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        sm4.update(cx, |s, cx| {
                                            s.gate_terraform_destroy = !s.gate_terraform_destroy;
                                            let _ = s.save_to_disk();
                                            cx.notify();
                                        });
                                        view.status_msg = Some("✓ Saved Mneme safety gate to disk".into());
                                        cx.notify();
                                    }))
                                    .child(div().text_color(rgb(0x8ec07c)).child(if gate_tf { "☑" } else { "☐" }))
                                    .child(div().text_xs().text_color(theme.fg).child("Block unattended Terraform infrastructure destruction (terraform destroy --auto-approve)")),
                            ),
                    )
                    .into_any_element()
            }
            SettingsSection::PythiaTelemetry => {
                let metis_priority = self.settings.read(cx).metis_offline_cache_priority;
                let sm_metis = settings_entity.clone();

                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .rounded_lg()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child("✨ Pythia AI, Metis Cache & Caduceus IPC Telemetry"),
                            )
                            .child(
                                div()
                                    .id("toggle-metis-priority")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        sm_metis.update(cx, |s, cx| {
                                            s.metis_offline_cache_priority = !s.metis_offline_cache_priority;
                                            let _ = s.save_to_disk();
                                            cx.notify();
                                        });
                                        view.status_msg = Some("✓ Saved Pythia/Metis priority to disk".into());
                                        cx.notify();
                                    }))
                                    .child(div().text_color(rgb(0x8ec07c)).child(if metis_priority { "☑" } else { "☐" }))
                                    .child(div().text_xs().text_color(theme.fg).child("Prioritize <2ms offline Metis SQLite/Trie recipe cache before remote LLM calls")),
                            )
                            .child(
                                div()
                                    .p_3()
                                    .rounded_md()
                                    .bg(theme.bg)
                                    .border_1()
                                    .border_color(theme.border_inactive)
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(0x8ec07c)).child("🟢 Caduceus IPC Endpoint: /tmp/well-caduceus.sock"))
                                    .child(div().text_xs().text_color(rgb(0xa89984)).child("Dual-Transport Multiplexer: 4-byte LE Framed Binary (4 MiB cap) + HTTP/WS JSON-RPC"))
                                    .child(div().text_xs().text_color(rgb(0xa89984)).child("BigQuery AI/ML Sync: completions_native (Partitioned + Vertex AI text-embedding-005)")),
                            ),
                    )
                    .into_any_element()
            }
            SettingsSection::SystemInfoGuide => {
                let make_lore_card = |icon: &'static str,
                                      name: &'static str,
                                      key_badge: &'static str,
                                      badge_color: u32,
                                      myth_origin: &'static str,
                                      desc_line1: &'static str,
                                      desc_line2: &'static str| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .p_2p5()
                        .rounded_lg()
                        .bg(theme.bg)
                        .border_1()
                        .border_color(theme.border_inactive)
                        .flex()
                        .flex_col()
                        .gap_1()
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
                                        .child(div().text_xs().child(icon))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.border_active)
                                                .child(name),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(badge_color))
                                        .text_size(px(10.0))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0x090d16))
                                        .child(key_badge),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(0x8ec07c))
                                .child(myth_origin),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.fg)
                                .child(desc_line1),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(rgb(0xd5c4a1))
                                .child(desc_line2),
                        )
                };

                div()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_active)
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
                                            .text_color(theme.border_active)
                                            .child("📖 WELL TERMINAL — MYTHOLOGICAL SUBSYSTEM NAMES & ARCHITECTURE"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(rgb(0xa89984))
                                            .child("Why every subsystem is named & what it powers"),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(make_lore_card(
                                        "🏛️",
                                        "THEIA & THEIA'S PRISM",
                                        "⌘, / ⇧⌘S",
                                        0x22d3ee,
                                        "Titaness of Sight, Light & Shining Ether",
                                        "Visual Theme Studio, 24-bit color refraction,",
                                        "per-host SSH tier auto-tint & 7-module NetOps.",
                                    ))
                                    .child(make_lore_card(
                                        "🌐",
                                        "ATLAS GRID & LATTICE",
                                        "Top ◈",
                                        0xa855f7,
                                        "Titan Bearing the Celestial Sphere",
                                        "Zero-copy libghostty VT state machine, GPU",
                                        "dirty-row damage, Kitty Graphics & Lattice frame.",
                                    )),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(make_lore_card(
                                        "✨",
                                        "PYTHIA AI ORACLE",
                                        "⌘K",
                                        0xfabd2f,
                                        "High Priestess & Oracle of Delphi",
                                        "Inline natural-language-to-shell AI synthesizer.",
                                        "Translates plain English into safe shell commands.",
                                    ))
                                    .child(make_lore_card(
                                        "🛡️",
                                        "MNEME SNIPPETS & GATES",
                                        "^R",
                                        0xfb4934,
                                        "Muse of Memory & Preservation",
                                        "Parameterized ${1:arg} workflow templates, multi-line",
                                        "command editor & Tree-sitter AST safety gates.",
                                    )),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(make_lore_card(
                                        "⚡",
                                        "METIS HISTORY ENGINE",
                                        "Sub-2ms",
                                        0x8ec07c,
                                        "Goddess of Wise Counsel & Cunning",
                                        "Radix-trie command history (~/.local/share/well/)",
                                        "& offline team recipe cache with sub-2ms lookup.",
                                    ))
                                    .child(make_lore_card(
                                        "🧭",
                                        "ASTRAEA & CADUCEUS IPC",
                                        "IPC / Git",
                                        0x38bdf8,
                                        "Goddess of Precision + Hermes' Staff",
                                        "Tracks live CWD (OSC 7), Git branch & dirty state.",
                                        "Caduceus exposes JSON-RPC 2.0 over SSH -R sockets.",
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(theme.tab_active_bg)
                            .border_1()
                            .border_color(theme.border_inactive)
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.border_active)
                                    .child("⚡ COMPLETE FUNCTIONS, BARS & THEIA SSH 7-MODULE REFERENCE"),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .p_2p5()
                                            .rounded_md()
                                            .bg(theme.bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x22d3ee))
                                                    .child("Window Bars & Quick Controls"),
                                            )
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• Top Center [ −  ∏  □  ◈ ]: Split Down/Right, Equalize & Lattice"))
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• Bottom Left [ >< Theia SSH ]: 7-module SSH Hub + Git branch"))
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• Bottom Center: Clickable ⇧⌘P Functions, ⌘K Pythia, ^R Mneme"))
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• Bottom Right [ 🏛️ Theia's Prism (⌘,) ]: Unified Control Center")),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .p_2p5()
                                            .rounded_md()
                                            .bg(theme.bg)
                                            .border_1()
                                            .border_color(theme.border_inactive)
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0x8ec07c))
                                                    .child("Theia SSH Left Icon Rail (7 Modules)"),
                                            )
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• [ >_ ] 1. SSH Hosts: Saved profiles & ~/.ssh/config launcher"))
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• [ ⇄ ] 2. Port Tunnels: Visual -L Local, -R Remote & -D SOCKS5"))
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• [ 🔑 / 🛡️ ] 3–4. Key Vault & SSH Server: Ed25519 & HTTP keys"))
                                            .child(div().text_size(px(10.5)).text_color(theme.fg).child("• [ 🚀 / 📁 / 📡 ] 5–7. NetOps: PXE Boot, SMB Shares & Ping/WoL")),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
        };

        let sm_switch_ssh = settings_entity.clone();
        let sm_close_bottom = settings_entity.clone();
        let is_info_tab = active_section == SettingsSection::SystemInfoGuide;

        div()
            .id("theia-prism-settings-window")
            .key_context("SettingsModal")
            .size_full()
            .bg(theme.bg)
            .text_color(theme.fg)
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _window, cx| {
                // ⌘S or Ctrl+S saves all changes immediately
                if (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
                    && event.keystroke.key == "s"
                {
                    view.save_all(cx);
                    return;
                }

                if let Some(idx) = view.rebinding_index {
                    let key = event.keystroke.key.as_str();
                    if key == "escape" {
                        view.rebinding_index = None;
                        cx.notify();
                        return;
                    }
                    let mut parts = Vec::new();
                    if event.keystroke.modifiers.control {
                        parts.push("⌃");
                    }
                    if event.keystroke.modifiers.alt {
                        parts.push("⌥");
                    }
                    if event.keystroke.modifiers.shift {
                        parts.push("⇧");
                    }
                    if event.keystroke.modifiers.platform {
                        parts.push("⌘");
                    }
                    parts.push(&event.keystroke.key);
                    let formatted = parts.join("").to_uppercase();
                    view.settings.update(cx, |s, cx| {
                        if let Some(slot) = s.custom_bindings.get_mut(idx) {
                            slot.1 = formatted.clone();
                        }
                        let _ = s.save_to_disk();
                        cx.notify();
                    });
                    view.status_msg = Some(format!("✓ Bound & saved shortcut '{formatted}'"));
                    view.rebinding_index = None;
                    cx.notify();
                    return;
                }

                let target_input = if view.active_input != ActiveFormInput::None {
                    view.active_input
                } else if view.active_section == SettingsSection::KeyboardShortcuts
                    && !view.shortcuts_filter.is_empty()
                {
                    ActiveFormInput::ShortcutsFilter
                } else {
                    ActiveFormInput::None
                };

                if target_input == ActiveFormInput::None && event.keystroke.key == "escape" {
                    view.save_all(cx);
                    view.settings.update(cx, |s, cx| {
                        s.settings_open = false;
                        s.modal_open = false;
                        cx.notify();
                    });
                    return;
                }

                let target_input = if target_input != ActiveFormInput::None {
                    target_input
                } else if view.active_section == SettingsSection::KeyboardShortcuts {
                    ActiveFormInput::ShortcutsFilter
                } else if view.active_section == SettingsSection::SnippetsWorkflows {
                    ActiveFormInput::SnippetTemplate
                } else {
                    ActiveFormInput::None
                };

                if target_input != ActiveFormInput::None {
                    let key = event.keystroke.key.as_str();
                    match key {
                        "tab" => {
                            view.active_input = match target_input {
                                ActiveFormInput::SnippetTrigger => ActiveFormInput::SnippetLabel,
                                ActiveFormInput::SnippetLabel => ActiveFormInput::SnippetTemplate,
                                ActiveFormInput::SnippetTemplate => ActiveFormInput::SnippetTrigger,
                                ActiveFormInput::SshName => ActiveFormInput::SshHostname,
                                ActiveFormInput::SshHostname => ActiveFormInput::SshUser,
                                ActiveFormInput::SshUser => ActiveFormInput::SshPort,
                                ActiveFormInput::SshPort => ActiveFormInput::SshIdentityFile,
                                ActiveFormInput::SshIdentityFile => ActiveFormInput::SshProxyJump,
                                ActiveFormInput::SshProxyJump => ActiveFormInput::SshName,
                                other => other,
                            };
                            cx.notify();
                        }
                        "escape" => {
                            if target_input == ActiveFormInput::ShortcutsFilter {
                                view.shortcuts_filter.clear();
                                view.shortcuts_page = 0;
                            }
                            view.active_input = ActiveFormInput::None;
                            cx.notify();
                        }
                        "backspace" => {
                            match target_input {
                                ActiveFormInput::ThemeHex(slot) => {
                                    view.theme_hex_inputs[slot].pop();
                                    view.apply_custom_theme_if_valid(slot, cx);
                                }
                                ActiveFormInput::ShortcutsFilter => {
                                    view.shortcuts_filter.pop();
                                    view.shortcuts_page = 0;
                                }
                                ActiveFormInput::SnippetTrigger => {
                                    view.new_snippet_trigger.pop();
                                }
                                ActiveFormInput::SnippetLabel => {
                                    view.new_snippet_label.pop();
                                }
                                ActiveFormInput::SnippetTemplate => {
                                    view.new_snippet_template.pop();
                                }
                                ActiveFormInput::SshName => {
                                    view.ssh_draft.name.pop();
                                }
                                ActiveFormInput::SshHostname => {
                                    view.ssh_draft.hostname.pop();
                                }
                                ActiveFormInput::SshUser => {
                                    view.ssh_draft.user.pop();
                                }
                                ActiveFormInput::SshPort => {
                                    view.ssh_draft.port.pop();
                                }
                                ActiveFormInput::SshIdentityFile => {
                                    view.ssh_draft.identity_file.pop();
                                }
                                ActiveFormInput::SshProxyJump => {
                                    view.ssh_draft.proxy_jump.pop();
                                }
                                ActiveFormInput::None => {}
                            }
                            cx.notify();
                        }
                        "enter" => {
                            if let ActiveFormInput::ThemeHex(slot) = target_input {
                                view.apply_custom_theme_if_valid(slot, cx);
                                view.active_input = ActiveFormInput::None;
                                cx.notify();
                            } else if matches!(
                                target_input,
                                ActiveFormInput::SnippetTrigger
                                    | ActiveFormInput::SnippetLabel
                                    | ActiveFormInput::SnippetTemplate
                            ) {
                                let trig = view
                                    .new_snippet_trigger
                                    .trim()
                                    .trim_start_matches(':')
                                    .to_string();
                                let lbl = view.new_snippet_label.trim().to_string();
                                let tpl = view.new_snippet_template.trim().to_string();
                                if !trig.is_empty() && !tpl.is_empty() {
                                    view.settings.update(cx, |s, cx| {
                                        s.snippets.retain(|x| x.trigger != trig);
                                        s.snippets.insert(
                                            0,
                                            WellSnippet {
                                                trigger: trig.clone(),
                                                label: if lbl.is_empty() {
                                                    trig.clone()
                                                } else {
                                                    lbl
                                                },
                                                template: tpl,
                                                scope: SnippetScope::Global,
                                            },
                                        );
                                        let _ = s.save_to_disk();
                                        cx.notify();
                                    });
                                    view.status_msg = Some(format!(
                                        "✓ Saved snippet ':{trig}' to operator_settings.json"
                                    ));
                                    cx.notify();
                                }
                            }
                        }
                        _ => {
                            if let Some(ch) = extract_printable_char(&event.keystroke) {
                                match target_input {
                                    ActiveFormInput::ThemeHex(slot) => {
                                        for c in ch.chars() {
                                            if (c.is_ascii_hexdigit() || c == '#')
                                                && view.theme_hex_inputs[slot].len() < 7
                                            {
                                                view.theme_hex_inputs[slot]
                                                    .push(c.to_ascii_uppercase());
                                            }
                                        }
                                        view.apply_custom_theme_if_valid(slot, cx);
                                    }
                                    ActiveFormInput::ShortcutsFilter => {
                                        view.shortcuts_filter.push_str(&ch);
                                        view.shortcuts_page = 0;
                                    }
                                    ActiveFormInput::SnippetTrigger => {
                                        view.new_snippet_trigger.push_str(&ch);
                                    }
                                    ActiveFormInput::SnippetLabel => {
                                        view.new_snippet_label.push_str(&ch);
                                    }
                                    ActiveFormInput::SnippetTemplate => {
                                        view.new_snippet_template.push_str(&ch);
                                    }
                                    ActiveFormInput::SshName => {
                                        view.ssh_draft.name.push_str(&ch);
                                    }
                                    ActiveFormInput::SshHostname => {
                                        view.ssh_draft.hostname.push_str(&ch);
                                    }
                                    ActiveFormInput::SshUser => {
                                        view.ssh_draft.user.push_str(&ch);
                                    }
                                    ActiveFormInput::SshPort => {
                                        for c in ch.chars() {
                                            if c.is_ascii_digit() && view.ssh_draft.port.len() < 5 {
                                                view.ssh_draft.port.push(c);
                                            }
                                        }
                                    }
                                    ActiveFormInput::SshIdentityFile => {
                                        view.ssh_draft.identity_file.push_str(&ch);
                                    }
                                    ActiveFormInput::SshProxyJump => {
                                        view.ssh_draft.proxy_jump.push_str(&ch);
                                    }
                                    ActiveFormInput::None => {}
                                }
                                cx.notify();
                            }
                        }
                    }
                }
            }))
            .child(
                // Top Title Banner with Unified Theia's Prism 3-Mode Switcher
                div()
                    .px_5()
                    .py_2p5()
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
                            .child(div().text_xl().child("🏛️"))
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
                                            .text_size(px(11.0))
                                            .text_color(rgb(0x928374))
                                            .child(active_section.title()),
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
                                    .id("hub-switch-prism-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if !is_info_tab {
                                        theme.border_active
                                    } else {
                                        theme.tab_active_bg
                                    })
                                    .border_1()
                                    .border_color(if !is_info_tab {
                                        theme.border_active
                                    } else {
                                        theme.border_inactive
                                    })
                                    .text_color(if !is_info_tab {
                                        theme.bg
                                    } else {
                                        theme.fg
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        if view.active_section == SettingsSection::SystemInfoGuide {
                                            view.active_section = SettingsSection::AppearancePrism;
                                            cx.notify();
                                        }
                                    }))
                                    .child("🎨 Theia's Prism (⌘,)"),
                            )
                            .child(
                                div()
                                    .id("hub-switch-ssh-btn")
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
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        sm_switch_ssh.update(cx, |s, cx| {
                                            s.settings_open = false;
                                            s.requested_hub_switch = Some("ssh");
                                            cx.notify();
                                        });
                                    }))
                                    .child("🌐 Theia SSH & Net Ops (⇧⌘S)"),
                            )
                            .child(
                                div()
                                    .id("hub-switch-info-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if is_info_tab {
                                        theme.border_active
                                    } else {
                                        theme.tab_active_bg
                                    })
                                    .border_1()
                                    .border_color(if is_info_tab {
                                        theme.border_active
                                    } else {
                                        theme.border_inactive
                                    })
                                    .text_color(if is_info_tab {
                                        theme.bg
                                    } else {
                                        theme.fg
                                    })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.active_section = SettingsSection::SystemInfoGuide;
                                        view.rebinding_index = None;
                                        view.active_input = ActiveFormInput::None;
                                        cx.notify();
                                    }))
                                    .child("📖 Info & System Guide"),
                            ),
                    ),
            )
            .child(
                // Split Two-Pane Body
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(
                        div()
                            .id("settings-right-scroll-pane")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .h_full()
                            .overflow_y_scroll()
                            .p_3()
                            .child(right_pane),
                    ),
            )
            .child(
                // Sticky Bottom Persistence & Single Save & Close Bar
                div()
                    .px_4()
                    .py_2()
                    .bg(theme.tab_inactive_bg)
                    .border_t_1()
                    .border_color(theme.border_inactive)
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x28332a))
                            .border_1()
                            .border_color(rgb(0x8ec07c))
                            .text_xs()
                            .text_color(rgb(0x8ec07c))
                            .child(status_msg),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("bottom-save-btn")
                                    .px_4()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(0x8ec07c))
                                    .text_color(rgb(0x1d2021))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.save_all(cx);
                                        sm_close_bottom.update(cx, |s, cx| {
                                            s.settings_open = false;
                                            s.modal_open = false;
                                            s.requested_hub_switch = Some("close");
                                            cx.notify();
                                        });
                                    }))
                                    .child("✓ Save & Close (⌘S)"),
                            ),
                    ),
            )
    }
}
