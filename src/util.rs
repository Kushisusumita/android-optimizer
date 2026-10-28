//! Small standard-library helpers: reading numbers out of command output, and dates without a crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// The integer at the start of the text, after leading spaces ("1800000 kHz" -> 1800000).
pub fn leading_number(text: &str) -> Option<i64> {
    let digits: String = text.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The first integer after `label` on any line ("MemTotal:   3800000 kB", "MemTotal:" -> 3800000).
pub fn num_after(text: &str, label: &str) -> Option<i64> {
    text.lines().find_map(|line| leading_number(&line[line.find(label)? + label.len()..]))
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Civil date (year, month, day) from days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
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

/// Compact UTC timestamp "YYYY-MM-DD_HH-MM-SS" for report folder names.
pub fn now_utc_compact() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", tod / 3600, (tod % 3600) / 60, tod % 60)
}

/// Parse "YYYY-MM-DD" into days since epoch.
pub fn parse_ymd_to_days(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let y = it.next()?.parse::<i64>().ok()?;
    let m = it.next()?.parse::<i64>().ok()?;
    let d = it.next()?.trim().parse::<i64>().ok()?;
    Some(days_from_civil(y, m, d))
}

/// Replace every character that is not alphanumeric or '-' with '-' (for safe folder names).
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_out_of_command_output() {
        assert_eq!(leading_number(" 1800000 kHz"), Some(1_800_000));
        assert_eq!(leading_number("abc"), None);
        assert_eq!(num_after("MemTotal:        3800000 kB", "MemTotal:"), Some(3_800_000));
        assert_eq!(num_after("nothing", "MemTotal:"), None);
    }

    #[test]
    fn dates_and_names() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(parse_ymd_to_days("2024-03-01").unwrap() - parse_ymd_to_days("2024-02-28").unwrap(), 2);
        assert_eq!(slug("Redmi Note 13 Pro+"), "Redmi-Note-13-Pro");
        let stamp = now_utc_compact();
        assert_eq!((stamp.len(), stamp.as_bytes()[10]), (19, b'_'));
    }
}
