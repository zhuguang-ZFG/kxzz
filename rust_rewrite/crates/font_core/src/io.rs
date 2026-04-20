use anyhow::{bail, Result};
use std::io::{Read, Write};

pub fn read_i32<R: Read>(r: &mut R) -> Result<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_be_bytes(buf))
}

pub fn read_f32<R: Read>(r: &mut R) -> Result<f32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_bits(u32::from_be_bytes(buf)))
}

pub fn read_u16<R: Read>(r: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

pub fn read_utf<R: Read>(r: &mut R) -> Result<String> {
    let len = read_u16(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

pub fn split_preview_and_zip(payload: &[u8]) -> Result<(&[u8], &[u8])> {
    let marker = b"PK\x03\x04";
    for i in 0..payload.len().saturating_sub(marker.len()) {
        if &payload[i..i + marker.len()] == marker {
            return Ok((&payload[..i], &payload[i..]));
        }
    }
    bail!("zip marker not found in gfont payload")
}

pub fn write_i32<W: Write>(w: &mut W, value: i32) -> Result<()> {
    w.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_f32<W: Write>(w: &mut W, value: f32) -> Result<()> {
    w.write_all(&value.to_bits().to_be_bytes())?;
    Ok(())
}

pub fn write_u16<W: Write>(w: &mut W, value: u16) -> Result<()> {
    w.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_utf<W: Write>(w: &mut W, value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let len: u16 = bytes
        .len()
        .try_into()
        .map_err(|_| anyhow::anyhow!("utf string too long"))?;
    write_u16(w, len)?;
    w.write_all(bytes)?;
    Ok(())
}
