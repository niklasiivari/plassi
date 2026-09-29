//! Which backups to keep: pure rules shared by the desktop and the browser.
//! A backup is named by its time, in milliseconds since 1970, zero-padded so
//! names sort in time order.

use web_time::{SystemTime, UNIX_EPOCH};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// How many backups of a project to keep.
#[derive(Clone, Copy)]
pub struct Keep {
    /// The newest this many, whatever their age.
    pub recent: usize,
    /// Plus the newest of each day for this many days back.
    pub days: u64,
}

/// Milliseconds since 1970, now.
pub fn now_stamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// A backup's name without extension, e.g. `001727000000000`.
pub fn stamp_name(stamp: u64) -> String {
    format!("{stamp:015}")
}

pub fn parse_stamp(name: &str) -> Option<u64> {
    (name.len() == 15 && name.bytes().all(|b| b.is_ascii_digit())).then(|| name.parse().ok()).flatten()
}

/// The backups to delete from `stamps`, as of `now`.
pub fn to_prune(stamps: &[u64], now: u64, keep: Keep) -> Vec<u64> {
    let mut newest_first = stamps.to_vec();
    newest_first.sort_unstable_by(|a, b| b.cmp(a));
    newest_first.dedup();
    let mut days_seen = Vec::new();
    newest_first
        .iter()
        .enumerate()
        .filter(|&(i, &stamp)| {
            if i < keep.recent {
                return false;
            }
            let day = stamp / DAY_MS;
            let recent_day = now.saturating_sub(stamp) < keep.days * DAY_MS;
            if recent_day && !days_seen.contains(&day) {
                days_seen.push(day);
                return false;
            }
            true
        })
        .map(|(_, &stamp)| stamp)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sort_in_time_order_and_parse_back() {
        let (a, b) = (stamp_name(999), stamp_name(1_727_000_000_000));
        assert!(a < b);
        assert_eq!(parse_stamp(&b), Some(1_727_000_000_000));
        assert_eq!(parse_stamp("12x"), None);
        assert_eq!(parse_stamp("notes"), None);
    }

    #[test]
    fn keeps_the_newest_and_one_per_recent_day() {
        let now = 100 * DAY_MS;
        // Hourly backups over the last 20 days.
        let stamps: Vec<u64> = (0..20 * 24).map(|h| now - h * DAY_MS / 24).collect();
        let pruned = to_prune(&stamps, now, Keep { recent: 5, days: 14 });
        let kept: Vec<u64> = stamps.iter().copied().filter(|s| !pruned.contains(s)).collect();
        assert!(stamps[..5].iter().all(|s| kept.contains(s)));
        // Older than 14 days: all gone.
        assert!(kept.iter().all(|&s| now - s < 14 * DAY_MS));
        // At most one per day beyond the newest five.
        assert!(kept.len() <= 5 + 14);
        assert!(kept.len() >= 14);
    }

    #[test]
    fn keeps_only_recent_when_days_is_zero() {
        let pruned = to_prune(&[1, 2, 3, 4, 5], 10, Keep { recent: 3, days: 0 });
        assert_eq!(pruned, vec![2, 1]);
        assert!(to_prune(&[1, 2], 10, Keep { recent: 3, days: 0 }).is_empty());
    }
}
