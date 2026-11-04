//! Reading numbers out of bytes from a file, where any offset may be wrong: a read past the end is
//! `None`, never a panic.

pub fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([*b.first()?, *b.get(1)?]))
}

pub fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]))
}

pub fn i32_at(d: &[u8], at: usize) -> Option<i32> {
    u32_at(d, at).map(|v| v as i32)
}

pub fn u64_at(d: &[u8], at: usize) -> Option<u64> {
    let lo = u32_at(d, at)?;
    let hi = u32_at(d, at.checked_add(4)?)?;
    Some(u64::from(lo) | (u64::from(hi) << 32))
}

/// UTF-16 code units from little-endian bytes (a stray last byte is left out).
pub fn utf16_units(d: &[u8]) -> Vec<u16> {
    d.chunks_exact(2).filter_map(|c| u16_at(c, 0)).collect()
}
