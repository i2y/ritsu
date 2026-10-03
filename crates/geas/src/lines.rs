//! Sets of line numbers, written as ranges: `1-4,6,9-11`. The record keeps every
//! file's code and every claim's run lines this way, so that a person can read a
//! line of it and a diff of two records stays small.

use std::collections::BTreeSet;

pub type Lines = BTreeSet<u32>;

/// `1-4,6,9-11`; the empty set is the empty string.
pub fn to_ranges(lines: &Lines) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut it = lines.iter().copied().peekable();
    while let Some(first) = it.next() {
        let mut last = first;
        while it.peek() == Some(&(last + 1)) {
            last += 1;
            it.next();
        }
        out.push(if first == last { first.to_string() } else { format!("{first}-{last}") });
    }
    out.join(",")
}

/// The set a string of ranges names. Lines start at 1, and a range runs upwards.
pub fn from_ranges(s: &str) -> Result<Lines, String> {
    let mut out = Lines::new();
    if s.is_empty() {
        return Ok(out);
    }
    let num = |t: &str| -> Result<u32, String> {
        match t.parse::<u32>() {
            Ok(n) if n > 0 && t.chars().all(|c| c.is_ascii_digit()) => Ok(n),
            _ => Err(format!("`{t}` is not a line number")),
        }
    };
    for part in s.split(',') {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (num(a)?, num(b)?);
                if a > b {
                    return Err(format!("the range `{part}` runs downwards"));
                }
                out.extend(a..=b);
            }
            None => {
                out.insert(num(part)?);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[u32]) -> Lines {
        v.iter().copied().collect()
    }

    #[test]
    fn ranges_both_ways() {
        for (v, s) in [
            (vec![], ""),
            (vec![7], "7"),
            (vec![1, 2, 3, 4, 6, 9, 10, 11], "1-4,6,9-11"),
            (vec![1, 3, 5], "1,3,5"),
            (vec![2, 3], "2-3"),
        ] {
            assert_eq!(to_ranges(&set(&v)), s);
            assert_eq!(from_ranges(s).unwrap(), set(&v));
        }
    }

    #[test]
    fn ranges_that_do_not_read() {
        assert!(from_ranges("0").is_err());
        assert!(from_ranges("4-2").is_err());
        assert!(from_ranges("1,,2").is_err());
        assert!(from_ranges("x").is_err());
        assert!(from_ranges("+3").is_err());
        assert!(from_ranges("1-").is_err());
        // written out of order or overlapping, a set is still a set
        assert_eq!(from_ranges("5,1-3,2").unwrap(), set(&[1, 2, 3, 5]));
    }
}
