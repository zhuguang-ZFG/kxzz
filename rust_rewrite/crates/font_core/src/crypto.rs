use crate::error::FontCoreError;
use anyhow::{anyhow, Result};

const HEADER_KEY: [u32; 4] = [0x0109_0809, 0x0008_0206, 0x0109_0902, 0x0008_0208];
const DELTA: u32 = 0x9E37_79B9;
const HEADER_RANDOM_PREFIX_BYTES: usize = 2;
const HEADER_ZERO_SUFFIX_BYTES: usize = 7;

pub fn decrypt_header(ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < 16 || ciphertext.len() % 8 != 0 {
        return Err(FontCoreError::EncryptedHeader(format!(
            "invalid encrypted header length {}",
            ciphertext.len()
        ))
        .into());
    }

    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev_plain = [0u8; 8];
    let mut prev_cipher = [0u8; 8];

    for block_bytes in ciphertext.chunks_exact(8) {
        let mut block = [0u8; 8];
        block.copy_from_slice(block_bytes);
        let tea = xor8(&block, &prev_plain);
        let decrypted = tea_decrypt_block(tea);
        let plain = xor8(&decrypted, &prev_cipher);
        out.extend_from_slice(&plain);
        prev_plain = tea;
        prev_cipher = block;
    }

    let pad_len = (out[0] & 0x07) as usize;
    let skip = 1 + pad_len + HEADER_RANDOM_PREFIX_BYTES;
    if skip > out.len() {
        return Err(FontCoreError::EncryptedHeader("header prefix is truncated".into()).into());
    }
    if out.len() < skip + HEADER_ZERO_SUFFIX_BYTES {
        return Err(FontCoreError::EncryptedHeader("header body is truncated".into()).into());
    }

    let end = out.len() - HEADER_ZERO_SUFFIX_BYTES;
    if out[end..].iter().any(|&b| b != 0) {
        return Err(FontCoreError::EncryptedHeader("header trailer check failed".into()).into());
    }

    Ok(out[skip..end].to_vec())
}

pub fn encrypt_header(plaintext: &[u8]) -> Vec<u8> {
    let padding = (8 - ((plaintext.len() + 10) % 8)) % 8;
    let mut body = Vec::with_capacity(plaintext.len() + padding + 10);
    body.push(0x20 | padding as u8);
    body.extend(std::iter::repeat(0u8).take(padding));
    body.extend(std::iter::repeat(0u8).take(HEADER_RANDOM_PREFIX_BYTES));
    body.extend_from_slice(plaintext);
    body.extend(std::iter::repeat(0u8).take(HEADER_ZERO_SUFFIX_BYTES));

    let mut out = Vec::with_capacity(body.len());
    let mut prev_plain = [0u8; 8];
    let mut prev_cipher = [0u8; 8];

    for chunk in body.chunks_exact(8) {
        let mut block = [0u8; 8];
        block.copy_from_slice(chunk);
        let xored = xor8(&block, &prev_cipher);
        let encrypted = tea_encrypt_block(xored);
        let cipher = xor8(&encrypted, &prev_plain);
        out.extend_from_slice(&cipher);
        prev_plain = xored;
        prev_cipher = cipher;
    }

    out
}

fn xor8(a: &[u8; 8], b: &[u8; 8]) -> [u8; 8] {
    let mut out = [0u8; 8];
    for i in 0..8 {
        out[i] = a[i] ^ b[i];
    }
    out
}

fn tea_encrypt_block(block: [u8; 8]) -> [u8; 8] {
    let mut y = u32::from_be_bytes(block[0..4].try_into().expect("4 bytes"));
    let mut z = u32::from_be_bytes(block[4..8].try_into().expect("4 bytes"));
    let mut sum = 0u32;

    for _ in 0..16 {
        sum = sum.wrapping_add(DELTA);
        y = y.wrapping_add(((z << 4).wrapping_add(HEADER_KEY[0])) ^ z.wrapping_add(sum) ^ ((z >> 5).wrapping_add(HEADER_KEY[1])));
        z = z.wrapping_add(((y << 4).wrapping_add(HEADER_KEY[2])) ^ y.wrapping_add(sum) ^ ((y >> 5).wrapping_add(HEADER_KEY[3])));
    }

    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&y.to_be_bytes());
    out[4..].copy_from_slice(&z.to_be_bytes());
    out
}

fn tea_decrypt_block(block: [u8; 8]) -> [u8; 8] {
    let mut y = u32::from_be_bytes(block[0..4].try_into().expect("4 bytes"));
    let mut z = u32::from_be_bytes(block[4..8].try_into().expect("4 bytes"));
    let mut sum = DELTA.wrapping_shl(4);

    for _ in 0..16 {
        z = z.wrapping_sub(((y << 4).wrapping_add(HEADER_KEY[2])) ^ y.wrapping_add(sum) ^ ((y >> 5).wrapping_add(HEADER_KEY[3])));
        y = y.wrapping_sub(((z << 4).wrapping_add(HEADER_KEY[0])) ^ z.wrapping_add(sum) ^ ((z >> 5).wrapping_add(HEADER_KEY[1])));
        sum = sum.wrapping_sub(DELTA);
    }

    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&y.to_be_bytes());
    out[4..].copy_from_slice(&z.to_be_bytes());
    out
}

#[allow(dead_code)]
fn _roundtrip_smoke(bytes: &[u8]) -> Result<()> {
    let encrypted = encrypt_header(bytes);
    let decrypted = decrypt_header(&encrypted)?;
    if decrypted != bytes {
        return Err(anyhow!("header crypto roundtrip mismatch"));
    }
    Ok(())
}
