use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const ENGLISH_TOKEN_SEPARATORS: [char; 15] = [
    ' ', '?', '\'', '"', ':', ',', '.', '-', '!', ';', '(', ')', '[', ']', '\u{2033}',
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontKind {
    Chinese2500,
    English,
    Chinese3500,
    Blank,
    Chinese6900,
    Korean,
    TraditionalChinese3500,
    Other(i32),
}

impl FontKind {
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Chinese2500,
            1 => Self::English,
            2 => Self::Chinese3500,
            3 => Self::Blank,
            4 => Self::Chinese6900,
            5 => Self::Korean,
            6 => Self::TraditionalChinese3500,
            other => Self::Other(other),
        }
    }

    pub fn to_raw(self) -> i32 {
        match self {
            Self::Chinese2500 => 0,
            Self::English => 1,
            Self::Chinese3500 => 2,
            Self::Blank => 3,
            Self::Chinese6900 => 4,
            Self::Korean => 5,
            Self::TraditionalChinese3500 => 6,
            Self::Other(v) => v,
        }
    }

    pub fn is_word_based(self) -> bool {
        matches!(self, Self::English)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMeta {
    pub version: i32,
    pub kind: FontKind,
    pub name: String,
    pub author: String,
    pub description: String,
    pub internal_name: String,
    pub size: i32,
    pub glyph_count: i32,
    pub vendor: Option<String>,
    pub password: Option<String>,
    pub uuid: Option<String>,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphPathChunk {
    pub points: Vec<f32>,
    pub verbs: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphData {
    pub key: String,
    pub chunks: Vec<GlyphPathChunk>,
    pub cached: bool,
}

impl GlyphData {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            chunks: Vec::new(),
            cached: true,
        }
    }

    pub fn path_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn get_path(&self, index: usize) -> Option<&GlyphPathChunk> {
        self.chunks.get(index)
    }

    pub fn get_path_mut(&mut self, index: usize) -> Option<&mut GlyphPathChunk> {
        self.chunks.get_mut(index)
    }

    pub fn push_path(&mut self, chunk: GlyphPathChunk) {
        self.chunks.push(chunk);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GfontFile {
    pub meta: FontMeta,
    pub glyphs: HashMap<String, GlyphData>,
    pub zip_blob: Option<Vec<u8>>,
}

impl GfontFile {
    pub fn version(&self) -> i32 {
        self.meta.version
    }

    pub fn list_glyph_keys(&self) -> Vec<String> {
        self.glyphs.keys().cloned().collect()
    }

    pub fn has_glyph(&self, key: &str) -> bool {
        self.glyphs.contains_key(key)
    }

    pub fn get_glyph(&self, key: &str) -> Option<&GlyphData> {
        self.glyphs.get(key)
    }

    pub fn get_glyph_mut(&mut self, key: &str) -> Option<&mut GlyphData> {
        self.glyphs.get_mut(key)
    }

    pub fn insert_glyph(&mut self, glyph: GlyphData) {
        self.glyphs.insert(glyph.key.clone(), glyph);
        self.meta.glyph_count = self.glyphs.len() as i32;
    }

    pub fn set_password(&mut self, password: Option<String>) {
        self.meta.password = password;
    }

    pub fn tokenize_text(&self, text: &str) -> Vec<String> {
        tokenize_text_for_kind(self.meta.kind, text)
    }

    pub fn add_missing_tokens_from_text(&mut self, text: &str) {
        for token in self.missing_tokens(text) {
            self.insert_glyph(GlyphData::new(token));
        }
    }

    pub fn missing_tokens(&self, text: &str) -> Vec<String> {
        self.tokenize_text(text)
            .into_iter()
            .filter(|token| !self.has_glyph(token))
            .collect()
    }
}

pub fn tokenize_text_for_kind(kind: FontKind, text: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    if kind.is_word_based() {
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();
        for (index, ch) in chars.iter().copied().enumerate() {
            if matches!(ch, '\n' | '\r' | '\t' | '\u{000C}') {
                continue;
            }

            if !is_english_separator(ch) {
                current.push(ch);
            }

            if is_english_separator(ch) || index == chars.len().saturating_sub(1) {
                if !current.is_empty() {
                    push_unique(&mut tokens, &current);
                } else if ch != ' ' {
                    let punctuation = ch.to_string();
                    push_unique(&mut tokens, &punctuation);
                }
                current.clear();
            }
        }

        return tokens;
    }

    for ch in text.chars() {
        if matches!(ch, '\n' | '\r' | '\t' | '\u{000C}' | ' ') {
            continue;
        }
        let token = ch.to_string();
        push_unique(&mut tokens, &token);
    }

    tokens
}

fn is_english_separator(ch: char) -> bool {
    ENGLISH_TOKEN_SEPARATORS.contains(&ch)
}

fn push_unique(tokens: &mut Vec<String>, value: &str) {
    if !tokens.iter().any(|existing| existing == value) {
        tokens.push(value.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GfontCompatibility {
    V1To4Plain,
    V5To8Encrypted,
    V9Encrypted,
}
