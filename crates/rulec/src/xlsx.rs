//! Reading a spreadsheet's `.xlsx` (§1.4, §15.50): the sheet as a grid of text, which is
//! what `import` already knows how to draft from.
//!
//! One direction only, and a draft rather than a rule — that decision is §1.4's and this
//! module does not widen it. What it adds over `import csv` is that **no export step comes
//! first**: the file the business sends is the file that is read.
//!
//! No dependencies (§12), so the container is opened here: a ZIP directory, DEFLATE, and
//! enough XML to walk elements and attributes. All three are small because the shapes that
//! turn up in a workbook are small — but the parts that are read are read properly, and
//! anything not understood is refused by name rather than guessed at.
//!
//! What a cell *means* is where a spreadsheet differs from a CSV, and it is why this is not
//! a line of shell: a date is a number, a percentage is a hundredth, and the unit of a
//! money column lives in the number format and not in the cell. Those three are read from
//! the styles; everything else is taken as the text it is.

use std::collections::BTreeMap;

// ---------------------------------------------------------------- the container (ZIP)

fn u16le(b: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]) as usize)
}

fn u32le(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]) as usize)
}

struct Entry {
    name: String,
    method: usize,
    comp: usize,
    raw: usize,
    local: usize,
}

/// The central directory, read from the end of the file as the format says to.
fn entries(b: &[u8]) -> Result<Vec<Entry>, String> {
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
fn open(b: &[u8], e: &Entry) -> Result<Vec<u8>, String> {
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
enum Node<'a> {
    Open { name: &'a str, attrs: &'a str, empty: bool },
    Close(&'a str),
    Text(&'a str),
}

/// Walk the document. Declarations, comments and doctypes are skipped; a CDATA section is
/// text. Everything a worksheet holds is one of those.
fn walk(src: &str, mut f: impl FnMut(Node)) {
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

fn attr(attrs: &str, key: &str) -> Option<String> {
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

fn unescape(s: &str) -> String {
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

// ---------------------------------------------------------------- what a cell means

/// What a number format does to the value stored in the cell. Three kinds matter; for
/// everything else the digits are taken as they are written.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Fmt {
    Plain,
    Date,
    Percent,
}

/// The built-in format ids that mean a date. 27–36 and 50–58 are the Japanese, Chinese and
/// Korean calendars' date formats, which is exactly where a tariff with dates turns up.
fn builtin(id: usize) -> (Fmt, String) {
    match id {
        14..=17 | 22 | 27..=36 | 50..=58 => (Fmt::Date, String::new()),
        9 | 10 => (Fmt::Percent, String::new()),
        _ => (Fmt::Plain, String::new()),
    }
}

/// Read a format code: what it does to the value, and the unit it writes after the number.
///
/// The unit is only taken when the workbook *states* it — a literal in quotes (`#,##0"円"`)
/// or a yen sign. A `$` is left alone on purpose: it is USD, CAD, AUD and several more, and
/// this importer does not decide which (§1.4 — it never decides an amount either).
fn read_format(code: &str) -> (Fmt, String) {
    let mut lit = String::new();
    let mut yen = false;
    let (mut date, mut percent) = (false, false);
    let (mut quoted, mut bracket, mut escape) = (false, false, false);
    for c in code.chars() {
        if escape {
            escape = false;
            lit.push(c);
            continue;
        }
        if quoted {
            if c == '"' {
                quoted = false;
            } else {
                lit.push(c);
            }
            continue;
        }
        if bracket {
            if c == ']' {
                bracket = false;
            } else if c == '¥' || c == '￥' {
                yen = true;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '\\' => escape = true,
            '[' => bracket = true,
            ';' => break, // the first section is the one positive numbers take
            '%' => percent = true,
            '¥' | '￥' => yen = true,
            // `m` is month *and* minute, and `e` is the exponent of a scientific format,
            // so the day and the year are what a date is recognised by. Every era format
            // (`[$-411]ggge"年"m"月"d"日"`) carries a `d` too.
            'y' | 'Y' | 'd' | 'D' => date = true,
            _ => {}
        }
    }
    if percent {
        return (Fmt::Percent, String::new());
    }
    if date {
        return (Fmt::Date, String::new());
    }
    let lit = lit.trim().to_string();
    if !lit.is_empty() && lit.chars().all(|c| c.is_alphanumeric()) {
        return (Fmt::Plain, lit);
    }
    (Fmt::Plain, if yen { "円".into() } else { String::new() })
}

/// The civil date `days` after 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A date cell's serial number as `YYYY-MM-DD`.
///
/// 1900-02-29 never existed; the serial 60 is a day Excel invented and kept for
/// compatibility with an older program. It is left as the number it is rather than turned
/// into a date that was never on a calendar.
fn serial_date(v: &str, date1904: bool) -> Option<String> {
    let n: i64 = v.split('.').next()?.parse().ok()?;
    let days = if date1904 {
        n - 24107
    } else if n >= 61 {
        n - 25569
    } else if (1..=59).contains(&n) {
        n - 25568
    } else {
        return None;
    };
    let (y, m, d) = civil(days);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// A decimal written as text, times a hundred, without going through a float: a percentage
/// is stored as its fraction and 0.1% steps are exactly what this importer has to keep.
fn times100(v: &str) -> Option<String> {
    let (sign, body) = match v.strip_prefix('-') {
        Some(b) => ("-", b),
        None => ("", v),
    };
    if !body.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let digits = format!("{int}{frac:0<2}");
    let point = int.len() + 2;
    let (mut i, f) = digits.split_at(point.min(digits.len()));
    i = i.trim_start_matches('0');
    let f = f.trim_end_matches('0');
    let i = if i.is_empty() { "0" } else { i };
    Some(if f.is_empty() { format!("{sign}{i}") } else { format!("{sign}{i}.{f}") })
}

// ---------------------------------------------------------------- the workbook

fn part(b: &[u8], es: &[Entry], name: &str) -> Result<Option<String>, String> {
    let Some(e) = es.iter().find(|e| e.name == name) else { return Ok(None) };
    String::from_utf8(open(b, e)?)
        .map(Some)
        .map_err(|_| tr!("xlsx の {name} が UTF-8 ではありません", "{name} in the xlsx is not UTF-8"))
}

/// The workbook's strings. A cell of text holds an index into this table, not the text.
///
/// `rPh` is the reading (furigana) Excel keeps beside a Japanese string; it is part of the
/// entry and not part of the value, so it is skipped rather than concatenated.
fn shared_strings(xml: &str) -> Vec<String> {
    let (mut out, mut cur) = (Vec::new(), String::new());
    let (mut in_si, mut in_ph, mut in_t) = (false, false, false);
    walk(xml, |n| match n {
        Node::Open { name: "si", .. } => {
            in_si = true;
            cur.clear();
        }
        Node::Open { name: "rPh", .. } => in_ph = true,
        Node::Open { name: "t", empty, .. } if in_si && !in_ph && !empty => in_t = true,
        Node::Text(t) if in_t => cur.push_str(&unescape(t)),
        Node::Close("t") => in_t = false,
        Node::Close("rPh") => in_ph = false,
        Node::Close("si") => {
            in_si = false;
            out.push(std::mem::take(&mut cur));
        }
        _ => {}
    });
    out
}

/// `cellXfs` in order: what each style index does to a number. `cellStyleXfs` holds `xf`
/// elements too and comes first, which is why the walk waits for the right parent.
fn read_styles(xml: &str) -> Vec<(Fmt, String)> {
    let mut custom: BTreeMap<usize, String> = BTreeMap::new();
    let mut ids: Vec<usize> = Vec::new();
    let mut in_cell_xfs = false;
    walk(xml, |n| match n {
        Node::Open { name: "numFmt", attrs, .. } => {
            if let (Some(id), Some(code)) = (attr(attrs, "numFmtId"), attr(attrs, "formatCode")) {
                if let Ok(id) = id.parse::<usize>() {
                    custom.insert(id, code);
                }
            }
        }
        Node::Open { name: "cellXfs", .. } => in_cell_xfs = true,
        Node::Open { name: "xf", attrs, .. } if in_cell_xfs => {
            ids.push(attr(attrs, "numFmtId").and_then(|s| s.parse().ok()).unwrap_or(0));
        }
        Node::Close("cellXfs") => in_cell_xfs = false,
        _ => {}
    });
    ids.iter()
        .map(|id| match custom.get(id) {
            Some(code) => read_format(code),
            None => builtin(*id),
        })
        .collect()
}

/// `rId3` → the part it names, as a path in the package.
fn relations(xml: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk(xml, |n| {
        if let Node::Open { name: "Relationship", attrs, .. } = n {
            if let (Some(id), Some(target)) = (attr(attrs, "Id"), attr(attrs, "Target")) {
                let target = match target.strip_prefix('/') {
                    Some(abs) => abs.to_string(),
                    None => format!("xl/{}", target.trim_start_matches("./")),
                };
                out.insert(id, target);
            }
        }
    });
    out
}

/// The letters of a cell reference as a 1-based column: `A` is 1, `AB` is 28.
fn ref_col(r: &str) -> Option<usize> {
    let mut n = 0usize;
    for c in r.chars().take_while(|c| c.is_ascii_alphabetic()) {
        n = n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    (n > 0).then_some(n)
}

/// One sheet as a grid of text: the sheet's name, and its rows.
///
/// Rows and columns that hold nothing at all are left out, so a table that starts at C4 is
/// read as a table and not as two empty columns and three empty rows. What is left is
/// rectangular, and `import`'s first row is its header, exactly as with a CSV.
pub fn grid(bytes: &[u8], want: Option<&str>) -> Result<(String, Vec<Vec<String>>), String> {
    let es = entries(bytes)?;
    let book = part(bytes, &es, "xl/workbook.xml")?.ok_or_else(|| {
        tr!("xlsx ではありません（xl/workbook.xml がありません）", "not an xlsx (there is no xl/workbook.xml)")
    })?;
    let rels = relations(&part(bytes, &es, "xl/_rels/workbook.xml.rels")?.unwrap_or_default());
    let shared = shared_strings(&part(bytes, &es, "xl/sharedStrings.xml")?.unwrap_or_default());
    let styles = read_styles(&part(bytes, &es, "xl/styles.xml")?.unwrap_or_default());

    let mut sheets: Vec<(String, String)> = Vec::new();
    let mut date1904 = false;
    walk(&book, |n| {
        if let Node::Open { name, attrs, .. } = n {
            match name {
                "workbookPr" => {
                    let v = attr(attrs, "date1904").unwrap_or_default();
                    date1904 = v == "1" || v == "true";
                }
                "sheet" => {
                    if let (Some(name), Some(id)) = (attr(attrs, "name"), attr(attrs, "id")) {
                        if let Some(target) = rels.get(&id) {
                            sheets.push((name, target.clone()));
                        }
                    }
                }
                _ => {}
            }
        }
    });
    if sheets.is_empty() {
        return Err(tr!("xlsx にシートがありません", "the xlsx has no sheets"));
    }
    let (name, target) = match want {
        None => sheets[0].clone(),
        Some(w) => sheets.iter().find(|(n, _)| n == w).cloned().ok_or_else(|| {
            let all: Vec<&str> = sheets.iter().map(|(n, _)| n.as_str()).collect();
            tr!(
                "`{w}` というシートはありません。あるのは: {}",
                "there is no sheet called `{w}`; the workbook has: {}",
                all.join(", ")
            )
        })?,
    };
    let xml = part(bytes, &es, &target)?
        .ok_or_else(|| tr!("シート {target} が xlsx にありません", "the sheet {target} is not in the xlsx"))?;

    // Only cells that hold something are kept, so an empty row or column simply never
    // appears; `number` is where a spreadsheet stops being a CSV (dates, percentages, the
    // unit in the format).
    let number = |v: &str, style: usize| -> String {
        let (fmt, unit) = styles.get(style).cloned().unwrap_or((Fmt::Plain, String::new()));
        match fmt {
            Fmt::Date => serial_date(v, date1904).unwrap_or_else(|| v.to_string()),
            Fmt::Percent => times100(v).map(|n| format!("{n}%")).unwrap_or_else(|| v.to_string()),
            Fmt::Plain => format!("{v}{unit}"),
        }
    };

    let mut cells: BTreeMap<(usize, usize), String> = BTreeMap::new();
    let (mut row, mut col, mut style) = (0usize, 0usize, 0usize);
    let (mut ty, mut buf) = (String::new(), String::new());
    let mut taking = false;
    walk(&xml, |n| match n {
        Node::Open { name: "row", attrs, .. } => {
            row = attr(attrs, "r").and_then(|r| r.parse().ok()).unwrap_or(row + 1);
            col = 0;
        }
        Node::Open { name: "c", attrs, .. } => {
            col = attr(attrs, "r").and_then(|r| ref_col(&r)).unwrap_or(col + 1);
            ty = attr(attrs, "t").unwrap_or_default();
            style = attr(attrs, "s").and_then(|s| s.parse().ok()).unwrap_or(0);
            buf.clear();
        }
        // The cached value, and the text of an inline string. A formula's own text sits in
        // `<f>` and is not read: what the sheet *shows* is the value.
        Node::Open { name: "v" | "t", empty, .. } if !empty => taking = true,
        Node::Text(t) if taking => buf.push_str(&unescape(t)),
        Node::Close("v" | "t") => taking = false,
        Node::Close("c") => {
            let text = match ty.as_str() {
                "s" => buf.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                "inlineStr" | "str" | "e" => buf.clone(),
                "b" => if buf.trim() == "1" { "TRUE".into() } else { "FALSE".into() },
                _ => number(buf.trim(), style),
            };
            if !text.trim().is_empty() {
                cells.insert((row, col), text);
            }
        }
        _ => {}
    });
    if cells.is_empty() {
        return Err(tr!("シート `{name}` は空です", "the sheet `{name}` is empty"));
    }

    let mut cols: Vec<usize> = cells.keys().map(|(_, c)| *c).collect();
    cols.sort_unstable();
    cols.dedup();
    let mut rows: Vec<usize> = cells.keys().map(|(r, _)| *r).collect();
    rows.sort_unstable();
    rows.dedup();
    let grid = rows
        .iter()
        .map(|r| cols.iter().map(|c| cells.get(&(*r, *c)).cloned().unwrap_or_default()).collect())
        .collect();
    Ok((name, grid))
}
