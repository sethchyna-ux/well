//! well-cli: Standalone Terminal Engine for Android (Termux / ADB) and Headless POSIX
//!
//! Provides a direct CLI/PTY runner using Well's Hypershell, Astraea prompt,
//! and Mneme editor without desktop GUI windowing.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use well_shell::pty::PtySession;

fn get_terminal_size() -> (u16, u16) {
    #[cfg(unix)]
    unsafe {
        let mut winsize: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) == 0
            && winsize.ws_col > 0
            && winsize.ws_row > 0
        {
            return (winsize.ws_row, winsize.ws_col);
        }
    }
    (24, 80)
}

#[cfg(unix)]
struct RawTerminalGuard {
    orig_termios: libc::termios,
}

#[cfg(unix)]
impl RawTerminalGuard {
    fn enter() -> io::Result<Self> {
        unsafe {
            let mut orig: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut orig) != 0 {
                return Err(io::Error::last_os_error());
            }

            let mut raw = orig;
            libc::cfmakeraw(&mut raw);
            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) != 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(Self { orig_termios: orig })
        }
    }
}

#[cfg(unix)]
impl Drop for RawTerminalGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.orig_termios);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let subcmd = args[1].as_str();
        if subcmd == "translate" || subcmd == "ai" || subcmd == "t" {
            let query = args[2..].join(" ");
            if query.trim().is_empty() {
                eprintln!("Usage: well-cli translate <natural language query>");
                std::process::exit(1);
            }

            let provider =
                if std::env::var("HF_TOKEN").is_ok() || std::env::var("WELL_USE_HF").is_ok() {
                    well_llm::LlmProvider::HuggingFaceGemmaAbliterated
                } else if std::env::var("GEMINI_API_KEY").is_ok() {
                    well_llm::LlmProvider::Gemini
                } else {
                    well_llm::LlmProvider::OfflineRules
                };

            let req = well_llm::TranslationRequest {
                query: query.clone(),
                shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
                cwd: std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                provider,
                gemini_api_key: std::env::var("GEMINI_API_KEY").ok(),
                gemini_model: Some("gemini-2.0-flash".to_string()),
                ollama_url: Some("http://localhost:11434".to_string()),
                ollama_model: Some("qwen2.5-coder".to_string()),
                hf_token: std::env::var("HF_TOKEN").ok(),
                hf_model: Some("failspy/gemma-2-9b-it-abliterated".to_string()),
                image_prompt: None,
                image_width: None,
                image_height: None,
                force_image_regeneration: false,
            };

            let result = well_llm::PythiaTranslator::translate(&req);
            println!("{}", result.command);
            return Ok(());
        } else if subcmd == "image" || subcmd == "img" {
            let prompt = args[2..].join(" ");
            if prompt.trim().is_empty() {
                eprintln!("Usage: well-cli image <prompt>");
                std::process::exit(1);
            }

            let provider =
                if std::env::var("HF_TOKEN").is_ok() || std::env::var("WELL_USE_HF").is_ok() {
                    well_llm::LlmProvider::HuggingFaceGemmaAbliterated
                } else if std::env::var("GEMINI_API_KEY").is_ok() {
                    well_llm::LlmProvider::Gemini
                } else {
                    well_llm::LlmProvider::OfflineRules
                };

            let req = well_llm::TranslationRequest {
                query: String::new(),
                shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
                cwd: std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                provider,
                gemini_api_key: std::env::var("GEMINI_API_KEY").ok(),
                gemini_model: Some("gemini-2.0-flash".to_string()),
                ollama_url: Some("http://localhost:11434".to_string()),
                ollama_model: Some("qwen2.5-coder".to_string()),
                hf_token: std::env::var("HF_TOKEN").ok(),
                hf_model: Some("runwayml/stable-diffusion-v1-5".to_string()),
                image_prompt: Some(prompt.clone()),
                image_width: Some(512),
                image_height: Some(512),
                force_image_regeneration: false,
            };

            let result = well_llm::PythiaTranslator::translate(&req);
            if let Some(path) = result.generated_image_path {
                println!("Generated image artifact: {}", path);
            } else {
                eprintln!("Image generation failed: {}", result.explanation);
                std::process::exit(1);
            }
            return Ok(());
        }
    }

    let (rows, cols) = get_terminal_size();

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_writer = is_running.clone();

    // Spawn PTY session connected to OS shell ($SHELL or /system/bin/sh on Android)
    // Directly stream output to stdout in real time
    let pty = PtySession::spawn_with_stream(rows, cols, None, move |chunk| {
        let mut stdout = io::stdout();
        let _ = stdout.write_all(chunk);
        let _ = stdout.flush();
    })?;

    #[cfg(unix)]
    let _guard = RawTerminalGuard::enter()?;

    // Read loop: forward stdin to PTY
    let mut stdin = io::stdin();
    let mut in_buf = [0u8; 1024];

    while is_running.load(Ordering::Relaxed) && pty.is_alive() {
        match stdin.read(&mut in_buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                if pty.write_all(&in_buf[..n]).is_err() {
                    break;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }

    is_running_writer.store(false, Ordering::Relaxed);
    Ok(())
}
