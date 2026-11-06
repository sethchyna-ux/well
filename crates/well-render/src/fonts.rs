//! System font discovery shared by renderer rebuilds. Font data is read into owned memory.
use ab_glyph::{Font, FontArc, FontRef, FontVec};
use fontdb::{Database, Family, Query, Style, Weight, ID};
use std::collections::HashMap;
use std::sync::Arc;

pub const EMBEDDED_OPENDYSLEXIC_MONO: &[u8] =
    include_bytes!("../assets/fonts/OpenDyslexicMNerdFontMono-Regular.otf");
pub const EMBEDDED_OPENDYSLEXIC_M_REG: &[u8] =
    include_bytes!("../assets/fonts/OpenDyslexicMNerdFont-Regular.otf");
pub const EMBEDDED_OPENDYSLEXIC_REG: &[u8] =
    include_bytes!("../assets/fonts/OpenDyslexicNerdFont-Regular.otf");
pub const EMBEDDED_OPENDYSLEXIC_BOLD: &[u8] =
    include_bytes!("../assets/fonts/OpenDyslexicNerdFont-Bold.otf");
pub const EMBEDDED_OPENDYSLEXIC_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/OpenDyslexicNerdFont-Italic.otf");
pub const EMBEDDED_OPENDYSLEXIC_BOLD_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/OpenDyslexicNerdFont-BoldItalic.otf");

/// Return all embedded OpenDyslexic font assets as static borrowed byte slices
/// suitable for direct registration in GPUI `cx.text_system().add_fonts(...)`.
pub fn embedded_opendyslexic_fonts() -> Vec<std::borrow::Cow<'static, [u8]>> {
    vec![
        std::borrow::Cow::Borrowed(EMBEDDED_OPENDYSLEXIC_MONO),
        std::borrow::Cow::Borrowed(EMBEDDED_OPENDYSLEXIC_M_REG),
        std::borrow::Cow::Borrowed(EMBEDDED_OPENDYSLEXIC_REG),
        std::borrow::Cow::Borrowed(EMBEDDED_OPENDYSLEXIC_BOLD),
        std::borrow::Cow::Borrowed(EMBEDDED_OPENDYSLEXIC_ITALIC),
        std::borrow::Cow::Borrowed(EMBEDDED_OPENDYSLEXIC_BOLD_ITALIC),
    ]
}

/// Normalizes font family names and aliases to canonical names recognized by GPUI and CoreText.
pub fn canonical_font_family(family: &str) -> &'static str {
    let lower = family.to_ascii_lowercase();
    if (lower.contains("mono") || lower.contains("mnerd")) && (lower.contains("opendyslexic") || lower.contains("dyslexic")) {
        "OpenDyslexicM Nerd Font Mono"
    } else if lower.contains("opendyslexic") || lower.contains("dyslexic") {
        "OpenDyslexic Nerd Font"
    } else if lower.contains("jetbrains") {
        "JetBrains Mono"
    } else if lower.contains("sf mono") || lower == "sfmono" {
        "SF Mono"
    } else if lower.contains("monaco") {
        "Monaco"
    } else if lower.contains("menlo") {
        "Menlo"
    } else {
        "OpenDyslexic Nerd Font"
    }
}

#[derive(Clone)]
pub struct FontCatalog(Arc<Database>);
impl Default for FontCatalog {
    fn default() -> Self {
        let mut db = Database::new();
        db.load_font_data(EMBEDDED_OPENDYSLEXIC_MONO.to_vec());
        db.load_font_data(EMBEDDED_OPENDYSLEXIC_M_REG.to_vec());
        db.load_font_data(EMBEDDED_OPENDYSLEXIC_REG.to_vec());
        db.load_font_data(EMBEDDED_OPENDYSLEXIC_BOLD.to_vec());
        db.load_font_data(EMBEDDED_OPENDYSLEXIC_ITALIC.to_vec());
        db.load_font_data(EMBEDDED_OPENDYSLEXIC_BOLD_ITALIC.to_vec());
        db.load_system_fonts();
        db.set_monospace_family("OpenDyslexicM Nerd Font Mono".to_string());
        Self(Arc::new(db))
    }
}

pub(crate) struct FontSet {
    catalog: FontCatalog,
    pub primary: Vec<FontArc>,
    loaded: HashMap<ID, FontArc>,
    resolved: HashMap<(char, u8), Option<ID>>,
}
impl FontSet {
    pub fn new(catalog: FontCatalog, family: &str) -> Result<Self, String> {
        let db = &catalog.0;
        let is_opendyslexic = family.eq_ignore_ascii_case("OpenDyslexic Nerd Font")
            || family.eq_ignore_ascii_case("OpenDyslexic")
            || family.eq_ignore_ascii_case("OpenDyslexicMono Nerd Font")
            || family.eq_ignore_ascii_case("OpenDyslexicM Nerd Font Mono")
            || family.contains("OpenDyslexic");

        let families = if is_opendyslexic {
            vec![
                Family::Name("OpenDyslexicM Nerd Font Mono"),
                Family::Name("OpenDyslexic Nerd Font"),
                Family::Monospace,
            ]
        } else if family == "monospace" {
            vec![Family::Monospace]
        } else {
            vec![Family::Name(family), Family::Monospace]
        };

        let mut primary = Vec::new();
        let mut loaded = HashMap::new();
        for style in 0..4 {
            let id = db.query(&Query {
                families: &families,
                weight: if style & 1 != 0 { Weight::BOLD } else { Weight::NORMAL },
                style: if style & 2 != 0 { Style::Italic } else { Style::Normal },
                ..Query::default()
            }).ok_or_else(|| format!("Font family '{family}' is unavailable. Choose an installed monospace family (Arch: install ttf-dejavu)."))?;
            if !db.face(id).is_some_and(|f| f.monospaced || is_opendyslexic) {
                return Err(format!("Font family '{family}' is not monospaced"));
            }
            let font = load(db, id).ok_or_else(|| format!("Cannot read font family '{family}'"))?;
            primary.push(font.clone());
            loaded.insert(id, font);
        }
        Ok(Self {
            catalog,
            primary,
            loaded,
            resolved: HashMap::new(),
        })
    }
    pub fn font_for(&mut self, ch: char, style: u8) -> FontArc {
        let primary = &self.primary[style as usize];
        if supports(primary, ch) {
            return primary.clone();
        }
        let db = &self.catalog.0;
        let id = *self.resolved.entry((ch, style)).or_insert_with(|| {
            // Prefer matching weight/style and monospaced faces. Only parse borrowed
            // data while searching; retain owned bytes for fonts that are actually used.
            let mut faces: Vec<_> = db.faces().collect();
            faces.sort_by_key(|f| {
                (
                    !f.monospaced,
                    (f.weight.0 as i32 - if style & 1 != 0 { 700 } else { 400 }).abs(),
                    (f.style
                        != if style & 2 != 0 {
                            Style::Italic
                        } else {
                            Style::Normal
                        }),
                )
            });
            faces
                .into_iter()
                .find(|f| {
                    db.with_face_data(f.id, |data, index| {
                        FontRef::try_from_slice_and_index(data, index)
                            .is_ok_and(|font| supports(&font, ch))
                    })
                    .unwrap_or(false)
                })
                .map(|f| f.id)
        });
        if let Some(id) = id {
            if let std::collections::hash_map::Entry::Vacant(e) = self.loaded.entry(id) {
                if let Some(font) = load(db, id) {
                    e.insert(font);
                }
            }
            if let Some(font) = self.loaded.get(&id) {
                return font.clone();
            }
        }
        primary.clone()
    }
}
fn supports(font: &impl Font, ch: char) -> bool {
    let id = font.glyph_id(ch);
    id.0 != 0 && (ch.is_whitespace() || font.outline(id).is_some())
}
fn load(db: &Database, id: ID) -> Option<FontArc> {
    db.with_face_data(id, |data, index| {
        FontVec::try_from_vec_and_index(data.to_vec(), index)
            .ok()
            .map(FontArc::new)
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opendyslexic_metadata() {
        let mut db = Database::new();
        let bytes = include_bytes!("../assets/fonts/OpenDyslexicMNerdFontMono-Regular.otf");
        db.load_font_data(bytes.to_vec());
        let bytes2 = include_bytes!("../assets/fonts/OpenDyslexicNerdFont-Regular.otf");
        db.load_font_data(bytes2.to_vec());

        for face in db.faces() {
            println!(
                "Face: post_script_name={:?}, families={:?}, monospaced={}",
                face.post_script_name, face.families, face.monospaced
            );
        }
    }
}
