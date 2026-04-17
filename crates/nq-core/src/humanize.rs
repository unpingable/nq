//! Operator-legible formatting helpers.
//!
//! NQ's default rendering policy: seconds are for machines and forensics,
//! not for the primary operator surface. Precise values stay in the
//! structured store and in detail drawers; the main surface uses scaled
//! human durations and, where applicable, NQ's native unit (generations)
//! as a co-primary label.
//!
//! See `project_operator_intent_model.md` in project memory for the
//! framing; the constitutional rule is that `5358s` is "technically
//! precise and spiritually useless."

/// Format a duration in seconds as a short operator-legible string.
///
/// Boundaries:
/// - `< 60s`           → `Ns`    (e.g. `47s`)
/// - `< 60m`           → `Nm`    (e.g. `8m`)       (59s rounds down to 0m; use `>=60` to enter this band)
/// - `< 24h`           → `Nh Mm` (e.g. `1h 29m`, `2h`)  (omits minutes when zero)
/// - `>= 24h`          → `Nd Mh` (e.g. `2d 4h`, `3d`)   (omits hours when zero)
///
/// Negative input is treated as zero; extremely large values still use
/// days (no weeks / months scale because operators reason poorly about
/// "5w" in an incident context).
pub fn humanize_duration_s(secs: i64) -> String {
    if secs < 60 {
        let s = secs.max(0);
        return format!("{s}s");
    }
    if secs < 3600 {
        let m = secs / 60;
        return format!("{m}m");
    }
    if secs < 86_400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            return format!("{h}h");
        }
        return format!("{h}h {m}m");
    }
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    if h == 0 {
        format!("{d}d")
    } else {
        format!("{d}d {h}h")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_minute() {
        assert_eq!(humanize_duration_s(0), "0s");
        assert_eq!(humanize_duration_s(1), "1s");
        assert_eq!(humanize_duration_s(47), "47s");
        assert_eq!(humanize_duration_s(59), "59s");
    }

    #[test]
    fn minutes_below_hour() {
        assert_eq!(humanize_duration_s(60), "1m");
        assert_eq!(humanize_duration_s(8 * 60), "8m");
        assert_eq!(humanize_duration_s(59 * 60), "59m");
        // Seconds are dropped inside the minute band.
        assert_eq!(humanize_duration_s(8 * 60 + 42), "8m");
    }

    #[test]
    fn hours_below_day() {
        assert_eq!(humanize_duration_s(3600), "1h");
        assert_eq!(humanize_duration_s(2 * 3600), "2h");
        assert_eq!(humanize_duration_s(3600 + 29 * 60), "1h 29m");
        // Seconds are dropped inside the hour band; minute remainder only.
        assert_eq!(humanize_duration_s(5358), "1h 29m");
    }

    #[test]
    fn days_and_up() {
        assert_eq!(humanize_duration_s(86_400), "1d");
        assert_eq!(humanize_duration_s(86_400 + 4 * 3600), "1d 4h");
        assert_eq!(humanize_duration_s(2 * 86_400 + 4 * 3600 + 15 * 60), "2d 4h");
        // Seven-day stretches stay in days rather than using weeks.
        assert_eq!(humanize_duration_s(7 * 86_400), "7d");
    }

    #[test]
    fn negative_treated_as_zero() {
        assert_eq!(humanize_duration_s(-1), "0s");
        assert_eq!(humanize_duration_s(i64::MIN), "0s");
    }
}
