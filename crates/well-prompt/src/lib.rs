use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    #[default]
    Insert,
    Normal,
    Visual,
    Replace,
}

impl VimMode {
    fn label(self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Normal => "NORMAL",
            Self::Visual => "VISUAL",
            Self::Replace => "REPLACE",
        }
    }

    fn character(self) -> &'static str {
        match self {
            Self::Normal | Self::Visual | Self::Replace => "❮",
            Self::Insert => "❯",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptContext {
    pub current_dir: PathBuf,
    pub exit_code: Option<i32>,
    pub command_duration: Duration,
    pub active_jobs: usize,
    pub vim_mode: VimMode,
}

impl Default for PromptContext {
    fn default() -> Self {
        Self {
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            exit_code: None,
            command_duration: Duration::ZERO,
            active_jobs: 0,
            vim_mode: VimMode::Insert,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Literal(String),
    CurrentDir,
    ExitCode,
    DurationMs,
    ActiveJobs,
    VimMode,
    Character,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplateError {
    pub byte_offset: usize,
    pub message: String,
}

impl std::fmt::Display for PromptTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid prompt template at byte {}: {}",
            self.byte_offset, self.message
        )
    }
}

impl std::error::Error for PromptTemplateError {}

impl PromptTemplate {
    pub fn compile(source: &str) -> Result<Self, PromptTemplateError> {
        let mut segments = Vec::new();
        let mut offset = 0;
        while let Some(open_relative) = source[offset..].find('{') {
            let open = offset + open_relative;
            if open > offset {
                segments.push(Segment::Literal(source[offset..open].to_owned()));
            }
            let value_start = open + 1;
            let Some(close_relative) = source[value_start..].find('}') else {
                return Err(PromptTemplateError {
                    byte_offset: open,
                    message: "unclosed placeholder".into(),
                });
            };
            let close = value_start + close_relative;
            let segment = match &source[value_start..close] {
                "cwd" => Segment::CurrentDir,
                "exit_code" => Segment::ExitCode,
                "duration_ms" => Segment::DurationMs,
                "jobs" => Segment::ActiveJobs,
                "vim_mode" => Segment::VimMode,
                "character" => Segment::Character,
                name => {
                    return Err(PromptTemplateError {
                        byte_offset: open,
                        message: format!("unknown placeholder {{{name}}}"),
                    });
                }
            };
            segments.push(segment);
            offset = close + 1;
        }

        if offset < source.len() {
            let tail = &source[offset..];
            if let Some(open_relative) = tail.find('}') {
                return Err(PromptTemplateError {
                    byte_offset: offset + open_relative,
                    message: "unmatched closing brace".into(),
                });
            }
            segments.push(Segment::Literal(tail.to_owned()));
        }

        Ok(Self { segments })
    }

    pub fn render(&self, context: &PromptContext) -> String {
        let mut rendered = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(text) => rendered.push_str(text),
                Segment::CurrentDir => {
                    rendered.push_str(&context.current_dir.display().to_string())
                }
                Segment::ExitCode => {
                    if let Some(code) = context.exit_code {
                        rendered.push_str(&code.to_string());
                    }
                }
                Segment::DurationMs => {
                    rendered.push_str(&context.command_duration.as_millis().to_string())
                }
                Segment::ActiveJobs => rendered.push_str(&context.active_jobs.to_string()),
                Segment::VimMode => rendered.push_str(context.vim_mode.label()),
                Segment::Character => rendered.push_str(context.vim_mode.character()),
            }
        }
        rendered
    }
}

/// Compile a prompt format string for repeated rendering with changing state.
pub fn compile_prompt(source: &str) -> Result<PromptTemplate, PromptTemplateError> {
    PromptTemplate::compile(source)
}

#[cfg(test)]
mod tests {
    use super::{compile_prompt, PromptContext, VimMode};
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn renders_context_placeholders_and_character() {
        let template =
            compile_prompt("{cwd} {exit_code} {duration_ms} {jobs} {vim_mode}{character}").unwrap();
        let context = PromptContext {
            current_dir: PathBuf::from("/work"),
            exit_code: Some(7),
            command_duration: Duration::from_micros(1_999),
            active_jobs: 2,
            vim_mode: VimMode::Visual,
        };

        assert_eq!(template.render(&context), "/work 7 1 2 VISUAL❮");
    }

    #[test]
    fn missing_exit_status_renders_empty_and_failures_keep_insert_character() {
        let template = compile_prompt("{exit_code}{character}").unwrap();
        let context = PromptContext::default();
        assert_eq!(template.render(&context), "❯");

        let failed = PromptContext {
            exit_code: Some(1),
            ..context
        };
        assert_eq!(template.render(&failed), "1❯");
    }

    #[test]
    fn rejects_unknown_and_unbalanced_placeholders_with_offsets() {
        let error = compile_prompt("x {unknown}").unwrap_err();
        assert_eq!(error.byte_offset, 2);
        assert!(error.message.contains("unknown placeholder"));

        let error = compile_prompt("x }").unwrap_err();
        assert_eq!(error.byte_offset, 2);
        assert!(error.message.contains("unmatched closing"));

        let error = compile_prompt("x {cwd").unwrap_err();
        assert_eq!(error.byte_offset, 2);
        assert!(error.message.contains("unclosed"));
    }
}
