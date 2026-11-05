//! High-performance prompt compiler reading directly from memory-mapped state vectors.
//! Guarantees prompt compile times strictly below 100 microseconds with zero filesystem queries.

use crate::state_vector::{AstraeaPreset, AstraeaSnapshot, AstraeaStateVector, ScmOperation};
use serde::{Deserialize, Serialize};

/// Comprehensive prompt configuration controlling presets and displayed modules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AstraeaConfig {
    pub preset: AstraeaPreset,
    pub show_cwd: bool,
    pub show_git: bool,
    pub show_toolchains: bool,
    pub show_cloud: bool,
    pub show_time: bool,
    pub show_battery: bool,
    pub show_system_metrics: bool,
    pub transient_symbol: String,
    pub multiline: bool,
}

impl Default for AstraeaConfig {
    fn default() -> Self {
        Self {
            preset: AstraeaPreset::Starship,
            show_cwd: true,
            show_git: true,
            show_toolchains: true,
            show_cloud: true,
            show_time: true,
            show_battery: true,
            show_system_metrics: false,
            transient_symbol: "❯".to_string(),
            multiline: true,
        }
    }
}

#[derive(Default, Clone)]
pub struct AstraeaCompiler {
    pub config: AstraeaConfig,
    snapshot: AstraeaSnapshot,
}

impl AstraeaCompiler {
    pub fn new() -> Self {
        Self {
            config: AstraeaConfig::default(),
            snapshot: AstraeaSnapshot::default(),
        }
    }

    pub fn with_snapshot(snapshot: AstraeaSnapshot) -> Self {
        let preset = snapshot.preset;
        Self {
            config: AstraeaConfig {
                preset,
                ..Default::default()
            },
            snapshot,
        }
    }

    pub fn with_config_and_snapshot(config: AstraeaConfig, snapshot: AstraeaSnapshot) -> Self {
        Self { config, snapshot }
    }

    pub fn from_vector(state_vector: &AstraeaStateVector) -> Self {
        let snapshot = state_vector.read_snapshot();
        let preset = snapshot.preset;
        Self {
            config: AstraeaConfig {
                preset,
                ..Default::default()
            },
            snapshot,
        }
    }

    pub fn update_snapshot(&mut self, snapshot: AstraeaSnapshot) {
        self.snapshot = snapshot;
    }

    pub fn snapshot(&self) -> &AstraeaSnapshot {
        &self.snapshot
    }

    /// Compiles a fully decorated prompt according to the selected preset in under 100 microseconds.
    pub fn compile_prompt(&self) -> String {
        match self.config.preset {
            AstraeaPreset::Starship => self.compile_starship(),
            AstraeaPreset::Minimal => self.compile_minimal(),
            AstraeaPreset::Powerline => self.compile_powerline(),
            AstraeaPreset::TokyoNight => self.compile_tokyo_night(),
            AstraeaPreset::Pastel => self.compile_pastel(),
            AstraeaPreset::Pure => self.compile_pure(),
            AstraeaPreset::BracketedSegments => self.compile_bracketed_segments(),
        }
    }

    /// Starship preset: rich multi-line layout with branch, toolchains, cloud, and timings.
    fn compile_starship(&self) -> String {
        let mut prompt = String::with_capacity(320);

        // Environment prefix (SSH / Container)
        if self.snapshot.in_container {
            prompt.push_str("\x1b[36m[📦 container]\x1b[0m ");
        }
        if self.snapshot.is_ssh {
            prompt.push_str("\x1b[35m[🌐 ssh]\x1b[0m ");
        }

        // 1. Directory
        if self.config.show_cwd {
            let cwd_visible = visible(&self.snapshot.cwd);
            prompt.push_str("\x1b[34m");
            prompt.push_str(&cwd_visible);
            prompt.push_str("\x1b[0m");
        }

        // 2. Git status & SCM operations
        if self.config.show_git && !self.snapshot.git_branch.is_empty() {
            prompt.push_str(" on ");
            let branch_color = if self.snapshot.is_dirty {
                "\x1b[33m"
            } else {
                "\x1b[32m"
            };
            prompt.push_str(branch_color);
            prompt.push_str(" ");
            prompt.push_str(&visible(&self.snapshot.git_branch));
            prompt.push_str("\x1b[0m");

            let mut git_badges = String::new();
            if self.snapshot.git_ahead > 0 {
                git_badges.push_str(&format!(" ⇡{}", self.snapshot.git_ahead));
            }
            if self.snapshot.git_behind > 0 {
                git_badges.push_str(&format!(" ⇣{}", self.snapshot.git_behind));
            }
            if self.snapshot.is_dirty {
                git_badges.push('*');
            }
            if self.snapshot.scm_op != ScmOperation::None {
                git_badges.push(' ');
                git_badges.push_str(self.snapshot.scm_op.as_str());
            }
            if !git_badges.is_empty() {
                prompt.push_str("\x1b[33m[");
                prompt.push_str(git_badges.trim());
                prompt.push_str("]\x1b[0m");
            }
        }

        // 3. Toolchain badges
        if self.config.show_toolchains {
            if let Some(rust) = &self.snapshot.rust_version {
                prompt.push_str(" \x1b[36m[");
                prompt.push_str(&visible(rust));
                prompt.push_str("]\x1b[0m");
            }
            if let Some(node) = &self.snapshot.node_version {
                prompt.push_str(" \x1b[32m[");
                prompt.push_str(&visible(node));
                prompt.push_str("]\x1b[0m");
            }
            if let Some(py) = &self.snapshot.python_version {
                prompt.push_str(" \x1b[33m[");
                prompt.push_str(&visible(py));
                prompt.push_str("]\x1b[0m");
            }
            if let Some(go) = &self.snapshot.go_version {
                prompt.push_str(" \x1b[36m[");
                prompt.push_str(&visible(go));
                prompt.push_str("]\x1b[0m");
            }
            if let Some(zig) = &self.snapshot.zig_version {
                prompt.push_str(" \x1b[35m[");
                prompt.push_str(&visible(zig));
                prompt.push_str("]\x1b[0m");
            }
        }

        // 4. Cloud context
        if self.config.show_cloud {
            if let Some(cloud) = &self.snapshot.cloud_context {
                prompt.push_str(" \x1b[35m[");
                prompt.push_str(&visible(cloud));
                prompt.push_str("]\x1b[0m");
            }
        }

        // 5. Execution timing
        if self.config.show_time {
            if self.snapshot.execution_time_ms >= 1000 {
                let secs = self.snapshot.execution_time_ms as f64 / 1000.0;
                prompt.push_str(&format!(" \x1b[33m[{secs:.1}s]\x1b[0m"));
            } else if self.snapshot.execution_time_ms > 500 {
                prompt.push_str(&format!(
                    " \x1b[33m[{}ms]\x1b[0m",
                    self.snapshot.execution_time_ms
                ));
            }
        }

        // 6. Battery warning
        if self.config.show_battery {
            if let Some(pct) = self.snapshot.battery_pct {
                if pct <= 20 && !self.snapshot.battery_charging {
                    prompt.push_str(&format!(" \x1b[31m[⚡ {pct}%]\x1b[0m"));
                }
            }
        }

        // 7. System metrics (resident RAM / CPU)
        if self.config.show_system_metrics && self.snapshot.memory_used_mb > 0 {
            prompt.push_str(&format!(
                " \x1b[38;5;244m[{}MB]\x1b[0m",
                self.snapshot.memory_used_mb
            ));
        }

        // 8. Prompt arrow line
        if self.config.multiline {
            prompt.push('\n');
        } else {
            prompt.push(' ');
        }

        let symbol = &self.config.transient_symbol;
        let arrow = if self.snapshot.exit_code == 0 {
            format!("\x1b[32m{symbol}\x1b[0m")
        } else {
            format!("\x1b[31m{symbol}\x1b[0m")
        };
        prompt.push_str(&arrow);
        prompt.push(' ');

        prompt
    }

    /// Minimal preset: ultra-fast clean path and arrow.
    fn compile_minimal(&self) -> String {
        let symbol = &self.config.transient_symbol;
        let color = if self.snapshot.exit_code == 0 {
            "\x1b[32m"
        } else {
            "\x1b[31m"
        };
        let branch = if !self.snapshot.git_branch.is_empty() {
            format!(
                " \x1b[38;5;244m({})\x1b[0m",
                visible(&self.snapshot.git_branch)
            )
        } else {
            String::new()
        };
        format!(
            "\x1b[34m{}\x1b[0m{branch} {color}{symbol}\x1b[0m ",
            visible(&self.snapshot.cwd)
        )
    }

    /// Powerline preset: solid color segments with glyph dividers (\u{e0b0}).
    fn compile_powerline(&self) -> String {
        let mut prompt = String::with_capacity(256);
        let divider = "\u{e0b0}";

        // Directory block (Blue background, white text)
        prompt.push_str("\x1b[44;37m ");
        prompt.push_str(&visible(&self.snapshot.cwd));
        prompt.push(' ');

        // Git block (Dark Cyan / Green background)
        if !self.snapshot.git_branch.is_empty() {
            let (bg, code) = if self.snapshot.is_dirty {
                ("\x1b[43;34m", "\x1b[43;30m")
            } else {
                ("\x1b[42;34m", "\x1b[42;30m")
            };
            prompt.push_str(bg);
            prompt.push_str(divider);
            prompt.push_str(code);
            prompt.push_str("  ");
            prompt.push_str(&visible(&self.snapshot.git_branch));
            prompt.push(' ');
            prompt.push_str("\x1b[49;32m");
            prompt.push_str(divider);
        } else {
            prompt.push_str("\x1b[49;34m");
            prompt.push_str(divider);
        }
        prompt.push_str("\x1b[0m ");
        prompt
    }

    /// Tokyo Night preset: sleek slate blue, vibrant magenta, and neon cyan.
    fn compile_tokyo_night(&self) -> String {
        let symbol = &self.config.transient_symbol;
        let arrow_color = if self.snapshot.exit_code == 0 {
            "\x1b[38;2;187;154;247m" // neon violet
        } else {
            "\x1b[38;2;247;118;142m" // red
        };

        let mut prompt = format!(
            "\x1b[38;2;122;162;247m{}\x1b[0m", // tokyo blue
            visible(&self.snapshot.cwd)
        );

        if !self.snapshot.git_branch.is_empty() {
            prompt.push_str(&format!(
                " \x1b[38;2;158;206;106m {}\x1b[0m", // tokyo green
                visible(&self.snapshot.git_branch)
            ));
        }

        if let Some(rust) = &self.snapshot.rust_version {
            prompt.push_str(&format!(" \x1b[38;2;224;175;104m[{rust}]\x1b[0m"));
        }

        prompt.push('\n');
        prompt.push_str(&format!("{arrow_color}{symbol}\x1b[0m "));
        prompt
    }

    /// Pastel preset: gentle soft colors.
    fn compile_pastel(&self) -> String {
        let symbol = &self.config.transient_symbol;
        let color = if self.snapshot.exit_code == 0 {
            "\x1b[38;2;166;227;161m" // pastel green
        } else {
            "\x1b[38;2;243;139;168m" // pastel red
        };
        format!(
            "\x1b[38;2;137;180;250m{}\x1b[0m {color}{symbol}\x1b[0m ",
            visible(&self.snapshot.cwd)
        )
    }

    /// Pure preset: minimal 2-line layout.
    fn compile_pure(&self) -> String {
        let symbol = &self.config.transient_symbol;
        let color = if self.snapshot.exit_code == 0 {
            "\x1b[35m"
        } else {
            "\x1b[31m"
        };
        let branch = if !self.snapshot.git_branch.is_empty() {
            format!(
                " \x1b[38;5;242m{}\x1b[0m",
                visible(&self.snapshot.git_branch)
            )
        } else {
            String::new()
        };
        format!(
            "\x1b[36m{}\x1b[0m{branch}\n{color}{symbol}\x1b[0m ",
            visible(&self.snapshot.cwd)
        )
    }

    /// Starship Bracketed Segments preset: modular square-bracket capsules with vivid color-coded modules.
    fn compile_bracketed_segments(&self) -> String {
        let mut prompt = String::with_capacity(320);

        // Bracket delimiter styling: high-tech slate/muted gray [ and ]
        let b_open = "\x1b[38;5;242m[\x1b[0m";
        let b_close = "\x1b[38;5;242m]\x1b[0m ";

        // 1. Environment prefix (Container / SSH)
        if self.snapshot.in_container {
            prompt.push_str(b_open);
            prompt.push_str("\x1b[1;36m📦 container\x1b[0m");
            prompt.push_str(b_close);
        }
        if self.snapshot.is_ssh {
            prompt.push_str(b_open);
            prompt.push_str("\x1b[1;35m🌐 ssh\x1b[0m");
            prompt.push_str(b_close);
        }

        // 2. Directory segment [path]
        if self.config.show_cwd {
            let cwd_visible = visible(&self.snapshot.cwd);
            prompt.push_str(b_open);
            prompt.push_str("\x1b[1;36m");
            prompt.push_str(&cwd_visible);
            prompt.push_str("\x1b[0m");
            prompt.push_str(b_close);
        }

        // 3. Git branch and status segment [ branch * ⇡1⇣2 REBASING]
        if self.config.show_git && !self.snapshot.git_branch.is_empty() {
            prompt.push_str(b_open);
            let branch_color = if self.snapshot.is_dirty {
                "\x1b[1;33m"
            } else {
                "\x1b[1;35m"
            };
            prompt.push_str(branch_color);
            prompt.push_str(" ");
            prompt.push_str(&visible(&self.snapshot.git_branch));
            prompt.push_str("\x1b[0m");

            if self.snapshot.is_dirty {
                prompt.push_str("\x1b[1;31m*\x1b[0m");
            }
            if self.snapshot.git_ahead > 0 {
                prompt.push_str(&format!(" \x1b[1;32m⇡{}\x1b[0m", self.snapshot.git_ahead));
            }
            if self.snapshot.git_behind > 0 {
                prompt.push_str(&format!(" \x1b[1;31m⇣{}\x1b[0m", self.snapshot.git_behind));
            }
            if self.snapshot.scm_op != ScmOperation::None {
                prompt.push_str(&format!(
                    " \x1b[1;33m{}\x1b[0m",
                    self.snapshot.scm_op.as_str()
                ));
            }
            prompt.push_str(b_close);
        }

        // 4. Toolchain segments
        if self.config.show_toolchains {
            if let Some(rust) = &self.snapshot.rust_version {
                prompt.push_str(b_open);
                prompt.push_str("\x1b[38;5;208m🦀 ");
                prompt.push_str(&visible(rust));
                prompt.push_str("\x1b[0m");
                prompt.push_str(b_close);
            }
            if let Some(node) = &self.snapshot.node_version {
                prompt.push_str(b_open);
                prompt.push_str("\x1b[32m ");
                prompt.push_str(&visible(node));
                prompt.push_str("\x1b[0m");
                prompt.push_str(b_close);
            }
            if let Some(py) = &self.snapshot.python_version {
                prompt.push_str(b_open);
                prompt.push_str("\x1b[33m🐍 ");
                prompt.push_str(&visible(py));
                prompt.push_str("\x1b[0m");
                prompt.push_str(b_close);
            }
            if let Some(go) = &self.snapshot.go_version {
                prompt.push_str(b_open);
                prompt.push_str("\x1b[36m🐹 ");
                prompt.push_str(&visible(go));
                prompt.push_str("\x1b[0m");
                prompt.push_str(b_close);
            }
            if let Some(zig) = &self.snapshot.zig_version {
                prompt.push_str(b_open);
                prompt.push_str("\x1b[35m⚡ ");
                prompt.push_str(&visible(zig));
                prompt.push_str("\x1b[0m");
                prompt.push_str(b_close);
            }
        }

        // 5. Cloud context [☁️ context]
        if self.config.show_cloud {
            if let Some(cloud) = &self.snapshot.cloud_context {
                prompt.push_str(b_open);
                prompt.push_str("\x1b[1;34m☁️ ");
                prompt.push_str(&visible(cloud));
                prompt.push_str("\x1b[0m");
                prompt.push_str(b_close);
            }
        }

        // 6. Execution timing [⏱ 1.2s]
        if self.config.show_time {
            if self.snapshot.execution_time_ms >= 1000 {
                let secs = self.snapshot.execution_time_ms as f64 / 1000.0;
                prompt.push_str(b_open);
                prompt.push_str(&format!("\x1b[1;33m⏱ {secs:.1}s\x1b[0m"));
                prompt.push_str(b_close);
            } else if self.snapshot.execution_time_ms > 500 {
                prompt.push_str(b_open);
                prompt.push_str(&format!(
                    "\x1b[1;33m⏱ {}ms\x1b[0m",
                    self.snapshot.execution_time_ms
                ));
                prompt.push_str(b_close);
            }
        }

        // 7. Battery warning [⚡ 15%]
        if self.config.show_battery {
            if let Some(pct) = self.snapshot.battery_pct {
                if pct <= 20 && !self.snapshot.battery_charging {
                    prompt.push_str(b_open);
                    prompt.push_str(&format!("\x1b[1;31m⚡ {pct}%\x1b[0m"));
                    prompt.push_str(b_close);
                }
            }
        }

        // 8. System metrics [2048MB]
        if self.config.show_system_metrics && self.snapshot.memory_used_mb > 0 {
            prompt.push_str(b_open);
            prompt.push_str(&format!(
                "\x1b[38;5;244m{}MB\x1b[0m",
                self.snapshot.memory_used_mb
            ));
            prompt.push_str(b_close);
        }

        // 9. Line break & Character symbol
        if self.config.multiline {
            prompt.push('\n');
        } else if !prompt.is_empty() && !prompt.ends_with(' ') {
            prompt.push(' ');
        }

        let symbol = &self.config.transient_symbol;
        let char_color = if self.snapshot.exit_code == 0 {
            "\x1b[1;32m"
        } else {
            "\x1b[1;31m"
        };
        prompt.push_str(b_open);
        prompt.push_str(char_color);
        prompt.push_str(symbol);
        prompt.push_str("\x1b[0m");
        prompt.push_str("\x1b[38;5;242m]\x1b[0m ");

        prompt
    }

    /// Strict prompt transience: collapse historical decorated multi-line prompt to a single character.
    pub fn compile_transient_prompt(&self) -> String {
        let symbol = &self.config.transient_symbol;
        let arrow = if self.snapshot.exit_code == 0 {
            format!("\x1b[32m{symbol}\x1b[0m")
        } else {
            format!("\x1b[31m{symbol}\x1b[0m")
        };
        format!("{arrow} ")
    }

    /// Generates standard VT100 control codes to collapse the historical prompt lines and print transient line.
    pub fn transient_collapse_escape_sequence(
        &self,
        lines_to_erase: usize,
        executed_command: &str,
    ) -> String {
        let transient = self.compile_transient_prompt();
        if lines_to_erase == 0 {
            return format!("\r{transient}{executed_command}\n");
        }
        format!(
            "\x1b[{}A\r\x1b[0J{}{}\n",
            lines_to_erase, transient, executed_command
        )
    }
}

/// Helper shell integration scripts for automatic prompt and transient hook registration.
pub struct AstraeaShellIntegration;

impl AstraeaShellIntegration {
    /// Native Zsh hook snippet.
    pub fn zsh() -> &'static str {
        r#"# Astraea Shell Integration for Zsh
autoload -Uz add-zsh-hook

_astraea_precmd() {
    PROMPT="$(well prompt 2>/dev/null || echo '%~ ❯ ')"
}

_astraea_preexec() {
    # Prompt transience: collapse to single compact symbol upon command execution
    local transient="$(well prompt --transient 2>/dev/null || echo '❯ ')"
    print -rn -- $'\e[1A\r\e[0J'"${transient}${1}"$'\n'
}

add-zsh-hook precmd _astraea_precmd
add-zsh-hook preexec _astraea_preexec
"#
    }

    /// Native Fish hook snippet.
    pub fn fish() -> &'static str {
        r#"# Astraea Shell Integration for Fish
function fish_prompt
    well prompt 2>/dev/null; or echo (prompt_pwd) ' ❯ '
end

function _astraea_transient --on-event fish_preexec
    set -l transient (well prompt --transient 2>/dev/null; or echo '❯ ')
    echo -en "\e[1A\r\e[0J$transient$argv\n"
end
"#
    }

    /// Native Bash hook snippet.
    pub fn bash() -> &'static str {
        r#"# Astraea Shell Integration for Bash
_astraea_prompt_cmd() {
    PS1="$(well prompt 2>/dev/null || echo '\w ❯ ')"
}
PROMPT_COMMAND="_astraea_prompt_cmd; $PROMPT_COMMAND"
"#
    }
}

fn visible(value: &str) -> String {
    let mut visible = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control() {
            visible.extend(ch.escape_default());
        } else {
            visible.push(ch);
        }
    }
    visible
}
