//! The zip of a bundle (`--extra bundle`, ringi's DESIGN 12.6), written one way, so that the same
//! files give the same bytes, and read in that way only.
//!
//! Written: one entry for each file, in the order of the bytes of their paths, and none for a
//! directory; each with the time 1980-01-01 00:00, the flag of a name in UTF-8 (bit 11) and no other,
//! made by Unix with the mode 0644, no extra field, no comment, and no data descriptor (the sizes and
//! the CRC-32 are in the local header). A file is compressed with deflate (method 8) in one block
//! of the fixed Huffman codes, its repeats found within the window of 32 KiB by a chain of hashes of
//! three bytes, the longest match of the first 64 candidates taken at each place; a file that would
//! not be smaller so is stored (method 0). No zip64: a file or the zip past 4 GiB, or more than
//! 65,535 files, is refused.
//!
//! Read: only that. A zip with a comment, a data descriptor, an extra field, a method other than 0
//! and 8, a deflate block other than a stored one or one of the fixed codes, a path that is not one
//! a module's tar holds (`ritsu_base::modules::tar::path_why`), a path twice, a CRC-32 or a size
//! that is not the file's, or anything past [`MAX`] is refused.

use ritsu_base::text::Text;
use ritsu_base::tr;
use std::collections::BTreeSet;

/// The most a zip read holds, its files uncompressed together.
pub const MAX: u64 = 512 * 1024 * 1024;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// 1980-01-01, as the DOS date of a zip writes it: ((1980 − 1980) << 9) | (1 << 5) | 1.
const DATE: u16 = 0x0021;
const UTF8: u16 = 1 << 11;
/// The version a reader needs (2.0, deflate), and the one that made it: Unix (3) and 2.0.
const NEEDED: u16 = 20;
const MADE_BY: u16 = (3 << 8) | 20;
/// A regular file, 0644, in the high half of the external attributes as Unix writes them.
const MODE: u32 = 0o100_644 << 16;

/// The zip of `files` (each by its path, `/` between the parts), in the order of the bytes of their
/// paths. Err with why it cannot be written.
pub fn write(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Text> {
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    if sorted.len() > usize::from(u16::MAX) {
        let n = sorted.len();
        return Err(tr!("ファイルが {n} 個あり、zip64 を使わない zip に入る数を超えます", "{n} files, more than a zip without zip64 holds"));
    }
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut seen = BTreeSet::new();
    for (path, bytes) in sorted {
        if !seen.insert(path.as_str()) {
            return Err(tr!("`{path}` が二度あります", "`{path}` twice"));
        }
        // a path the reader refuses is not written: none that goes up, none absolute
        if let Some(why) = ritsu_base::modules::tar::path_why(path) {
            return Err(path_error(path, why));
        }
        let crc = crc32(bytes);
        let packed = deflate(bytes);
        let (method, data): (u16, &[u8]) = if packed.len() < bytes.len() { (8, &packed) } else { (0, bytes) };
        let size = u32::try_from(bytes.len()).map_err(|_| tr!("`{path}` が 4 GiB を超えます", "`{path}` is past 4 GiB"))?;
        let csize = u32::try_from(data.len()).map_err(|_| tr!("`{path}` が 4 GiB を超えます", "`{path}` is past 4 GiB"))?;
        let offset = u32::try_from(out.len()).map_err(|_| past_4_gib())?;
        let name = path.as_bytes();
        let nlen = u16::try_from(name.len()).map_err(|_| tr!("パス `{path}` が長すぎます", "the path `{path}` is too long"))?;
        // the local header
        put32(&mut out, LOCAL);
        for v in [NEEDED, UTF8, method, 0, DATE] {
            put16(&mut out, v);
        }
        for v in [crc, csize, size] {
            put32(&mut out, v);
        }
        put16(&mut out, nlen);
        put16(&mut out, 0);
        out.extend(name);
        out.extend(data);
        // its entry of the central directory
        put32(&mut central, CENTRAL);
        for v in [MADE_BY, NEEDED, UTF8, method, 0, DATE] {
            put16(&mut central, v);
        }
        for v in [crc, csize, size] {
            put32(&mut central, v);
        }
        for v in [nlen, 0, 0, 0, 0] {
            put16(&mut central, v);
        }
        put32(&mut central, MODE);
        put32(&mut central, offset);
        central.extend(name);
    }
    let n = seen.len() as u16;
    let at = u32::try_from(out.len()).map_err(|_| past_4_gib())?;
    let csize = u32::try_from(central.len()).map_err(|_| past_4_gib())?;
    out.extend(central);
    put32(&mut out, END);
    for v in [0, 0, n, n] {
        put16(&mut out, v);
    }
    put32(&mut out, csize);
    put32(&mut out, at);
    put16(&mut out, 0);
    if u32::try_from(out.len()).is_err() {
        return Err(past_4_gib());
    }
    Ok(out)
}

/// The files of a zip [`write`] wrote, by their paths, in the order written. Err with why it is not.
pub fn read(z: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Text> {
    let u16_at = |i: usize| -> Result<u16, Text> { z.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(too_soon) };
    let u32_at = |i: usize| -> Result<u32, Text> { z.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(too_soon) };
    if z.len() < 22 {
        return Err(tr!("zip にしては短すぎます", "it is too short to be a zip"));
    }
    let end = z.len() - 22;
    if u32_at(end)? != END {
        return Err(tr!("終わりが、コメントの無い zip の中央ディレクトリの終わりではありません", "it does not end with the end of a central directory with no comment, as a zip ritsu writes does"));
    }
    let n = usize::from(u16_at(end + 10)?);
    if usize::from(u16_at(end + 8)?) != n || u16_at(end + 4)? != 0 || u16_at(end + 6)? != 0 {
        return Err(tr!("複数のディスクに分けた zip です", "it is a zip of more than one disk"));
    }
    if u16_at(end + 20)? != 0 {
        return Err(tr!("コメントがあります", "it has a comment"));
    }
    let (csize, at) = (u32_at(end + 12)? as usize, u32_at(end + 16)? as usize);
    if at.checked_add(csize) != Some(end) {
        return Err(tr!("中央ディレクトリが、終わりのすぐ前にありません", "its central directory is not just before its end"));
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut total: u64 = 0;
    let mut i = at;
    let mut next_local = 0usize;
    for _ in 0..n {
        if u32_at(i)? != CENTRAL {
            return Err(tr!("中央ディレクトリの項目の印が違います", "an entry of its central directory does not begin as one does"));
        }
        let (flags, method) = (u16_at(i + 8)?, u16_at(i + 10)?);
        let (crc, csize, size) = (u32_at(i + 16)?, u32_at(i + 20)? as usize, u32_at(i + 24)? as usize);
        let (nlen, xlen, clen) = (usize::from(u16_at(i + 28)?), usize::from(u16_at(i + 30)?), usize::from(u16_at(i + 32)?));
        let offset = u32_at(i + 42)? as usize;
        let name = z.get(i + 46..i + 46 + nlen).ok_or_else(|| tr!("項目の名前が zip の終わりを越えます", "an entry's name ends past the zip"))?;
        let path = std::str::from_utf8(name).map_err(|_| tr!("UTF-8 でないパスがあります", "a path is not UTF-8"))?.to_string();
        let made = [u16_at(i + 4)?, u16_at(i + 6)?, u16_at(i + 12)?, u16_at(i + 14)?, u16_at(i + 34)?, u16_at(i + 36)?];
        if xlen != 0 || clen != 0 || flags & !UTF8 != 0 || made != [MADE_BY, NEEDED, 0, DATE, 0, 0] || u32_at(i + 38)? != MODE {
            return Err(tr!("`{path}` に、ritsu の書くバンドルには無い拡張フィールド、コメント、フラグ、時刻か属性があります", "`{path}` has an extra field, a comment, a flag, a time or attributes no bundle ritsu writes has"));
        }
        if let Some(why) = ritsu_base::modules::tar::path_why(&path) {
            return Err(path_error(&path, why));
        }
        if !seen.insert(path.clone()) {
            return Err(tr!("`{path}` が二度あります", "`{path}` twice"));
        }
        total += size as u64;
        if total > MAX {
            return Err(tr!("ファイルの合計が {MAX} バイトを超えます", "its files are more than {MAX} bytes"));
        }
        if offset != next_local || u32_at(offset)? != LOCAL || u16_at(offset + 4)? != NEEDED || u16_at(offset + 10)? != 0 || u16_at(offset + 12)? != DATE || u16_at(offset + 6)? != flags || u16_at(offset + 8)? != method || u32_at(offset + 14)? != crc {
            return Err(tr!("`{path}` のローカルヘッダーが、中央ディレクトリの項目と合いません", "the local header of `{path}` is not as its central entry says"));
        }
        if u32_at(offset + 18)? as usize != csize || u32_at(offset + 22)? as usize != size || usize::from(u16_at(offset + 26)?) != nlen || u16_at(offset + 28)? != 0 {
            return Err(tr!("`{path}` のローカルヘッダーが、中央ディレクトリの項目と合いません", "the local header of `{path}` is not as its central entry says"));
        }
        if z.get(offset + 30..offset + 30 + nlen) != Some(name) {
            return Err(tr!("`{path}` のローカルヘッダーの名前が、中央ディレクトリの項目と違います", "the local header's name of `{path}` is not the one its central entry says"));
        }
        let start = offset + 30 + nlen;
        let data = z.get(start..start + csize).ok_or_else(|| tr!("`{path}` が zip の終わりを越えます", "`{path}` ends past the zip"))?;
        let bytes = match method {
            0 => data.to_vec(),
            8 => inflate(data, size).map_err(|why| tr!("`{path}`: ", "`{path}`: ").then(&why))?,
            m => return Err(tr!("`{path}` の圧縮の方式 {m} は、バンドルの使わない方式です", "`{path}` is compressed by the method {m}, which a bundle does not use")),
        };
        if bytes.len() != size || crc32(&bytes) != crc {
            return Err(tr!("`{path}` の中身が、CRC-32 と大きさに合いません", "`{path}` is not the file its CRC-32 and its size say"));
        }
        out.push((path, bytes));
        next_local = start + csize;
        i += 46 + nlen;
    }
    if next_local != at || i != end {
        return Err(tr!("ファイルの間か中央ディレクトリの後に、余分なバイトがあります", "something is between its files or after its central directory"));
    }
    Ok(out)
}

/// Why a path a bundle would hold is refused, with the characters that cannot print escaped.
pub fn path_error(path: &str, why: ritsu_base::modules::tar::PathWhy) -> Text {
    use ritsu_base::modules::tar::{MAX_PARTS, MAX_PATH, PathWhy as W};
    let p = ritsu_base::text::escape_controls(path);
    let w = match why {
        W::NotUtf8 => tr!("UTF-8 ではありません", "it is not UTF-8"),
        W::Empty => tr!("空です", "it is empty"),
        W::Absolute => tr!("絶対パスです", "it is absolute"),
        W::EmptyPart => tr!("空の要素があります", "it has an empty part"),
        W::Dot => tr!("`.` という要素があります", "it has a part `.`"),
        W::DotDot => tr!("`..` という要素があります", "it has a part `..`"),
        W::Backslash => tr!("`\\` があります", "it has a `\\` in it"),
        W::Nul => tr!("NUL があります", "it has a NUL in it"),
        W::Control => tr!("制御文字があります", "it has a control character in it"),
        W::DotStart => tr!("`.` で始まる要素があります", "it has a part that starts with `.`"),
        W::AtStart => tr!("最初の要素が `@` で始まります", "its first part starts with `@`"),
        W::TooLong => tr!("{MAX_PATH} バイトを超えます", "it is longer than {MAX_PATH} bytes"),
        W::TooDeep => tr!("要素が {MAX_PARTS} 個を超えます", "it has more than {MAX_PARTS} parts"),
    };
    tr!("バンドルのファイルのパス `{p}` は受け付けません。", "the path `{p}` of a bundle's file is refused: ").then(&w)
}

fn too_soon() -> Text {
    tr!("途中で終わっています", "it ends too soon")
}

fn past_4_gib() -> Text {
    tr!("zip が 4 GiB を超えます", "the zip is past 4 GiB")
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

/// CRC-32 as zip computes it (the polynomial 0xEDB88320, reflected).
pub fn crc32(bytes: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, slot) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c = table[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// The bases and the extra bits of the codes of the lengths 3–258 (257–285) and of the distances
/// 1–32768 (0–29), as RFC 1951 3.2.5 gives them.
const LEN_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

const WINDOW: usize = 32 * 1024;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// How many earlier places of the same three bytes are tried at each place.
const CHAIN: usize = 64;
const HASH_BITS: u32 = 15;

/// Bits written from the lowest, as deflate packs them.
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, value: u32, bits: u32) {
        self.acc |= u64::from(value) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// A Huffman code, whose bits go from its highest.
    fn code(&mut self, code: u32, len: u32) {
        let mut r = 0;
        for i in 0..len {
            r |= ((code >> i) & 1) << (len - 1 - i);
        }
        self.put(r, len);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

/// The fixed code of a literal or a length's symbol (0–287), and its length in bits.
fn fixed(sym: u32) -> (u32, u32) {
    match sym {
        0..=143 => (0x30 + sym, 8),
        144..=255 => (0x190 + sym - 144, 9),
        256..=279 => (sym - 256, 7),
        _ => (0xC0 + sym - 280, 8),
    }
}

/// `bytes` compressed with deflate: one final block of the fixed codes.
pub fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut w = Bits { out: Vec::with_capacity(bytes.len() / 4), acc: 0, n: 0 };
    // BFINAL, then BTYPE 01
    w.put(1, 1);
    w.put(1, 2);
    let hash = |i: usize| -> usize { ((u32::from(bytes[i]) << 16 | u32::from(bytes[i + 1]) << 8 | u32::from(bytes[i + 2])).wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize };
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let insert = |i: usize, head: &mut Vec<usize>, prev: &mut Vec<usize>| {
        if i + MIN_MATCH <= bytes.len() {
            let h = hash(i);
            prev[i % WINDOW] = head[h];
            head[h] = i;
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        let (mut best, mut at) = (0usize, 0usize);
        if i + MIN_MATCH <= bytes.len() {
            let mut cand = head[hash(i)];
            let limit = (bytes.len() - i).min(MAX_MATCH);
            let mut tries = 0;
            while cand != usize::MAX && i - cand <= WINDOW && tries < CHAIN {
                let mut l = 0;
                while l < limit && bytes[cand + l] == bytes[i + l] {
                    l += 1;
                }
                if l > best {
                    best = l;
                    at = i - cand;
                    if l == limit {
                        break;
                    }
                }
                let p = prev[cand % WINDOW];
                // an older place is further back; one that is not was written over
                if p == usize::MAX || p >= cand {
                    break;
                }
                cand = p;
                tries += 1;
            }
        }
        if best >= MIN_MATCH {
            let li = LEN_BASE.iter().rposition(|b| usize::from(*b) <= best).expect("a length of 3 or more");
            let (code, len) = fixed(257 + li as u32);
            w.code(code, len);
            w.put((best - usize::from(LEN_BASE[li])) as u32, u32::from(LEN_EXTRA[li]));
            let di = DIST_BASE.iter().rposition(|b| usize::from(*b) <= at).expect("a distance of 1 or more");
            w.code(di as u32, 5);
            w.put((at - usize::from(DIST_BASE[di])) as u32, u32::from(DIST_EXTRA[di]));
            for k in i..i + best {
                insert(k, &mut head, &mut prev);
            }
            i += best;
        } else {
            let (code, len) = fixed(u32::from(bytes[i]));
            w.code(code, len);
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    let (code, len) = fixed(256);
    w.code(code, len);
    w.finish()
}

/// What [`deflate`] compressed, of `size` bytes: blocks stored or of the fixed codes only.
pub fn inflate(data: &[u8], size: usize) -> Result<Vec<u8>, Text> {
    struct In<'a> {
        b: &'a [u8],
        pos: usize,
        acc: u64,
        n: u32,
    }
    impl In<'_> {
        fn bits(&mut self, k: u32) -> Result<u32, Text> {
            while self.n < k {
                let byte = *self.b.get(self.pos).ok_or_else(|| tr!("データがブロックの途中で終わっています", "the data ends inside a block"))?;
                self.pos += 1;
                self.acc |= u64::from(byte) << self.n;
                self.n += 8;
            }
            let v = (self.acc & ((1u64 << k) - 1)) as u32;
            self.acc >>= k;
            self.n -= k;
            Ok(v)
        }
        /// A symbol of the fixed codes of literals and lengths.
        fn symbol(&mut self) -> Result<u32, Text> {
            let mut code = 0u32;
            for len in 1..=9 {
                code = (code << 1) | self.bits(1)?;
                let sym = match len {
                    7 if code <= 0x17 => Some(code + 256),
                    8 if (0x30..=0xBF).contains(&code) => Some(code - 0x30),
                    8 if (0xC0..=0xC7).contains(&code) => Some(code - 0xC0 + 280),
                    9 if (0x190..=0x1FF).contains(&code) => Some(code - 0x190 + 144),
                    _ => None,
                };
                if let Some(s) = sym {
                    return Ok(s);
                }
            }
            Err(tr!("固定の符号に無い記号です", "no symbol of the fixed codes"))
        }
    }
    let mut r = In { b: data, pos: 0, acc: 0, n: 0 };
    let mut out: Vec<u8> = Vec::with_capacity(size);
    loop {
        let last = r.bits(1)?;
        match r.bits(2)? {
            0 => {
                r.acc = 0;
                r.n = 0;
                let b = r.b.get(r.pos..r.pos + 4).ok_or_else(|| tr!("無圧縮のブロックが途中で終わっています", "a stored block ends too soon"))?;
                let (len, nlen) = (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]));
                if len != !nlen {
                    return Err(tr!("無圧縮のブロックの長さが、その補数と合いません", "a stored block's length is not as its complement says"));
                }
                r.pos += 4;
                let s = r.b.get(r.pos..r.pos + usize::from(len)).ok_or_else(|| tr!("無圧縮のブロックが途中で終わっています", "a stored block ends too soon"))?;
                out.extend(s);
                r.pos += usize::from(len);
            }
            1 => loop {
                let sym = r.symbol()?;
                match sym {
                    0..=255 => out.push(sym as u8),
                    256 => break,
                    257..=285 => {
                        let li = (sym - 257) as usize;
                        let len = usize::from(LEN_BASE[li]) + r.bits(u32::from(LEN_EXTRA[li]))? as usize;
                        let mut d = 0u32;
                        for _ in 0..5 {
                            d = (d << 1) | r.bits(1)?;
                        }
                        let di = d as usize;
                        if di >= DIST_BASE.len() {
                            return Err(tr!("距離の符号が 29 を超えます", "a distance code past 29"));
                        }
                        let dist = usize::from(DIST_BASE[di]) + r.bits(u32::from(DIST_EXTRA[di]))? as usize;
                        if dist > out.len() {
                            return Err(tr!("距離が先頭より前を指します", "a distance back past the start"));
                        }
                        let from = out.len() - dist;
                        for k in 0..len {
                            out.push(out[from + k]);
                        }
                    }
                    _ => return Err(tr!("長さの符号が 285 を超えます", "a length code past 285")),
                }
                if out.len() > size {
                    return Err(tr!("大きさより多くのバイトがあります", "more bytes than its size"));
                }
            },
            2 => return Err(tr!("動的な符号のブロックで、バンドルの使わない形です", "a block of dynamic codes, which a bundle does not use")),
            _ => return Err(tr!("deflate に無い型のブロックです", "a block of a type deflate does not have")),
        }
        if out.len() > size {
            return Err(tr!("大きさより多くのバイトがあります", "more bytes than its size"));
        }
        if last == 1 {
            break;
        }
    }
    if r.pos != data.len() {
        return Err(tr!("最後のブロックの後に、余分なバイトがあります", "something after its last block"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_is_zips() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn what_deflate_writes_inflates_back() {
        let mut cases: Vec<Vec<u8>> = vec![Vec::new(), b"a".to_vec(), b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec()];
        cases.push((0..100_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect());
        cases.push("{\"from\":\"approve\",\"answer\":{\"ok\":true}}\n".repeat(5000).into_bytes());
        cases.push("休暇の申請、経費の申請。".repeat(3000).into_bytes());
        for c in cases {
            let d = deflate(&c);
            assert_eq!(inflate(&d, c.len()).unwrap(), c);
        }
    }

    #[test]
    fn a_zip_reads_back_as_written_and_its_bytes_do_not_depend_on_the_order_given() {
        let files = vec![("b/経費.ringi".to_string(), "request 経費 v1\n".repeat(100).into_bytes()), ("a.json".to_string(), Vec::new()), ("wasm.json".to_string(), b"{}\n".to_vec())];
        let z = write(&files).unwrap();
        let mut back = read(&z).unwrap();
        back.sort();
        let mut want = files.clone();
        want.sort();
        assert_eq!(back, want);
        let mut reversed = files.clone();
        reversed.reverse();
        assert_eq!(write(&reversed).unwrap(), z);
        assert!(write(&[("a".to_string(), Vec::new()), ("a".to_string(), Vec::new())]).is_err());
        // what is not the way it is written is refused
        let mut commented = z.clone();
        let n = commented.len();
        commented[n - 2] = 1;
        assert!(read(&commented).is_err());
        let mut flipped = z.clone();
        flipped[40] ^= 0xff;
        assert!(read(&flipped).is_err());
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;

    #[test]
    fn a_local_name_that_differs_is_refused() {
        let z = write(&[("source/leave.ringi".to_string(), b"x".to_vec()), ("wasm.json".to_string(), b"{}".to_vec())]).unwrap();
        let mut bad = z.clone();
        // the first file's local header is at 0; flip one byte of its name there alone
        let at = 30 + "source/".len();
        bad[at] = b'L';
        assert!(read(&bad).is_err());
    }

    #[test]
    fn control_characters_make_a_path_refused() {
        assert!(ritsu_base::modules::tar::path_why("a\nb").is_some());
        assert!(write(&[("a\nb".to_string(), Vec::new())]).is_err());
    }
}
