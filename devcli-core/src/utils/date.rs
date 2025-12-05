use chrono::{NaiveDate, Utc};

/// Formats a date string (YYYYMMDD) into a smart human-readable format
/// - Today -> "Today"
/// - Yesterday -> "Yesterday"
/// - 2-3 days ago -> "X days ago"
/// - Older -> "YYYY-MM-DD"
/// - Invalid -> Original string
pub fn format_smart_date(date_str: &str) -> String {
    if date_str.len() != 8 {
        return date_str.to_string();
    }

    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
        let today = Utc::now().naive_utc().date();
        let diff = today.signed_duration_since(date).num_days();

        match diff {
            0 => "Today".to_string(),
            1 => "Yesterday".to_string(),
            2..=3 => format!("{} days ago", diff),
            _ => date.format("%Y-%m-%d").to_string(),
        }
    } else {
        date_str.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_format_smart_date() {
        let today = Utc::now().naive_utc().date();

        let today_str = today.format("%Y%m%d").to_string();
        assert_eq!(format_smart_date(&today_str), "Today");

        let yesterday = today - Duration::days(1);
        let yesterday_str = yesterday.format("%Y%m%d").to_string();
        assert_eq!(format_smart_date(&yesterday_str), "Yesterday");

        let two_days_ago = today - Duration::days(2);
        let two_days_ago_str = two_days_ago.format("%Y%m%d").to_string();
        assert_eq!(format_smart_date(&two_days_ago_str), "2 days ago");

        let three_days_ago = today - Duration::days(3);
        let three_days_ago_str = three_days_ago.format("%Y%m%d").to_string();
        assert_eq!(format_smart_date(&three_days_ago_str), "3 days ago");

        let four_days_ago = today - Duration::days(4);
        let four_days_ago_str = four_days_ago.format("%Y%m%d").to_string();
        // Should be YYYY-MM-DD
        let expected = four_days_ago.format("%Y-%m-%d").to_string();
        assert_eq!(format_smart_date(&four_days_ago_str), expected);

        // Test invalid
        assert_eq!(format_smart_date("invalid"), "invalid");
        assert_eq!(format_smart_date("20251345"), "20251345"); // Invalid date
    }
}
