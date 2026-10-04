//! Shift_JIS, read the way the WHATWG Encoding Standard's decoder reads it (DESIGN 1.5).
//!
//! The copy of a holiday table is kept as the bytes its publisher serves, so that anyone can
//! check a pin with `curl <url> | shasum -a 256`. koyomi reads those bytes itself, with the
//! table in `sjis_table.rs`. A byte sequence it cannot read is an error with its position,
//! never a replacement character: a holiday whose name came out wrong would be shown on the
//! page for people as if it were right.

use std::sync::OnceLock;

fn table() -> &'static [char] {
    static T: OnceLock<Vec<char>> = OnceLock::new();
    T.get_or_init(|| {
        let v: Vec<char> = crate::sjis_table::TABLE.chars().collect();
        assert_eq!(v.len(), crate::sjis_table::POINTERS);
        v
    })
}

/// Where the bytes stop being Shift_JIS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadByte {
    /// The offset of the first byte that could not be read.
    pub offset: usize,
    /// That byte and, for a two-byte sequence, the one after it.
    pub bytes: Vec<u8>,
}

/// The character a pointer of index jis0208 decodes to, as the Shift_JIS decoder uses it.
pub fn pointer_char(pointer: usize) -> Option<char> {
    if (8836..=10715).contains(&pointer) {
        // The decoder maps this stretch to the Private Use Area without the index.
        return char::from_u32(0xE000 + (pointer - 8836) as u32);
    }
    match table().get(pointer) {
        Some('\u{ffff}') | None => None,
        Some(c) => Some(*c),
    }
}

/// The two bytes that encode a pointer, the inverse of the decoder's arithmetic.
pub fn pointer_bytes(pointer: usize) -> (u8, u8) {
    let lead = pointer / 188;
    let trail = pointer % 188;
    let lead = if lead < 0x1F { lead + 0x81 } else { lead + 0xC1 };
    let trail = if trail < 0x3F { trail + 0x40 } else { trail + 0x41 };
    (lead as u8, trail as u8)
}

/// The text the bytes encode.
pub fn decode(bytes: &[u8]) -> Result<String, BadByte> {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            0x00..=0x80 => {
                out.push(b as char);
                i += 1;
            }
            0xA1..=0xDF => {
                out.push(char::from_u32(0xFF61 + (b - 0xA1) as u32).unwrap());
                i += 1;
            }
            0x81..=0x9F | 0xE0..=0xFC => {
                let Some(&t) = bytes.get(i + 1) else {
                    return Err(BadByte { offset: i, bytes: vec![b] });
                };
                let ok_trail = matches!(t, 0x40..=0x7E | 0x80..=0xFC);
                let c = if ok_trail {
                    let lead_off = if b < 0xA0 { 0x81 } else { 0xC1 };
                    let trail_off = if t < 0x7F { 0x40 } else { 0x41 };
                    pointer_char((b as usize - lead_off) * 188 + t as usize - trail_off)
                } else {
                    None
                };
                match c {
                    Some(c) => out.push(c),
                    None => return Err(BadByte { offset: i, bytes: vec![b, t] }),
                }
                i += 2;
            }
            _ => return Err(BadByte { offset: i, bytes: vec![b] }),
        }
    }
    Ok(out)
}
