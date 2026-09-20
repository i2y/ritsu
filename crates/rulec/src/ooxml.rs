//! What an `.xlsx` and a `.docx` are both made of (§15.50, §15.82): a ZIP directory with
//! DEFLATE inside it, and enough XML to walk elements, attributes and text.
//!
//! No dependencies (§12), so the container is opened here. Both readers are small because the
//! shapes that turn up in an Office file are small — but the parts that are read are read
//! properly, and anything not understood is refused by name rather than guessed at. They were
//! written for the workbook; the Word document that came later needed no line of them changed,
//! which is the whole reason a `.docx` costs so little.

// ---------------------------------------------------------------- the container (ZIP)

pub(crate) fn u16le(b: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]) as usize)
}

pub(crate) fn u32le(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]) as usize)
}

pub(crate) struct Entry {
    pub(crate) name: String,
    method: usize,
    comp: usize,
    raw: usize,
    local: usize,
}

/// The central directory, read from the end of the file as the format says to.
pub(crate) fn entries(b: &[u8]) -> Result<Vec<Entry>, String> {
    let bad = || tr!("xlsx として読めません（ZIP ではありません）", "not readable as an xlsx (it is not a ZIP)");
    // The end-of-directory record is last, after a comment of up to 64 KiB.
    let start = b.len().saturating_sub(22 + 0xFFFF);
    let eocd = (start..b.len().saturating_sub(21))
        .rev()
        .find(|&i| b[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))
        .ok_or_else(bad)?;
    let count = u16le(b, eocd + 10).ok_or_else(bad)?;
    let mut at = u32le(b, eocd + 16).ok_or_else(bad)?;
    if at == 0xFFFF_FFFF || count == 0xFFFF {
        return Err(tr!("ZIP64 の xlsx は読めません", "a ZIP64 xlsx cannot be read"));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if !b.get(at..at + 4).is_some_and(|s| s == [0x50, 0x4b, 0x01, 0x02]) {
            return Err(bad());
        }
        let method = u16le(b, at + 10).ok_or_else(bad)?;
        let comp = u32le(b, at + 20).ok_or_else(bad)?;
        let raw = u32le(b, at + 24).ok_or_else(bad)?;
        let nlen = u16le(b, at + 28).ok_or_else(bad)?;
        let elen = u16le(b, at + 30).ok_or_else(bad)?;
        let clen = u16le(b, at + 32).ok_or_else(bad)?;
        let local = u32le(b, at + 42).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(b.get(at + 46..at + 46 + nlen).ok_or_else(bad)?).into_owned();
        out.push(Entry { name, method, comp, raw, local });
        at += 46 + nlen + elen + clen;
    }
    Ok(out)
}

/// One entry's bytes. The sizes come from the directory, so an entry written with a data
/// descriptor (its local header's sizes left at zero) is read the same way.
pub(crate) fn open(b: &[u8], e: &Entry) -> Result<Vec<u8>, String> {
    let bad = || tr!("xlsx の中身が壊れています: {}", "the xlsx's contents are damaged: {}", e.name);
    if !b.get(e.local..e.local + 4).is_some_and(|s| s == [0x50, 0x4b, 0x03, 0x04]) {
        return Err(bad());
    }
    let nlen = u16le(b, e.local + 26).ok_or_else(bad)?;
    let elen = u16le(b, e.local + 28).ok_or_else(bad)?;
    let from = e.local + 30 + nlen + elen;
    let data = b.get(from..from + e.comp).ok_or_else(bad)?;
    match e.method {
        0 => Ok(data.to_vec()),
        8 => inflate(data, e.raw),
        m => Err(tr!(
            "xlsx の {} は圧縮方式 {m} で、読めるのは無圧縮と deflate だけです",
            "{} in the xlsx uses compression method {m}; only stored and deflate are read",
            e.name
        )),
    }
}

// ---------------------------------------------------------------- DEFLATE (RFC 1951)

struct Bits<'a> {
    src: &'a [u8],
    at: usize,
    bit: u32,
    have: u32,
}

impl<'a> Bits<'a> {
    fn new(src: &'a [u8]) -> Self {
        Bits { src, at: 0, bit: 0, have: 0 }
    }

    /// The next `n` bits, least significant first. After any call at most seven bits are
    /// left buffered, which is what lets a stored block re-align on the next byte.
    fn take(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        while self.have < n {
            let byte = *self.src.get(self.at).ok_or_else(|| {
                tr!("deflate の途中でデータが尽きました", "the deflate stream ended in the middle")
            })?;
            self.bit |= (byte as u32) << self.have;
            self.have += 8;
            self.at += 1;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.have -= n;
        Ok(v)
    }
}

/// A canonical Huffman table as the lengths give it: how many codes of each length, and the
/// symbols in code order.
struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for l in 1..16 {
            offs[l] = offs[l - 1] + counts[l - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { counts, symbols }
    }

    fn decode(&self, b: &mut Bits) -> Result<u16, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.take(1)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(tr!("deflate の符号が壊れています", "the deflate stream has a broken code"))
    }
}

const LEN_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u32; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn fixed() -> (Huff, Huff) {
    let mut lit = [8u8; 288];
    lit[144..256].fill(9);
    lit[256..280].fill(7);
    (Huff::new(&lit), Huff::new(&[5u8; 30]))
}

/// The code lengths of a dynamic block, themselves Huffman-coded.
fn dynamic(b: &mut Bits) -> Result<(Huff, Huff), String> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let nlit = b.take(5)? as usize + 257;
    let ndist = b.take(5)? as usize + 1;
    let nclen = b.take(4)? as usize + 4;
    let mut clen = [0u8; 19];
    for &i in ORDER.iter().take(nclen) {
        clen[i] = b.take(3)? as u8;
    }
    let ctree = Huff::new(&clen);
    let mut lengths = vec![0u8; nlit + ndist];
    let mut i = 0;
    while i < lengths.len() {
        let sym = ctree.decode(b)?;
        let (value, times) = match sym {
            0..=15 => (sym as u8, 1),
            16 => (*lengths.get(i.wrapping_sub(1)).unwrap_or(&0), 3 + b.take(2)? as usize),
            17 => (0, 3 + b.take(3)? as usize),
            18 => (0, 11 + b.take(7)? as usize),
            _ => return Err(tr!("deflate の符号長が壊れています", "the deflate code lengths are broken")),
        };
        if sym == 16 && i == 0 {
            return Err(tr!("deflate の符号長が壊れています", "the deflate code lengths are broken"));
        }
        for _ in 0..times {
            if i >= lengths.len() {
                return Err(tr!("deflate の符号長が壊れています", "the deflate code lengths are broken"));
            }
            lengths[i] = value;
            i += 1;
        }
    }
    Ok((Huff::new(&lengths[..nlit]), Huff::new(&lengths[nlit..])))
}

/// Raw DEFLATE, with the uncompressed size the directory promised as the ceiling.
fn inflate(src: &[u8], expect: usize) -> Result<Vec<u8>, String> {
    let mut b = Bits::new(src);
    let mut out: Vec<u8> = Vec::with_capacity(expect);
    let broken = || tr!("deflate のデータが壊れています", "the deflate data is damaged");
    loop {
        let last = b.take(1)?;
        match b.take(2)? {
            0 => {
                // A stored block restarts on a byte boundary.
                b.bit = 0;
                b.have = 0;
                let len = u16le(src, b.at).ok_or_else(broken)?;
                let nlen = u16le(src, b.at + 2).ok_or_else(broken)?;
                if len + nlen != 0xFFFF {
                    return Err(broken());
                }
                let from = b.at + 4;
                out.extend_from_slice(src.get(from..from + len).ok_or_else(broken)?);
                b.at = from + len;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 { fixed() } else { dynamic(&mut b)? };
                loop {
                    let sym = lit.decode(&mut b)? as usize;
                    if sym == 256 {
                        break;
                    }
                    if sym < 256 {
                        out.push(sym as u8);
                        if out.len() > expect {
                            return Err(broken());
                        }
                        continue;
                    }
                    let i = sym - 257;
                    let (lb, le) = (*LEN_BASE.get(i).ok_or_else(broken)?, LEN_EXTRA[i]);
                    let len = lb as usize + b.take(le)? as usize;
                    let d = dist.decode(&mut b)? as usize;
                    let (db, de) = (*DIST_BASE.get(d).ok_or_else(broken)?, DIST_EXTRA[d]);
                    let back = db as usize + b.take(de)? as usize;
                    if back > out.len() {
                        return Err(broken());
                    }
                    let from = out.len() - back;
                    for k in 0..len {
                        out.push(out[from + k]);
                    }
                    // The directory said how long this entry is; anything past it is damage.
                    if out.len() > expect {
                        return Err(broken());
                    }
                }
            }
            _ => return Err(broken()),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- enough XML

/// What the walk hands back: an element with its attributes, a closing tag, or text.
pub(crate) enum Node<'a> {
    Open { name: &'a str, attrs: &'a str, empty: bool },
    Close(&'a str),
    Text(&'a str),
}

/// Walk the document. Declarations, comments and doctypes are skipped; a CDATA section is
/// text. Everything a worksheet holds is one of those.
pub(crate) fn walk(src: &str, mut f: impl FnMut(Node)) {
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let end = src[i..].find('<').map(|k| i + k).unwrap_or(b.len());
            f(Node::Text(&src[i..end]));
            i = end;
            continue;
        }
        if src[i..].starts_with("<![CDATA[") {
            let end = src[i + 9..].find("]]>").map(|k| i + 9 + k).unwrap_or(b.len());
            f(Node::Text(&src[i + 9..end]));
            i = (end + 3).min(b.len());
            continue;
        }
        if src[i..].starts_with("<?") || src[i..].starts_with("<!") {
            let end = src[i..].find('>').map(|k| i + k + 1).unwrap_or(b.len());
            i = end;
            continue;
        }
        let end = match src[i..].find('>') {
            Some(k) => i + k,
            None => break,
        };
        let inner = &src[i + 1..end];
        if let Some(name) = inner.strip_prefix('/') {
            f(Node::Close(name.trim()));
        } else {
            let empty = inner.ends_with('/');
            let inner = inner.strip_suffix('/').unwrap_or(inner);
            let (name, attrs) = match inner.find(|c: char| c.is_whitespace()) {
                Some(k) => (&inner[..k], &inner[k..]),
                None => (inner, ""),
            };
            f(Node::Open { name, attrs, empty });
        }
        i = end + 1;
    }
}

pub(crate) fn attr(attrs: &str, key: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some(k) = rest.find('=') {
        let name = rest[..k].trim();
        let after = rest[k + 1..].trim_start();
        let q = after.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        let end = after[1..].find(q)? + 1;
        if name == key || name.rsplit(':').next() == Some(key) {
            return Some(unescape(&after[1..end]));
        }
        rest = &after[end + 1..];
    }
    None
}

pub(crate) fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let Some(end) = rest.find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        match name {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            n if n.starts_with("#x") || n.starts_with("#X") => {
                if let Some(c) = u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            n if n.starts_with('#') => {
                if let Some(c) = n[1..].parse::<u32>().ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            _ => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// One part of the container, as text. A part that is not there is `None`; a part that is not
/// UTF-8 is an error, because guessing an encoding would make the digest depend on the guess.
pub(crate) fn part(b: &[u8], es: &[Entry], name: &str) -> Result<Option<String>, String> {
    let Some(e) = es.iter().find(|e| e.name == name) else { return Ok(None) };
    String::from_utf8(open(b, e)?).map(Some).map_err(|_| tr!("{name} が UTF-8 ではありません", "{name} is not UTF-8"))
}
