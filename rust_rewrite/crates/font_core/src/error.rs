use thiserror::Error;

#[derive(Debug, Error)]
pub enum FontCoreError {
    #[error("unsupported gfont version: {0}")]
    UnsupportedVersion(i32),
    #[error("encrypted gfont header error: {0}")]
    EncryptedHeader(String),
    #[error("invalid glyph path data: {0}")]
    InvalidPath(String),
}
