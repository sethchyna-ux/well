//! Syntax-aware inline composition editing.
pub mod mneme;
pub use mneme::{
    DiagnosticSeverity, HighlightSpan, LspDiagnostic, MnemeEditMode, MnemeEditor, SyntaxClass,
    WellInputMode,
};
pub fn init_editor(language: Option<tree_sitter::Language>) -> MnemeEditor {
    MnemeEditor::new(language)
}
