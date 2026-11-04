//! NFC and NFKC (Unicode Standard Annex #15). Rust compares identifiers in NFC, and Python in NFKC,
//! so two names a language of ritsu keeps apart can be one identifier in the code of a target, which
//! chobo's W108 says. The languages themselves do not normalize a name. The tables are the Unicode
//! Character Database's (src/normal_table.rs); a character the database has not assigned is left as
//! it is.

use crate::normal_table::{COMBINING, COMPOSITIONS, DECOMPOSED, DECOMPOSITIONS};

// The syllables of Hangul decompose and compose by arithmetic (The Unicode Standard, 3.12).
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

/// A name in NFC: the form Rust compares identifiers in.
pub fn nfc(s: &str) -> String {
    normalized(s, false)
}

/// A name in NFKC: the form Python compares identifiers in.
pub fn nfkc(s: &str) -> String {
    normalized(s, true)
}

fn normalized(s: &str, compatibility: bool) -> String {
    if s.is_ascii() {
        return s.to_string();
    }
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        decompose(c as u32, compatibility, &mut out);
    }
    reorder(&mut out);
    compose(&out).into_iter().filter_map(char::from_u32).collect()
}

fn decompose(u: u32, compatibility: bool, out: &mut Vec<u32>) {
    if (S_BASE..S_BASE + S_COUNT).contains(&u) {
        let i = u - S_BASE;
        out.push(L_BASE + i / N_COUNT);
        out.push(V_BASE + (i % N_COUNT) / T_COUNT);
        if i % T_COUNT != 0 {
            out.push(T_BASE + i % T_COUNT);
        }
        return;
    }
    let Ok(k) = DECOMPOSITIONS.binary_search_by_key(&u, |e| e.0) else {
        out.push(u);
        return;
    };
    let packed = DECOMPOSITIONS[k].1;
    if packed & 1 == 1 && !compatibility {
        out.push(u);
        return;
    }
    let (at, n) = ((packed >> 6) as usize, ((packed >> 1) & 31) as usize);
    for x in &DECOMPOSED[at..at + n] {
        decompose(*x, compatibility, out);
    }
}

fn class(u: u32) -> u8 {
    COMBINING
        .binary_search_by(|&(lo, hi, _)| {
            if hi < u {
                std::cmp::Ordering::Less
            } else if lo > u {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .map_or(0, |k| COMBINING[k].2)
}

/// The canonical ordering: each run of characters of a class but 0, in the order of their classes,
/// those of one class as they stand.
fn reorder(v: &mut [u32]) {
    let mut i = 0;
    while i < v.len() {
        if class(v[i]) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < v.len() && class(v[i]) != 0 {
            i += 1;
        }
        v[start..i].sort_by_key(|u| class(*u));
    }
}

fn pair(a: u32, b: u32) -> Option<u32> {
    if (L_BASE..L_BASE + L_COUNT).contains(&a) && (V_BASE..V_BASE + V_COUNT).contains(&b) {
        return Some(S_BASE + ((a - L_BASE) * V_COUNT + (b - V_BASE)) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&a) && (a - S_BASE) % T_COUNT == 0 && (T_BASE + 1..T_BASE + T_COUNT).contains(&b) {
        return Some(a + (b - T_BASE));
    }
    COMPOSITIONS.binary_search_by_key(&(a, b), |e| (e.0, e.1)).ok().map(|k| COMPOSITIONS[k].2)
}

/// Canonical composition: each character joins the last starter before it, unless a character
/// between them blocks it (one of a class as high or higher, or a starter).
fn compose(v: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(v.len());
    let mut starter: Option<usize> = None;
    let mut last = 0u8;
    for &c in v {
        let cc = class(c);
        if let Some(s) = starter {
            let adjacent = s + 1 == out.len();
            if (adjacent || (last != 0 && last < cc))
                && let Some(made) = pair(out[s], c)
            {
                out[s] = made;
                continue;
            }
        }
        if cc == 0 {
            starter = Some(out.len());
        }
        last = cc;
        out.push(c);
    }
    out
}
