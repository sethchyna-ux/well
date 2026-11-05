//! Syntax-aware inline composition editing with background Safety AST validation and LSP streaming.

pub mod lsp;
pub mod mneme;
pub mod snippets;
pub mod validator;

pub use lsp::LspChannel;
pub use mneme::{
    CursorMovement, DiagnosticSeverity, HighlightSpan, LspDiagnostic, MnemeEditMode, MnemeEditor,
    SyntaxClass, WellInputMode,
};
pub use snippets::{
    expand_snippet_template, ExpandedSnippet, ExpandedTabStop, SnippetScope, WellSnippet,
};
pub use validator::{SafetyAstValidator, ValidationRequest, ValidationResult};

pub fn init_editor(language: Option<tree_sitter::Language>) -> MnemeEditor {
    MnemeEditor::new(language)
}
