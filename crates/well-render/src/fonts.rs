//! System font discovery shared by renderer rebuilds. Font data is read into owned memory.
use ab_glyph::{Font, FontArc, FontRef, FontVec};
use fontdb::{Database, Family, Query, Style, Weight, ID};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct FontCatalog(Arc<Database>);
impl Default for FontCatalog {
    fn default() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();
        let family = db
            .faces()
            .find(|f| f.monospaced)
            .and_then(|f| f.families.first())
            .map(|f| f.0.clone());
        if let Some(family) = family {
            db.set_monospace_family(family);
        }
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
        let families = [if family == "monospace" {
            Family::Monospace
        } else {
            Family::Name(family)
        }];
        let mut primary = Vec::new();
        let mut loaded = HashMap::new();
        for style in 0..4 {
            let id = db.query(&Query {
                families: &families,
                weight: if style & 1 != 0 { Weight::BOLD } else { Weight::NORMAL },
                style: if style & 2 != 0 { Style::Italic } else { Style::Normal },
                ..Query::default()
            }).ok_or_else(|| format!("Font family '{family}' is unavailable. Choose an installed monospace family (Arch: install ttf-dejavu)."))?;
            if !db.face(id).is_some_and(|f| f.monospaced) {
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
            if !self.loaded.contains_key(&id) {
                if let Some(font) = load(db, id) {
                    self.loaded.insert(id, font);
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
