//! Background incremental Tree-sitter parser and Safety AST Validator.
//! Runs real-time syntax tree compilation, path verification (green for valid paths),
//! unclosed delimiter/bracket detection (red for unclosed blocks), and destructive command filtering.

use crate::mneme::{DiagnosticSeverity, HighlightSpan, LspDiagnostic, SyntaxClass};
use std::env;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tree_sitter::{Node, Parser, Tree};

/// Request sent from the editor to the background AST validator.
#[derive(Debug, Clone)]
pub struct ValidationRequest {
    pub version: u64,
    pub text: String,
}

/// Result produced by the background AST validator.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub version: u64,
    pub has_syntax_error: bool,
    pub is_unclosed: bool,
    pub unclosed_construct: Option<String>,
    pub destructive_warning: Option<String>,
    pub spans: Vec<HighlightSpan>,
    pub diagnostics: Vec<LspDiagnostic>,
}

pub struct SafetyAstValidator {
    request_tx: Option<Sender<ValidationRequest>>,
    result_rx: Receiver<ValidationResult>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    latest_result: Option<ValidationResult>,
}

impl Default for SafetyAstValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyAstValidator {
    pub fn new() -> Self {
        let (request_tx, request_rx) = channel::<ValidationRequest>();
        let (result_tx, result_rx) = channel::<ValidationResult>();
        let running = Arc::new(AtomicBool::new(true));

        let run_clone = running.clone();
        let handle = thread::Builder::new()
            .name("mneme-ast-validator".to_string())
            .spawn(move || {
                let mut parser = Parser::new();
                if parser
                    .set_language(&tree_sitter_bash::LANGUAGE.into())
                    .is_err()
                {
                    return;
                }

                while run_clone.load(Ordering::Relaxed) {
                    // Receive newest request, draining stale queued requests
                    let mut request =
                        match request_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                            Ok(req) => req,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                    while let Ok(newer) = request_rx.try_recv() {
                        request = newer;
                    }

                    let parsed = parser.parse(&request.text, None);
                    if let Some(tree) = parsed {
                        let outcome = analyze_syntax_tree(&tree, &request.text, request.version);
                        if result_tx.send(outcome).is_err() {
                            break;
                        }
                    }
                }
            })
            .ok();

        Self {
            request_tx: Some(request_tx),
            result_rx,
            running,
            handle,
            latest_result: None,
        }
    }

    /// Submit a buffer string for background incremental AST validation.
    pub fn submit(&self, version: u64, text: String) {
        if let Some(tx) = &self.request_tx {
            let _ = tx.send(ValidationRequest { version, text });
        }
    }

    /// Poll for newly completed validation results from the background thread.
    pub fn poll_result(&mut self) -> Option<&ValidationResult> {
        while let Ok(result) = self.result_rx.try_recv() {
            self.latest_result = Some(result);
        }
        self.latest_result.as_ref()
    }

    /// Synchronously check if a text block contains an unclosed construct (for inline prompt expansion).
    pub fn check_unclosed_sync(text: &str) -> (bool, Option<String>) {
        let mut parser = Parser::new();
        if parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .is_err()
        {
            return (false, None);
        }
        if let Some(tree) = parser.parse(text, None) {
            let root = tree.root_node();
            let mut unclosed_reason = None;
            let unclosed = inspect_unclosed_nodes(root, text, &mut unclosed_reason);
            (unclosed, unclosed_reason)
        } else {
            (false, None)
        }
    }
}

impl Drop for SafetyAstValidator {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        drop(self.request_tx.take());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Analyze a Tree-sitter Bash syntax tree for highlights, path validity, unclosed blocks, and safety gates.
pub fn analyze_syntax_tree(tree: &Tree, text: &str, version: u64) -> ValidationResult {
    let root = tree.root_node();
    let mut spans = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unclosed_construct = None;

    let is_unclosed = inspect_unclosed_nodes(root, text, &mut unclosed_construct);
    let destructive_warning = inspect_destructive_commands(root, text);

    if let Some(ref warn) = destructive_warning {
        diagnostics.push(LspDiagnostic {
            range: 0..text.chars().count().min(64),
            severity: DiagnosticSeverity::Warning,
            message: warn.clone(),
        });
    }

    // Traverse nodes to collect highlighting spans and validate paths
    collect_spans_and_paths(root, text, &mut spans, &mut diagnostics);

    ValidationResult {
        version,
        has_syntax_error: root.has_error(),
        is_unclosed,
        unclosed_construct,
        destructive_warning,
        spans,
        diagnostics,
    }
}

/// Recursively traverses the tree to find unclosed block keywords and missing brackets/quotes.
fn safe_str_slice(s: &str, start: usize, end: usize) -> &str {
    let len = s.len();
    if start >= len || start >= end {
        return "";
    }
    let mut safe_start = start;
    while safe_start < len && !s.is_char_boundary(safe_start) {
        safe_start += 1;
    }
    let mut safe_end = end.min(len);
    while safe_end > safe_start && !s.is_char_boundary(safe_end) {
        safe_end -= 1;
    }
    if safe_start >= safe_end {
        ""
    } else {
        &s[safe_start..safe_end]
    }
}

fn inspect_unclosed_nodes(node: Node<'_>, text: &str, reason: &mut Option<String>) -> bool {
    if !node.has_error() {
        return false;
    }

    if node.is_missing() {
        let kind = node.kind();
        *reason = Some(format!("Missing closing token: '{kind}'"));
        return true;
    }

    if node.is_error() {
        let byte_range = node.byte_range();
        let snippet = safe_str_slice(text, byte_range.start, byte_range.end);
        if snippet.contains("do") && !snippet.contains("done") {
            *reason = Some("Unclosed loop block: missing 'done'".into());
            return true;
        }
        if snippet.contains("then") && !snippet.contains("fi") {
            *reason = Some("Unclosed if condition: missing 'fi'".into());
            return true;
        }
        if snippet.contains('{') && !snippet.contains('}') {
            *reason = Some("Unclosed brace block: missing '}'".into());
            return true;
        }
        if snippet.contains('(') && !snippet.contains(')') {
            *reason = Some("Unclosed parenthesis: missing ')'".into());
            return true;
        }
    }

    // Check specific statement node kinds
    match node.kind() {
        "if_statement" => {
            if node.child_by_field_name("consequence").is_some() && !node_has_child_kind(node, "fi")
            {
                *reason = Some("Unclosed if statement: missing 'fi'".into());
                return true;
            }
        }
        "for_statement" | "while_statement" => {
            if node.child_by_field_name("body").is_none() || !node_has_child_kind(node, "done") {
                *reason = Some("Unclosed loop: missing 'done'".into());
                return true;
            }
        }
        "function_definition" => {
            if node.child_by_field_name("body").is_none() {
                *reason = Some("Unclosed function body".into());
                return true;
            }
        }
        "case_statement" if !node_has_child_kind(node, "esac") => {
            *reason = Some("Unclosed case statement: missing 'esac'".into());
            return true;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if inspect_unclosed_nodes(child, text, reason) {
            return true;
        }
    }
    false
}

fn node_has_child_kind(node: Node<'_>, expected_kind: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == expected_kind {
            return true;
        }
    }
    false
}

/// Inspects the syntax tree for destructive patterns:
/// e.g. rm -rf /, mkfs, raw disk redirection (> /dev/sd*), fork bombs.
#[derive(Clone, Copy, Debug)]
pub struct SafetyPolicy {
    pub root_removal: bool,
    pub cluster_delete: bool,
    pub force_push: bool,
    pub terraform_destroy: bool,
}
impl Default for SafetyPolicy {
    fn default() -> Self {
        Self {
            root_removal: true,
            cluster_delete: true,
            force_push: true,
            terraform_destroy: true,
        }
    }
}
/// Revalidate current text at execution; background results may describe an older edit.
pub fn check_destructive_sync(text: &str, policy: SafetyPolicy) -> Option<String> {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .is_err()
    {
        return Some("Safety parser unavailable".into());
    }
    let Some(tree) = parser.parse(text, None) else {
        return Some("Safety validation failed".into());
    };
    inspect_destructive_with_policy(tree.root_node(), text, policy)
}
fn inspect_destructive_commands(node: Node<'_>, text: &str) -> Option<String> {
    inspect_destructive_with_policy(node, text, SafetyPolicy::default())
}
fn inspect_destructive_with_policy(
    node: Node<'_>,
    text: &str,
    policy: SafetyPolicy,
) -> Option<String> {
    if node.kind() == "command" {
        let mut command_name = None;
        let mut args = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let start = child.start_byte();
            let end = child.end_byte();
            let snippet = safe_str_slice(text, start, end);

            if child.kind() == "command_name" {
                command_name = Some(snippet.trim());
            } else if !snippet.trim().is_empty() {
                args.push(snippet.trim());
            }
        }

        if let Some(cmd) = command_name {
            if cmd == "rm" && policy.root_removal {
                let has_recursive = args
                    .iter()
                    .any(|a| *a == "-r" || *a == "-rf" || *a == "-fr");
                let targets_root = args.iter().any(|a| {
                    *a == "/"
                        || *a == "/*"
                        || *a == "~"
                        || *a == "/System"
                        || *a == "/etc"
                        || *a == "/usr"
                });
                if has_recursive && targets_root {
                    return Some(
                        "⚠️ Destructive command blocked: recursive removal of root/system path"
                            .into(),
                    );
                }
            } else if cmd == "mkfs" || cmd.starts_with("mkfs.") {
                return Some(
                    "⚠️ Destructive command warning: filesystem formatting utility detected".into(),
                );
            } else if cmd == "dd" {
                let targets_disk = args.iter().any(|a| {
                    a.starts_with("of=/dev/sd")
                        || a.starts_with("of=/dev/disk")
                        || a.starts_with("of=/dev/nvme")
                });
                if targets_disk {
                    return Some(
                        "⚠️ Destructive command warning: dd write target is a raw block device"
                            .into(),
                    );
                }
            } else if cmd == "kubectl" && policy.cluster_delete {
                let is_delete = args.iter().any(|a| a.contains("delete"));
                let has_all = args.iter().any(|a| a.contains("--all") || a.contains("-A"));
                let targets_critical = args.iter().any(|a| {
                    *a == "namespace"
                        || *a == "namespaces"
                        || *a == "ns"
                        || *a == "node"
                        || *a == "nodes"
                });
                if is_delete && (has_all || targets_critical) {
                    return Some("⚠️ Destructive command warning: bulk or critical cluster resource deletion via kubectl".into());
                }
            } else if cmd == "git" {
                let is_push = args.contains(&"push");
                let is_force = args
                    .iter()
                    .any(|a| *a == "--force" || *a == "-f" || a.starts_with('+'));
                let targets_main = args.iter().any(|a| {
                    *a == "main" || *a == "master" || *a == "production" || *a == "release"
                });
                if is_push && is_force && targets_main && policy.force_push {
                    return Some(
                        "⚠️ Destructive command warning: force push to production branch detected"
                            .into(),
                    );
                }
                let is_reset = args.contains(&"reset");
                let is_hard = args.contains(&"--hard");
                if is_reset && is_hard {
                    return Some("⚠️ Destructive command warning: git reset --hard will discard uncommitted changes".into());
                }
            } else if cmd == "terraform" && policy.terraform_destroy {
                let is_destroy = args.contains(&"destroy");
                let auto_approve = args
                    .iter()
                    .any(|a| *a == "--auto-approve" || *a == "-auto-approve");
                if is_destroy && auto_approve {
                    return Some("⚠️ Destructive command warning: unattended terraform destroy with auto-approve detected".into());
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(warn) = inspect_destructive_with_policy(child, text, policy) {
            return Some(warn);
        }
    }
    None
}

/// Traverses nodes, categorizes tokens, checks filesystem paths, and produces highlight spans.
fn collect_spans_and_paths(
    node: Node<'_>,
    text: &str,
    spans: &mut Vec<HighlightSpan>,
    diagnostics: &mut Vec<LspDiagnostic>,
) {
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();

    if start_byte >= end_byte || start_byte >= text.len() {
        return;
    }

    if node.is_missing() {
        spans.push(HighlightSpan {
            start_byte,
            end_byte,
            kind: SyntaxClass::UnclosedDelimiter,
        });
        diagnostics.push(LspDiagnostic {
            range: char_range_from_bytes(text, start_byte, end_byte),
            severity: DiagnosticSeverity::Error,
            message: format!("Missing required token: '{}'", node.kind()),
        });
        return;
    }

    if node.is_error() {
        spans.push(HighlightSpan {
            start_byte,
            end_byte,
            kind: SyntaxClass::InvalidSyntax,
        });
        diagnostics.push(LspDiagnostic {
            range: char_range_from_bytes(text, start_byte, end_byte),
            severity: DiagnosticSeverity::Error,
            message: "Syntax error".into(),
        });
        return;
    }

    match node.kind() {
        "command_name" => {
            let name = safe_str_slice(text, start_byte, end_byte).trim();
            if is_shell_builtin(name) {
                spans.push(HighlightSpan {
                    start_byte,
                    end_byte,
                    kind: SyntaxClass::Builtin,
                });
            } else if is_valid_command_or_path(name) {
                spans.push(HighlightSpan {
                    start_byte,
                    end_byte,
                    kind: SyntaxClass::ValidPath,
                });
            } else {
                spans.push(HighlightSpan {
                    start_byte,
                    end_byte,
                    kind: SyntaxClass::Command,
                });
            }
            return;
        }
        "number" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::Number,
            });
            return;
        }
        "word" => {
            let snippet = safe_str_slice(text, start_byte, end_byte).trim();
            if looks_like_path(snippet) {
                if check_path_exists(snippet) {
                    spans.push(HighlightSpan {
                        start_byte,
                        end_byte,
                        kind: SyntaxClass::ValidPath,
                    });
                } else {
                    spans.push(HighlightSpan {
                        start_byte,
                        end_byte,
                        kind: SyntaxClass::InvalidPath,
                    });
                }
                return;
            } else if snippet.starts_with('-') {
                spans.push(HighlightSpan {
                    start_byte,
                    end_byte,
                    kind: SyntaxClass::Parameter,
                });
                return;
            } else if snippet
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '_')
                && snippet.chars().any(|c| c.is_ascii_digit())
            {
                spans.push(HighlightSpan {
                    start_byte,
                    end_byte,
                    kind: SyntaxClass::Number,
                });
                return;
            }
        }
        "string" | "raw_string" | "ansi_c_string" | "heredoc_body" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::String,
            });
            return;
        }
        "comment" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::Comment,
            });
            return;
        }
        "variable_name" | "special_variable_name" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::Variable,
            });
            return;
        }
        "if" | "then" | "else" | "elif" | "fi" | "for" | "do" | "done" | "while" | "until"
        | "case" | "esac" | "function" | "in" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::Keyword,
            });
            return;
        }
        "|" | "||" | "&&" | ">" | ">>" | "<" | "&" | "=" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::Operator,
            });
            return;
        }
        "(" | ")" | "{" | "}" | "[" | "]" | "[[" | "]]" | ";" => {
            spans.push(HighlightSpan {
                start_byte,
                end_byte,
                kind: SyntaxClass::Delimiter,
            });
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_spans_and_paths(child, text, spans, diagnostics);
    }
}

fn char_range_from_bytes(text: &str, start_byte: usize, end_byte: usize) -> std::ops::Range<usize> {
    let start_char = safe_str_slice(text, 0, start_byte).chars().count();
    let end_char = safe_str_slice(text, 0, end_byte).chars().count();
    start_char..end_char
}

fn looks_like_path(val: &str) -> bool {
    val.starts_with('/')
        || val.starts_with("./")
        || val.starts_with("../")
        || val.starts_with("~/")
        || (val.contains('/') && !val.starts_with('-'))
}

fn check_path_exists(raw_path: &str) -> bool {
    let resolved = if let Some(stripped) = raw_path.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            Path::new(&home).join(stripped)
        } else {
            Path::new(raw_path).to_path_buf()
        }
    } else {
        Path::new(raw_path).to_path_buf()
    };
    resolved.exists()
}

fn is_shell_builtin(cmd: &str) -> bool {
    matches!(
        cmd,
        "cd" | "echo"
            | "pwd"
            | "export"
            | "set"
            | "unset"
            | "exit"
            | "history"
            | "help"
            | "clear"
            | "printf"
            | "alias"
            | "unalias"
            | "source"
            | "eval"
            | "read"
            | "test"
            | "true"
            | "false"
            | "type"
            | "exec"
            | "local"
            | "return"
            | "shift"
            | "trap"
            | "wait"
    )
}

fn is_valid_command_or_path(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }
    // Shell builtins
    if is_shell_builtin(cmd) {
        return true;
    }
    if looks_like_path(cmd) {
        return check_path_exists(cmd);
    }
    // Check PATH
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let full = dir.join(cmd);
            if full.is_file() {
                return true;
            }
        }
    }
    false
}
