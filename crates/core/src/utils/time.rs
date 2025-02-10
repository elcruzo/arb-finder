use chrono::{DateTime, Duration, Utc, Timelike, Datelike};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn now_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

pub fn now_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

pub fn timestamp_to_datetime(timestamp_ms: u64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(timestamp_ms as i64).unwrap_or_else(Utc::now)
}

pub fn datetime_to_timestamp(dt: DateTime<Utc>) -> u64 {
    dt.timestamp_millis() as u64
}

pub fn parse_iso8601(s: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}

pub fn format_iso8601(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

pub fn is_market_hours(_dt: DateTime<Utc>) -> bool {
    true
}

pub fn next_market_open(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt + Duration::hours(1)
}

pub fn time_until_market_close(dt: DateTime<Utc>) -> Duration {
    Duration::hours(24) - Duration::nanoseconds(dt.timestamp_subsec_nanos() as i64)
}

pub fn duration_between(start: DateTime<Utc>, end: DateTime<Utc>) -> Duration {
    end.signed_duration_since(start)
}

pub fn add_business_days(dt: DateTime<Utc>, days: i32) -> DateTime<Utc> {
    let mut result = dt;
    let mut remaining_days = days;

    while remaining_days > 0 {
        result = result + Duration::days(1);
        let weekday = result.weekday();
        
        if weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun {
            remaining_days -= 1;
        }
    }

    result
}

pub fn is_weekend(dt: DateTime<Utc>) -> bool {
    let weekday = dt.weekday();
    weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun
}

pub fn round_to_nearest_second(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_nanosecond(0).unwrap_or(dt)
}

pub fn round_to_nearest_minute(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_second(0).unwrap_or(dt).with_nanosecond(0).unwrap_or(dt)
}

pub fn start_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
}

pub fn end_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_conversion() {
        let now = Utc::now();
        let timestamp = datetime_to_timestamp(now);
        let converted = timestamp_to_datetime(timestamp);
        
        assert!((now.timestamp_millis() - converted.timestamp_millis()).abs() < 1000);
    }

    #[test]
    fn test_iso8601_parsing() {
        let iso_string = "2023-01-01T12:00:00Z";
        let parsed = parse_iso8601(iso_string).unwrap();
        let formatted = format_iso8601(parsed);
        
        assert_eq!(formatted, iso_string);
    }

    #[test]
    fn test_business_days() {
        let friday = DateTime::parse_from_rfc3339("2023-01-06T12:00:00Z").unwrap().with_timezone(&Utc);
        let next_business_day = add_business_days(friday, 1);
        
        assert_eq!(next_business_day.weekday(), chrono::Weekday::Mon);
    }

    #[test]
    fn test_weekend_detection() {
        let saturday = DateTime::parse_from_rfc3339("2023-01-07T12:00:00Z").unwrap().with_timezone(&Utc);
        let monday = DateTime::parse_from_rfc3339("2023-01-09T12:00:00Z").unwrap().with_timezone(&Utc);
        
        assert!(is_weekend(saturday));
        assert!(!is_weekend(monday));
    }
}