#[derive(Debug, Clone, Default)]
pub struct PromptState {
    pub cwd: String,
    pub git_branch: String,
    pub is_dirty: bool,
    pub exit_code: i32,
    pub execution_time_ms: u64,
}

#[derive(Default)]
pub struct AstraeaCompiler {
    state: PromptState,
}

impl AstraeaCompiler {
    pub fn new() -> Self {
        Self {
            state: PromptState::default(),
        }
    }

    pub fn update_state(&mut self, state: PromptState) {
        self.state = state;
    }

    /// Format cached prompt state without filesystem queries or subprocesses.
    pub fn compile_prompt(&self) -> String {
        let branch = if self.state.git_branch.is_empty() {
            String::new()
        } else {
            let color = if self.state.is_dirty {
                "\x1b[33m"
            } else {
                "\x1b[32m"
            };
            format!(
                " on {color} {}{}",
                visible(&self.state.git_branch),
                "\x1b[0m"
            )
        };

        let cwd = format!("\x1b[34m{}\x1b[0m", visible(&self.state.cwd));
        let arrow = if self.state.exit_code == 0 {
            "\x1b[32m❯\x1b[0m"
        } else {
            "\x1b[31m❯\x1b[0m"
        };

        let timing = if self.state.execution_time_ms > 2000 {
            format!(" \x1b[33m{}ms\x1b[0m", self.state.execution_time_ms)
        } else {
            String::new()
        };

        format!("{}{}{}\n{} ", cwd, branch, timing, arrow)
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filesystem_and_branch_names_cannot_inject_terminal_controls() {
        let mut compiler = AstraeaCompiler::new();
        compiler.update_state(PromptState {
            cwd: "/tmp/\x1b[2J\nname".into(),
            git_branch: "branch\x07".into(),
            exit_code: 1,
            ..Default::default()
        });
        let prompt = compiler.compile_prompt();
        assert!(!prompt.contains("\x1b[2J"));
        assert!(!prompt.contains('\x07'));
        assert_eq!(prompt.chars().filter(|&ch| ch == '\n').count(), 1);
        assert!(prompt.contains("\\u{1b}"));
        assert!(prompt.contains("\x1b[31m"));
    }
}
