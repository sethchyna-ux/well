//! Pythia: Terminal AI Copilot Subsystem for Well-Shell
//! Provides data contracts and asynchronous intelligence pipelines for shell command
//! synthesis, stderr diagnosis, and command explanation.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PythiaProvider {
    #[default]
    GeminiFlash,
    GeminiPro,
    LocalGemma,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryIntent {
    NaturalLanguageSynthesis,
    StderrDiagnosis,
    CommandExplanation,
    GeneralAssistance,
    ImageGeneration,
}

#[derive(Debug, Clone)]
pub struct PythiaRequest {
    pub prompt: String,
    pub intent: QueryIntent,
    pub current_working_dir: Option<String>,
    pub last_exit_code: Option<i32>,
    pub stderr_context: Option<String>,
    pub provider: PythiaProvider,
    pub image_prompt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl PythiaRequest {
    pub fn new_synthesis(prompt: impl Into<String>, cwd: Option<String>) -> Self {
        Self {
            prompt: prompt.into(),
            intent: QueryIntent::NaturalLanguageSynthesis,
            current_working_dir: cwd,
            last_exit_code: None,
            stderr_context: None,
            provider: PythiaProvider::default(),
            image_prompt: None,
            width: None,
            height: None,
        }
    }

    /// Create a request for image generation. The `prompt` describes the desired image.
    pub fn new_image(prompt: impl Into<String>, cwd: Option<String>, width: Option<u32>, height: Option<u32>) -> Self {
        Self {
            prompt: String::new(),
            intent: QueryIntent::ImageGeneration,
            current_working_dir: cwd,
            last_exit_code: None,
            stderr_context: None,
            provider: PythiaProvider::default(),
            image_prompt: Some(prompt.into()),
            width,
            height,
        }
    }

    pub fn new_diagnosis(
        exit_code: i32,
        stderr: impl Into<String>,
        cwd: Option<String>,
    ) -> Self {
        Self {
            prompt: String::new(),
            intent: QueryIntent::StderrDiagnosis,
            current_working_dir: cwd,
            last_exit_code: Some(exit_code),
            stderr_context: Some(stderr.into()),
            provider: PythiaProvider::default(),
            image_prompt: None,
            width: None,
            height: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PythiaResponse {
    pub suggested_command: Option<String>,
    pub explanation: String,
    pub is_destructive: bool,
    pub confidence_score: f32,
    /// Path to a persisted image generated for ImageGeneration intents.
    pub generated_image_path: Option<String>,
}

impl PythiaResponse {
    pub fn new(suggested: Option<String>, explanation: impl Into<String>, confidence: f32) -> Self {
        let is_destructive = suggested
            .as_ref()
            .map(|cmd| detect_destructive_command(cmd))
            .unwrap_or(false);

        Self {
            suggested_command: suggested,
            explanation: explanation.into(),
            is_destructive,
            confidence_score: confidence.clamp(0.0, 1.0),
            generated_image_path: None,
        }
    }

    /// Construct a response that includes a generated image.
    pub fn with_image(mut self, image_path: impl Into<String>) -> Self {
        self.generated_image_path = Some(image_path.into());
        self
    }
}

/// Detects potentially destructive shell commands to warn the user before execution.
pub fn detect_destructive_command(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();

    if tokens.is_empty() {
        return false;
    }

    // Direct destructive patterns
    let has_rm_rf = tokens.contains(&"rm")
        && (tokens.iter().any(|t| t.starts_with("-") && t.contains('r') && t.contains('f'))
            || (tokens.contains(&"-r") && tokens.contains(&"-f")));

    let has_root_target = lower.contains(" /") || lower.ends_with(" /") || lower.contains(" ~/");

    let has_dd = tokens.first() == Some(&"dd") || tokens.iter().any(|t| t.starts_with("of=/dev/"));
    let has_mkfs = tokens.iter().any(|t| t.starts_with("mkfs"));
    let has_forkbomb = lower.contains(":(){ :|:& };:");
    let has_chmod_777 = lower.contains("chmod -r 777") || lower.contains("chmod 777");

    (has_rm_rf && has_root_target) || has_dd || has_mkfs || has_forkbomb || has_chmod_777
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_destructive_command_detection() {
        assert!(detect_destructive_command("rm -rf /"));
        assert!(detect_destructive_command("rm -rf /usr"));
        assert!(detect_destructive_command("dd if=/dev/zero of=/dev/sda"));
        assert!(detect_destructive_command("mkfs.ext4 /dev/nvme0n1"));
        assert!(detect_destructive_command("chmod -R 777 /var/www"));

        assert!(!detect_destructive_command("cargo build --release"));
        assert!(!detect_destructive_command("ls -la"));
        assert!(!detect_destructive_command("git status"));
    }

    #[test]
    fn test_pythia_response_construction() {
        let resp = PythiaResponse::new(
            Some("rm -rf /".to_string()),
            "Deletes everything",
            0.95,
        );
        assert!(resp.is_destructive);
        assert_eq!(resp.confidence_score, 0.95);

        let safe_resp = PythiaResponse::new(
            Some("cargo check".to_string()),
            "Checks compilation",
            1.5, // should clamp to 1.0
        );
        assert!(!safe_resp.is_destructive);
        assert_eq!(safe_resp.confidence_score, 1.0);
    }
}
