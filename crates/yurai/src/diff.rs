//! The diff a mark shows (DESIGN 4.3, 16): the lines of what was looked at against the lines
//! of what is there now, as unified hunks with two lines of context. Past 40 lines the rest
//! is counted, not shown.

use crate::diag::DiffLine;

/// At most this many lines of a diff are shown.
pub const MAX_LINES: usize = 40;

/// The edit script of `a` into `b`: `' '`, `'-'` and `'+'` with the line, by the longest
/// common subsequence of lines. The common start and end are taken off first, so a change
/// in a long file costs only what is between.
fn script<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<(char, &'a str)> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut out: Vec<(char, &str)> = a[..pre].iter().map(|l| (' ', *l)).collect();
    // The table of LCS lengths of the suffixes; the middle is small unless two files differ
    // throughout, and then the table is bounded by refusing to align past a size.
    let (n, m) = (am.len(), bm.len());
    if n * m > 4_000_000 {
        out.extend(am.iter().map(|l| ('-', *l)));
        out.extend(bm.iter().map(|l| ('+', *l)));
    } else {
        let mut t = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * (m + 1) + j] = if am[i] == bm[j] { t[(i + 1) * (m + 1) + j + 1] + 1 } else { t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if am[i] == bm[j] {
                out.push((' ', am[i]));
                i += 1;
                j += 1;
            } else if t[(i + 1) * (m + 1) + j] >= t[i * (m + 1) + j + 1] {
                out.push(('-', am[i]));
                i += 1;
            } else {
                out.push(('+', bm[j]));
                j += 1;
            }
        }
        out.extend(am[i..].iter().map(|l| ('-', *l)));
        out.extend(bm[j..].iter().map(|l| ('+', *l)));
    }
    out.extend(a[a.len() - suf..].iter().map(|l| (' ', *l)));
    out
}

/// The unified diff of two texts, line by line: hunks headed `@@ -a,n +b,m @@`, two lines
/// of context around each change. Empty when the texts have the same lines. `more` is how
/// many lines past [`MAX_LINES`] were left out.
pub fn unified(old: &str, new: &str) -> (Vec<DiffLine>, usize) {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let s = script(&a, &b);
    if s.iter().all(|(op, _)| *op == ' ') {
        return (vec![], 0);
    }
    // Which lines of the script are shown: the changes and two lines around each.
    let ctx = 2;
    let mut show = vec![false; s.len()];
    for (k, (op, _)) in s.iter().enumerate() {
        if *op != ' ' {
            show[k.saturating_sub(ctx)..(k + ctx + 1).min(s.len())].fill(true);
        }
    }
    let mut out = Vec::new();
    let (mut ai, mut bi) = (1usize, 1usize);
    let mut k = 0;
    while k < s.len() {
        if !show[k] {
            match s[k].0 {
                '-' => ai += 1,
                '+' => bi += 1,
                _ => {
                    ai += 1;
                    bi += 1;
                }
            }
            k += 1;
            continue;
        }
        let start = k;
        while k < s.len() && show[k] {
            k += 1;
        }
        let hunk = &s[start..k];
        let na = hunk.iter().filter(|(op, _)| *op != '+').count();
        let nb = hunk.iter().filter(|(op, _)| *op != '-').count();
        let a0 = if na == 0 { ai - 1 } else { ai };
        let b0 = if nb == 0 { bi - 1 } else { bi };
        out.push(DiffLine { op: '@', text: format!("@@ -{a0},{na} +{b0},{nb} @@") });
        for (op, l) in hunk {
            out.push(DiffLine { op: *op, text: l.to_string() });
        }
        ai += na;
        bi += nb;
    }
    let total = out.len();
    if total > MAX_LINES {
        out.truncate(MAX_LINES);
        (out, total - MAX_LINES)
    } else {
        (out, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(d: &[DiffLine]) -> Vec<String> {
        d.iter().map(|l| if l.op == '@' { l.text.clone() } else { format!("{}{}", l.op, l.text) }).collect()
    }

    #[test]
    fn one_line_changed_in_the_middle() {
        let old = "a\nb\nc\nd\ne\nf\ng\n";
        let new = "a\nb\nc\nD\ne\nf\ng\n";
        let (d, more) = unified(old, new);
        assert_eq!(more, 0);
        assert_eq!(shown(&d), ["@@ -2,5 +2,5 @@", " b", " c", "-d", "+D", " e", " f"]);
    }

    #[test]
    fn two_hunks_an_addition_and_nothing() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
        let new = "0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n";
        let (d, _) = unified(old, new);
        assert_eq!(shown(&d), ["@@ -1,2 +1,3 @@", "+0", " 1", " 2", "@@ -8,3 +9,2 @@", " 8", " 9", "-10"]);
        assert_eq!(unified("x\n", "x\n").0, vec![]);
    }

    #[test]
    fn a_long_diff_is_cut() {
        let old: String = (0..100).map(|i| format!("{i}\n")).collect();
        let new: String = (0..100).map(|i| format!("{}\n", i * 7)).collect();
        let (d, more) = unified(&old, &new);
        assert_eq!(d.len(), MAX_LINES);
        assert!(more > 0);
    }
}
