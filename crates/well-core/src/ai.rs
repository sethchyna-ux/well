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
    pub fn new_image(
        prompt: impl Into<String>,
        cwd: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Self {
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

    pub fn new_diagnosis(exit_code: i32, stderr: impl Into<String>, cwd: Option<String>) -> Self {
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
///
/// This is a conservative warning heuristic, not a shell sandbox or an authorization
/// boundary. Callers must still require appropriate user confirmation before execution.
pub fn detect_destructive_command(cmd: &str) -> bool {
    let lower = unquoted_shell_text(cmd).to_ascii_lowercase();
    if lower.contains(":(){ :|:& };:") || lower.contains(": () { : | : & }; :") {
        return true;
    }

    if contains_destructive_command_substitution(cmd) {
        return true;
    }

    shell_command_segments(cmd)
        .iter()
        .any(|segment| segment_is_destructive(segment))
}

/// Split a shell command into executable segments while preserving quoted text as one word.
/// This is deliberately a small lexer, not a shell parser: the safety gate should remain
/// conservative and must never be used as a sandbox or authorization boundary.
fn shell_command_segments(input: &str) -> Vec<Vec<String>> {
    let mut segments = Vec::new();
    let mut segment = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;

    let push_word = |segment: &mut Vec<String>, word: &mut String| {
        if !word.is_empty() {
            segment.push(std::mem::take(word));
        }
    };
    let push_segment = |segments: &mut Vec<Vec<String>>, segment: &mut Vec<String>| {
        if !segment.is_empty() {
            segments.push(std::mem::take(segment));
        }
    };

    for ch in input.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
            continue;
        }

        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    word.push(ch);
                }
            }
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => escaped = true,
                _ => word.push(ch),
            },
            Some(_) => unreachable!(),
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => escaped = true,
                ';' | '\n' | '|' | '&' => {
                    push_word(&mut segment, &mut word);
                    push_segment(&mut segments, &mut segment);
                }
                ch if ch.is_whitespace() => push_word(&mut segment, &mut word),
                _ => word.push(ch),
            },
        }
    }

    if escaped {
        word.push('\\');
    }
    push_word(&mut segment, &mut word);
    push_segment(&mut segments, &mut segment);
    segments
}

fn segment_is_destructive(segment: &[String]) -> bool {
    let Some((command, args)) = executable_and_args(segment) else {
        return false;
    };
    let command = command
        .rsplit('/')
        .next()
        .unwrap_or(command)
        .to_ascii_lowercase();
    let normalized_args: Vec<String> = args.iter().map(|arg| arg.to_ascii_lowercase()).collect();
    match command.as_str() {
        // File deletion and direct file-content destruction.
        "rm" | "rmdir" | "unlink" | "shred" | "truncate" => true,
        "find" => find_command_is_destructive(&normalized_args),

        // Filesystem, partition, and raw-device operations.
        "dd" | "wipefs" | "fdisk" | "sfdisk" | "cfdisk" | "parted" => true,
        command if command.starts_with("mkfs") => true,
        "diskutil" => normalized_args.iter().any(|arg| {
            matches!(
                arg.as_str(),
                "erasevolume"
                    | "erasedisk"
                    | "partitiondisk"
                    | "zerodisk"
                    | "randomdisk"
                    | "secureerase"
                    | "deletevolume"
            )
        }),

        // Permission and ownership mutations warrant confirmation even when scoped to one file.
        "chmod" | "chown" | "chgrp" => permission_command_is_destructive(&normalized_args),

        // Common source-control operations that discard uncommitted data.
        "git" => git_command_is_destructive(&normalized_args),

        // Infrastructure and container commands with explicit deletion semantics.
        "docker" | "podman" => container_command_is_destructive(&normalized_args),
        "kubectl" => normalized_args.first().is_some_and(|arg| arg == "delete"),
        "terraform" | "tofu" => normalized_args.first().is_some_and(|arg| arg == "destroy"),

        // Process/system shutdown commands should not run from an AI suggestion silently.
        "shutdown" | "reboot" | "halt" | "poweroff" => true,
        "kill" | "killall" | "pkill" => true,

        // Inspect commands passed to an explicit shell invocation.
        "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" => shell_payload_is_destructive(args),
        "eval" => eval_payload_is_destructive(args),
        "xargs" => xargs_command_is_destructive(args),
        _ => false,
    }
}

fn executable_and_args(segment: &[String]) -> Option<(&str, &[String])> {
    let mut index = 0;

    while index < segment.len() && is_environment_assignment(&segment[index]) {
        index += 1;
    }

    loop {
        let wrapper = segment.get(index)?.rsplit('/').next()?.to_ascii_lowercase();
        match wrapper.as_str() {
            "sudo" | "doas" => {
                index += 1;
                index = skip_wrapper_options(segment, index, wrapper.as_str());
            }
            "command" | "builtin" | "nohup" => index += 1,
            "env" => {
                index += 1;
                index = skip_wrapper_options(segment, index, "env");
                while segment
                    .get(index)
                    .is_some_and(|arg| is_environment_assignment(arg))
                {
                    index += 1;
                }
            }
            "nice" | "time" | "setsid" => {
                index += 1;
                index = skip_wrapper_options(segment, index, wrapper.as_str());
            }
            "timeout" => {
                index += 1;
                index = skip_wrapper_options(segment, index, "timeout");
                // timeout's first non-option operand is the duration, not the executable.
                if index < segment.len() {
                    index += 1;
                }
            }
            _ => break,
        }
    }

    let command = segment.get(index)?;
    Some((command, &segment[index + 1..]))
}

fn skip_wrapper_options(segment: &[String], mut index: usize, wrapper: &str) -> usize {
    while let Some(option) = segment.get(index) {
        if option == "--" {
            return index + 1;
        }
        if !option.starts_with('-') || option == "-" {
            break;
        }

        let consumes_value = !option.contains('=')
            && match wrapper {
                "sudo" => matches!(
                    option.as_str(),
                    "-u" | "--user"
                        | "-g"
                        | "--group"
                        | "-h"
                        | "--host"
                        | "-p"
                        | "--prompt"
                        | "-r"
                        | "--role"
                        | "-t"
                        | "--type"
                        | "-c"
                        | "--close-from"
                        | "-d"
                        | "--chdir"
                ),
                "doas" => matches!(option.as_str(), "-u"),
                "env" => matches!(
                    option.as_str(),
                    "-u" | "--unset" | "-c" | "--chdir" | "-s" | "--split-string"
                ),
                "nice" => matches!(option.as_str(), "-n" | "--adjustment"),
                "time" => matches!(option.as_str(), "-f" | "--format" | "-o" | "--output"),
                "timeout" => matches!(option.as_str(), "-k" | "--kill-after" | "-s" | "--signal"),
                _ => false,
            };
        index += if consumes_value { 2 } else { 1 };
    }
    index
}

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.chars().enumerate().all(|(index, ch)| {
            ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit())
        })
}

fn permission_command_is_destructive(args: &[String]) -> bool {
    !args.is_empty()
        && !args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "--version"))
}

fn find_command_is_destructive(args: &[String]) -> bool {
    if args.iter().any(|arg| arg == "-delete") {
        return true;
    }

    args.windows(2).any(|pair| {
        matches!(pair[0].as_str(), "-exec" | "-execdir" | "-ok" | "-okdir")
            && matches!(
                pair[1].rsplit('/').next(),
                Some("rm" | "rmdir" | "unlink" | "shred")
            )
    })
}

fn git_command_is_destructive(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("clean") => args.iter().skip(1).any(|arg| {
            arg == "--force"
                || arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains('f')
        }),
        Some("reset") => args.iter().any(|arg| arg == "--hard" || arg == "--merge"),
        Some("restore") => args
            .iter()
            .any(|arg| arg == "--worktree" || !arg.starts_with('-')),
        Some("checkout") => args.iter().any(|arg| arg == "--"),
        Some("branch") => args.iter().any(|arg| arg == "-d"),
        Some("stash") => args
            .get(1)
            .is_some_and(|arg| matches!(arg.as_str(), "drop" | "clear")),
        _ => false,
    }
}

fn container_command_is_destructive(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("rm") | Some("rmi") => true,
        Some("system") | Some("image") | Some("container") | Some("volume") | Some("network")
        | Some("builder") => args.get(1).is_some_and(|arg| arg == "prune" || arg == "rm"),
        Some("compose") => args
            .iter()
            .any(|arg| matches!(arg.as_str(), "down" | "rm" | "kill")),
        _ => false,
    }
}

fn shell_payload_is_destructive(args: &[String]) -> bool {
    args.iter()
        .position(|arg| {
            arg == "-c"
                || arg.starts_with('-')
                    && !arg.starts_with("--")
                    && arg[1..].chars().any(|option| option == 'c')
        })
        .and_then(|index| args.get(index + 1))
        .is_some_and(|payload| detect_destructive_command(payload))
}

fn eval_payload_is_destructive(args: &[String]) -> bool {
    !args.is_empty() && detect_destructive_command(&args.join(" "))
}

fn xargs_command_is_destructive(args: &[String]) -> bool {
    let mut index = 0;
    while let Some(option) = args.get(index) {
        if option == "--" {
            index += 1;
            break;
        }
        if !option.starts_with('-') || option == "-" {
            break;
        }

        let consumes_value = !option.contains('=')
            && matches!(
                option.as_str(),
                "-a" | "--arg-file"
                    | "-d"
                    | "--delimiter"
                    | "-E"
                    | "--eof"
                    | "-I"
                    | "--replace"
                    | "-J"
                    | "-L"
                    | "--max-lines"
                    | "-n"
                    | "--max-args"
                    | "-P"
                    | "--max-procs"
                    | "-R"
                    | "--max-replacements"
                    | "-S"
                    | "-s"
                    | "--max-chars"
                    | "--process-slot-var"
            );
        index += if consumes_value { 2 } else { 1 };
    }

    args.get(index..)
        .is_some_and(|command| !command.is_empty() && segment_is_destructive(command))
}

/// Return shell text outside quotes. This is used only for syntax-level hazards such as a
/// fork bomb; ordinary command detection retains quoted words so executable arguments work.
fn unquoted_shell_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut quote = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            output.push(' ');
            escaped = false;
            continue;
        }
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                }
                output.push(' ');
            }
            Some('"') => {
                if ch == '"' {
                    quote = None;
                } else if ch == '\\' {
                    escaped = true;
                }
                output.push(' ');
            }
            Some(_) => unreachable!(),
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    output.push(' ');
                }
                '\\' => {
                    escaped = true;
                    output.push(' ');
                }
                _ => output.push(ch),
            },
        }
    }
    output
}

/// Inspect command substitutions because their payload executes even when the surrounding
/// command is otherwise harmless (for example, `echo "$(rm file)"`). Single-quoted and
/// backslash-escaped mentions remain literal and are intentionally ignored.
fn contains_destructive_command_substitution(input: &str) -> bool {
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    let mut quote = None;

    while index < chars.len() {
        match quote {
            Some('\'') => {
                if chars[index] == '\'' {
                    quote = None;
                }
                index += 1;
            }
            Some('"') => match chars[index] {
                '"' => {
                    quote = None;
                    index += 1;
                }
                '\\' => index = (index + 2).min(chars.len()),
                '$' if chars.get(index + 1) == Some(&'(') => {
                    let Some((payload, next)) = command_substitution_payload(&chars, index + 2)
                    else {
                        return false;
                    };
                    if detect_destructive_command(&payload) {
                        return true;
                    }
                    index = next;
                }
                '`' => {
                    let Some((payload, next)) = backtick_payload(&chars, index + 1) else {
                        return false;
                    };
                    if detect_destructive_command(&payload) {
                        return true;
                    }
                    index = next;
                }
                _ => index += 1,
            },
            Some(_) => unreachable!(),
            None => match chars[index] {
                '\'' => {
                    quote = Some('\'');
                    index += 1;
                }
                '"' => {
                    quote = Some('"');
                    index += 1;
                }
                '\\' => index = (index + 2).min(chars.len()),
                '$' if chars.get(index + 1) == Some(&'(') => {
                    let Some((payload, next)) = command_substitution_payload(&chars, index + 2)
                    else {
                        return false;
                    };
                    if detect_destructive_command(&payload) {
                        return true;
                    }
                    index = next;
                }
                '`' => {
                    let Some((payload, next)) = backtick_payload(&chars, index + 1) else {
                        return false;
                    };
                    if detect_destructive_command(&payload) {
                        return true;
                    }
                    index = next;
                }
                _ => index += 1,
            },
        }
    }
    false
}

fn command_substitution_payload(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut index = start;
    let mut depth = 1;
    let mut quote = None;

    while index < chars.len() {
        match quote {
            Some('\'') => {
                if chars[index] == '\'' {
                    quote = None;
                }
                index += 1;
            }
            Some('"') => match chars[index] {
                '"' => {
                    quote = None;
                    index += 1;
                }
                '\\' => index = (index + 2).min(chars.len()),
                _ => index += 1,
            },
            Some(_) => unreachable!(),
            None => match chars[index] {
                '\'' | '"' => {
                    quote = Some(chars[index]);
                    index += 1;
                }
                '\\' => index = (index + 2).min(chars.len()),
                '(' => {
                    depth += 1;
                    index += 1;
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((chars[start..index].iter().collect(), index + 1));
                    }
                    index += 1;
                }
                _ => index += 1,
            },
        }
    }
    None
}

fn backtick_payload(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut index = start;
    while index < chars.len() {
        match chars[index] {
            '\\' => index = (index + 2).min(chars.len()),
            '`' => return Some((chars[start..index].iter().collect(), index + 1)),
            _ => index += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_destructive_file_and_disk_commands() {
        assert!(detect_destructive_command("rm -rf /"));
        assert!(detect_destructive_command("sudo rm -rf \"$HOME\""));
        assert!(detect_destructive_command("sudo -u root rm -rf /srv/app"));
        assert!(detect_destructive_command("env LC_ALL=C rm old.log"));
        assert!(detect_destructive_command("env -u DEBUG rm old.log"));
        assert!(detect_destructive_command("find . -name '*.tmp' -delete"));
        assert!(detect_destructive_command(
            "find . -name '*.tmp' -exec rm {} +"
        ));
        assert!(detect_destructive_command("truncate -s 0 important.db"));
        assert!(detect_destructive_command("dd if=/dev/zero of=/dev/sda"));
        assert!(detect_destructive_command("mkfs.ext4 /dev/nvme0n1"));
        assert!(detect_destructive_command(
            "diskutil eraseDisk APFS Empty /dev/disk4"
        ));
        assert!(detect_destructive_command("chmod -R 777 /var/www"));
        assert!(detect_destructive_command(
            "chown --recursive user /srv/data"
        ));
    }

    #[test]
    fn detects_destructive_commands_in_compound_shell_input() {
        assert!(detect_destructive_command("cargo build && rm -rf target"));
        assert!(detect_destructive_command(
            "echo ready; git reset --hard HEAD"
        ));
        assert!(detect_destructive_command(
            "bash -c 'find /tmp/cache -delete'"
        ));
        assert!(detect_destructive_command("FOO=bar sudo git clean -fdx"));
        assert!(detect_destructive_command("docker system prune -af"));
        assert!(detect_destructive_command(
            "kubectl delete namespace production"
        ));
        assert!(detect_destructive_command(
            "terraform destroy -auto-approve"
        ));
        assert!(detect_destructive_command("shutdown -h now"));
    }

    #[test]
    fn detects_destructive_commands_behind_execution_wrappers() {
        assert!(detect_destructive_command("eval 'rm -rf /tmp/cache'"));
        assert!(detect_destructive_command(
            "eval \"sudo git reset --hard HEAD\""
        ));
        assert!(detect_destructive_command(
            "printf '%s\\0' old.log | xargs -0 rm -f"
        ));
        assert!(detect_destructive_command(
            "find-pids | xargs -n 1 /bin/kill -9"
        ));
        assert!(detect_destructive_command("find . -print0 | xargs -0r rm"));
        assert!(detect_destructive_command("find-pids | xargs -P 4 kill"));
        assert!(detect_destructive_command("nice -n 10 rm old.log"));
        assert!(detect_destructive_command(
            "/usr/bin/time -f '%E' shred secret.txt"
        ));
        assert!(detect_destructive_command(
            "timeout --signal=KILL 5 sh -c 'rm -rf /tmp/cache'"
        ));
        assert!(detect_destructive_command(
            "bash -lc 'git reset --hard HEAD'"
        ));
        assert!(detect_destructive_command("setsid --fork /sbin/reboot"));
    }

    #[test]
    fn detects_destructive_executable_substitutions() {
        assert!(detect_destructive_command("echo $(rm -rf /tmp/cache)"));
        assert!(detect_destructive_command(
            "result=\"$(git reset --hard HEAD)\""
        ));
        assert!(detect_destructive_command("echo `kill -9 1234`"));
        assert!(detect_destructive_command(
            "echo \"$(printf ready; chmod +x script.sh)\""
        ));
        assert!(detect_destructive_command("echo \"$(echo $(rm old.log))\""));
    }

    #[test]
    fn permits_non_destructive_commands_and_mentions() {
        assert!(!detect_destructive_command("cargo build --release"));
        assert!(!detect_destructive_command("ls -la"));
        assert!(!detect_destructive_command("git status"));
        assert!(!detect_destructive_command("git reset --soft HEAD~1"));
        assert!(!detect_destructive_command("find . -name '*.tmp' -print"));
        assert!(!detect_destructive_command("find . -name rm -print"));
        assert!(!detect_destructive_command("echo 'rm -rf /'"));
        assert!(!detect_destructive_command(
            "printf '%s\\n' 'mkfs.ext4 /dev/disk9'"
        ));
        assert!(!detect_destructive_command("man rm"));
        assert!(!detect_destructive_command("chmod --help"));
        assert!(!detect_destructive_command("nice -n 10 cargo check"));
        assert!(!detect_destructive_command("time -p git status"));
        assert!(!detect_destructive_command("timeout 5 cargo test"));
        assert!(!detect_destructive_command("setsid sleep 1"));
        assert!(!detect_destructive_command(
            "printf '%s\\n' kill | xargs echo"
        ));
        assert!(!detect_destructive_command("printf ready | xargs -r echo"));
        assert!(!detect_destructive_command("eval 'echo rm -rf /'"));
    }

    #[test]
    fn permits_quoted_or_escaped_substitution_mentions() {
        assert!(!detect_destructive_command("echo '$(rm -rf /)'"));
        assert!(!detect_destructive_command("echo '`rm -rf /`'"));
        assert!(!detect_destructive_command("echo \"\\$(rm -rf /)\""));
        assert!(!detect_destructive_command("echo \"\\`rm -rf /\\`\""));
        assert!(!detect_destructive_command(
            "echo \"$(printf '%s' 'rm -rf /')\""
        ));
        assert!(!detect_destructive_command("echo ':(){ :|:& };:'"));
    }

    #[test]
    fn detects_fork_bombs() {
        assert!(detect_destructive_command(":(){ :|:& };:"));
        assert!(detect_destructive_command(": () { : | : & }; :"));
    }

    #[test]
    fn test_pythia_response_construction() {
        let resp = PythiaResponse::new(Some("rm -rf /".to_string()), "Deletes everything", 0.95);
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
