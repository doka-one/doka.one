use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};

pub(crate) fn format_date(iso_date: &str) -> String {
    // Parse the ISO date and format it to "DD Month YYYY"
    let date = NaiveDate::parse_from_str(iso_date, "%Y-%m-%d");
    match date {
        Ok(d) => format!("{} {} {}", d.day(), d.month(), d.year()),
        Err(_) => "Invalid date".to_string(),
    }
}

pub(crate) fn format_date_in_timezone(iso_date_time: &str, timezone_offset: i32) -> String {
    // Parse the ISO 8601 date string into a DateTime object
    let Ok(dt) = DateTime::parse_from_rfc3339(iso_date_time) else {
        //expect("Invalid ISO date");
        return "Invalid date".to_string();
    };

    // Apply the desired timezone offset (in hours)
    let offset = FixedOffset::east(timezone_offset * 3600); // timezone_offset is in hours
    let dt_in_timezone = dt.with_timezone(&offset);

    // Extract the components of the formatted date
    let month = dt_in_timezone.month0() + 1; // months in chrono are 0-indexed
    let day = dt_in_timezone.day();
    let year = dt_in_timezone.year();
    let hour = dt_in_timezone.hour();
    let minute = dt_in_timezone.minute();

    // Format the date part (e.g., "October, 12th, 2024")
    let month_name = dt_in_timezone.format("%B").to_string(); // Full month name
    let day_suffix = match day {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    };

    // Format the time part (e.g., "6:51")
    let time_formatted = format!("{:02}:{:02}", hour, minute);

    // Final formatted output
    format!(
        "{}, {}{}, {}  {}",
        month_name, day, day_suffix, year, time_formatted
    )
}
