//! The values pins hold (DESIGN §12), read as the spec is read: a time written as
//! RFC 3339, and a locale written as a language and a region.

/// Seconds since 1970-01-01T00:00:00Z of a time written as RFC 3339
/// (`2026-08-29T09:00:00+09:00`, `2026-08-29T00:00:00.5Z`); None for anything else.
/// `T` and `Z` may be lower case, as RFC 3339 allows.
pub fn rfc3339(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> {
        let part = b.get(from..from + len)?;
        part.iter().all(u8::is_ascii_digit).then(|| std::str::from_utf8(part).ok()?.parse().ok())?
    };
    let at = |i: usize, c: u8| b.get(i).is_some_and(|x| x.eq_ignore_ascii_case(&c));
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    if !(at(4, b'-') && at(7, b'-') && at(10, b'T') && at(13, b':') && at(16, b':')) {
        return None;
    }
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let mut i = 19;
    let mut fraction = 0.0;
    if at(i, b'.') {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
        fraction = format!("0.{}", std::str::from_utf8(&b[start..i]).ok()?).parse().ok()?;
    }
    let offset = if at(i, b'Z') {
        i += 1;
        0
    } else if at(i, b'+') || at(i, b'-') {
        let sign = if at(i, b'-') { -1 } else { 1 };
        let (h, m) = (num(i + 1, 2)?, num(i + 4, 2)?);
        if !at(i + 3, b':') || h > 23 || m > 59 {
            return None;
        }
        i += 6;
        sign * (h * 3600 + m * 60)
    } else {
        return None;
    };
    if i != b.len() {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month[(month - 1) as usize] {
        return None;
    }
    // RFC 3339 allows a leap second, 60
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some((days * 86_400 + hour * 3600 + minute * 60 + second - offset) as f64 + fraction)
}

/// Days from 1970-01-01 to a date of the proleptic Gregorian calendar (Howard
/// Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Whether a locale is written as a language and a region, as both `LANG` and
/// Chrome take it: two or three lower-case letters, `-`, two upper-case letters
/// (`ja-JP`, `de-DE`).
pub fn locale(s: &str) -> bool {
    let Some((lang, region)) = s.split_once('-') else {
        return false;
    };
    (2..=3).contains(&lang.len())
        && lang.bytes().all(|c| c.is_ascii_lowercase())
        && region.len() == 2
        && region.bytes().all(|c| c.is_ascii_uppercase())
}

/// What a locale written another way probably means: `ja_JP.UTF-8` is `ja-JP`.
pub fn locale_meant(s: &str) -> Option<String> {
    let base = s.split('.').next()?.replace('_', "-");
    let (lang, region) = base.split_once('-')?;
    let guess = format!("{}-{}", lang.to_ascii_lowercase(), region.to_ascii_uppercase());
    locale(&guess).then_some(guess)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_written_as_rfc_3339() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(rfc3339("2026-08-29T00:00:00Z"), Some(1_787_961_600.0));
        assert_eq!(rfc3339("2026-08-29T09:00:00+09:00"), Some(1_787_961_600.0));
        assert_eq!(rfc3339("2026-08-28T15:00:00-09:00"), Some(1_787_961_600.0));
        assert_eq!(rfc3339("2026-08-29t00:00:00.25z"), Some(1_787_961_600.25));
        assert_eq!(rfc3339("2024-02-29T00:00:00Z"), Some(1_709_164_800.0));
        for bad in [
            "2026-08-29",
            "2026-08-29 00:00:00Z",
            "2026-08-29T00:00:00",
            "2026-13-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "2026-08-29T24:00:00Z",
            "2026-08-29T00:00:00+0900",
            "2026-08-29T00:00:00.Z",
            "2026-08-29T00:00:00Zx",
            "29/08/2026",
        ] {
            assert_eq!(rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn locales() {
        assert!(locale("ja-JP") && locale("de-DE") && locale("fil-PH"));
        for bad in ["ja", "ja_JP", "ja-jp", "JA-JP", "ja-JP.UTF-8", "zh-Hant-TW", ""] {
            assert!(!locale(bad), "{bad}");
        }
        assert_eq!(locale_meant("ja_JP.UTF-8").as_deref(), Some("ja-JP"));
        assert_eq!(locale_meant("de-de").as_deref(), Some("de-DE"));
        assert_eq!(locale_meant("ja"), None);
    }
}
