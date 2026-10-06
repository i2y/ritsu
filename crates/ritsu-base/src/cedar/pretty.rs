//! Wadler's pretty printer as the `pretty` crate 0.12.5 renders it (`RcDoc` and
//! `render::best`), which `cedar format` lays its policies out with: a group is flat when its
//! content fits on the line together with what follows it up to the next possible line break;
//! a line break takes the indentation of the document after it; text is measured in bytes when
//! it is ASCII and in display columns otherwise.

use std::rc::Rc;

#[derive(Debug)]
pub(crate) enum D {
    Nil,
    /// The text and its width.
    Text(String, usize),
    Hardline,
    /// `FlatAlt(broken, flat)`.
    FlatAlt(Doc, Doc),
    Append(Doc, Doc),
    Group(Doc),
    Nest(isize, Doc),
}

pub(crate) type Doc = Rc<D>;

pub(crate) fn nil() -> Doc {
    Rc::new(D::Nil)
}

pub(crate) fn text(s: impl Into<String>) -> Doc {
    let s: String = s.into();
    if s.is_empty() {
        return nil();
    }
    let w = if s.is_ascii() { s.len() } else { display_width(&s) };
    Rc::new(D::Text(s, w))
}

pub(crate) fn space() -> Doc {
    text(" ")
}

pub(crate) fn hardline() -> Doc {
    Rc::new(D::Hardline)
}

/// A space when flat, a line break when not.
pub(crate) fn line() -> Doc {
    Rc::new(D::FlatAlt(hardline(), space()))
}

/// Nothing when flat, a line break when not.
pub(crate) fn line_() -> Doc {
    Rc::new(D::FlatAlt(hardline(), nil()))
}

pub(crate) fn append(a: Doc, b: Doc) -> Doc {
    match (&*a, &*b) {
        (D::Nil, _) => b,
        (_, D::Nil) => a,
        _ => Rc::new(D::Append(a, b)),
    }
}

pub(crate) fn group(d: Doc) -> Doc {
    match &*d {
        D::Group(_) | D::Nil | D::Text(..) => d,
        _ => Rc::new(D::Group(d)),
    }
}

pub(crate) fn nest(d: Doc, off: isize) -> Doc {
    if matches!(&*d, D::Nil) || off == 0 {
        return d;
    }
    Rc::new(D::Nest(off, d))
}

/// `docs` with `sep` between them.
pub(crate) fn intersperse(docs: impl IntoIterator<Item = Doc>, sep: Doc) -> Doc {
    let mut out = nil();
    for (i, d) in docs.into_iter().enumerate() {
        if i > 0 {
            out = append(out, sep.clone());
        }
        out = append(out, d);
    }
    out
}

/// A builder that reads like the crate's: `a.app(b).grp()`.
pub(crate) trait DocExt {
    fn app(self, other: Doc) -> Doc;
    fn grp(self) -> Doc;
    fn nst(self, off: isize) -> Doc;
}

impl DocExt for Doc {
    fn app(self, other: Doc) -> Doc {
        append(self, other)
    }
    fn grp(self) -> Doc {
        group(self)
    }
    fn nst(self, off: isize) -> Doc {
        nest(self, off)
    }
}

/// The columns a text takes, as `unicode-width` counts the characters a policy usually holds:
/// two for East Asian wide and full-width characters and for emoji, none for combining marks,
/// zero-width spaces and variation selectors, one for the rest.
pub(crate) fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if matches!(u, 0x0300..=0x036F | 0x200B..=0x200F | 0xFE00..=0xFE0F | 0x20D0..=0x20FF) {
                0
            } else if matches!(u,
                0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF |
                0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 |
                0xFFE0..=0xFFE6 | 0x1F300..=0x1F64F | 0x1F680..=0x1F6FF | 0x1F900..=0x1F9FF | 0x1FA70..=0x1FAFF |
                0x20000..=0x2FFFD | 0x30000..=0x3FFFD)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Break,
    Flat,
}

/// Render as `RcDoc::render(width, out)` does.
pub(crate) fn render(doc: &Doc, width: usize) -> String {
    let mut out = String::new();
    let mut pos = 0usize;
    let mut bcmds: Vec<(usize, Mode, Doc)> = vec![(0, Mode::Break, doc.clone())];
    while let Some(first) = bcmds.pop() {
        let mut cmd = first;
        loop {
            let (ind, mode, d) = (cmd.0, cmd.1, cmd.2.clone());
            match &*d {
                D::Nil => {}
                D::Append(l, r) => {
                    // what comes after waits on the stack; the leftmost part goes on
                    bcmds.push((ind, mode, r.clone()));
                    let mut left = l.clone();
                    while let D::Append(ll, lr) = &*left.clone() {
                        bcmds.push((ind, mode, lr.clone()));
                        left = ll.clone();
                    }
                    cmd = (ind, mode, left);
                    continue;
                }
                D::FlatAlt(b, f) => {
                    cmd = (ind, mode, if mode == Mode::Break { b.clone() } else { f.clone() });
                    continue;
                }
                D::Group(inner) => {
                    let m = if mode == Mode::Break && fits(inner, pos, width, &bcmds) { Mode::Flat } else { mode };
                    cmd = (ind, m, inner.clone());
                    continue;
                }
                D::Nest(off, inner) => {
                    let ni = if *off >= 0 { ind.saturating_add(*off as usize) } else { ind.saturating_sub(off.unsigned_abs()) };
                    cmd = (ni, mode, inner.clone());
                    continue;
                }
                D::Hardline => {
                    // the new line takes the indentation of the document after it
                    if let Some(next) = bcmds.pop() {
                        out.push('\n');
                        out.push_str(&" ".repeat(next.0));
                        pos = next.0;
                        cmd = next;
                        continue;
                    }
                    out.push('\n');
                    out.push_str(&" ".repeat(ind));
                    pos = ind;
                }
                D::Text(s, w) => {
                    out.push_str(s);
                    pos += w;
                }
            }
            break;
        }
    }
    out
}

/// Whether `next`, flat, and what follows it (read as broken) reach a line break before the
/// width runs out.
fn fits(next: &Doc, mut pos: usize, width: usize, bcmds: &[(usize, Mode, Doc)]) -> bool {
    let mut bidx = bcmds.len();
    let mut fcmds: Vec<Doc> = vec![next.clone()];
    let mut mode = Mode::Flat;
    loop {
        let mut d = match fcmds.pop() {
            Some(d) => d,
            None => {
                if bidx == 0 {
                    return true;
                }
                bidx -= 1;
                mode = Mode::Break;
                bcmds[bidx].2.clone()
            }
        };
        loop {
            match &*d.clone() {
                D::Nil => {}
                D::Append(l, r) => {
                    fcmds.push(r.clone());
                    let mut left = l.clone();
                    while let D::Append(ll, lr) = &*left.clone() {
                        fcmds.push(lr.clone());
                        left = ll.clone();
                    }
                    d = left;
                    continue;
                }
                D::Hardline => return mode == Mode::Break,
                D::Text(_, w) => {
                    pos += w;
                    if pos > width {
                        return false;
                    }
                }
                D::FlatAlt(b, f) => {
                    d = if mode == Mode::Break { b.clone() } else { f.clone() };
                    continue;
                }
                D::Nest(_, inner) | D::Group(inner) => {
                    d = inner.clone();
                    continue;
                }
            }
            break;
        }
    }
}
