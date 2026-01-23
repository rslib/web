//! Date module (rs.date)

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use mlua::{Lua, Result, Table, Value};

/// Parse a date/datetime from various input formats
fn parse_datetime(value: &Value) -> Option<NaiveDateTime> {
    match value {
        // Unix timestamp (seconds)
        Value::Integer(ts) => DateTime::from_timestamp(*ts, 0).map(|dt| dt.naive_utc()),
        Value::Number(ts) => DateTime::from_timestamp(*ts as i64, 0).map(|dt| dt.naive_utc()),

        // String formats
        Value::String(s) => {
            let s = s.to_str().ok()?.to_string();
            let s = s.as_str();
            // Try datetime formats first
            let datetime_formats = [
                "%Y-%m-%dT%H:%M:%S%.fZ", // ISO 8601 with Z
                "%Y-%m-%dT%H:%M:%SZ",    // ISO 8601 with Z (no frac)
                "%Y-%m-%dT%H:%M:%S%.f",  // ISO 8601 local
                "%Y-%m-%dT%H:%M:%S",     // ISO 8601 local (no frac)
                "%Y-%m-%d %H:%M:%S%.f",  // Space separated with frac
                "%Y-%m-%d %H:%M:%S",     // Space separated
                "%Y-%m-%d %H:%M",        // No seconds
                "%d/%m/%Y %H:%M:%S",     // European with time
                "%m/%d/%Y %H:%M:%S",     // US with time
            ];
            for fmt in datetime_formats {
                if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
                    return Some(dt);
                }
            }
            // Try date-only formats (default to midnight)
            let date_formats = ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d"];
            for fmt in date_formats {
                if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
                    return d.and_hms_opt(0, 0, 0);
                }
            }
            None
        }

        // Table format {year, month, day, hour?, min?, sec?}
        Value::Table(t) => {
            let year: i32 = t.get("year").unwrap_or(2000);
            let month: u32 = t.get("month").unwrap_or(1);
            let day: u32 = t.get("day").unwrap_or(1);
            let hour: u32 = t.get("hour").unwrap_or(0);
            let min: u32 = t.get("min").unwrap_or(0);
            let sec: u32 = t.get("sec").unwrap_or(0);
            NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, min, sec)
        }

        _ => None,
    }
}

/// Create datetime result table
fn create_datetime_table(lua: &Lua, dt: NaiveDateTime) -> Result<Table> {
    let result = lua.create_table()?;
    result.set("year", dt.year())?;
    result.set("month", dt.month())?;
    result.set("day", dt.day())?;
    result.set("hour", dt.hour())?;
    result.set("min", dt.minute())?;
    result.set("sec", dt.second())?;
    // Also include weekday (1=Monday, 7=Sunday)
    result.set("weekday", dt.weekday().number_from_monday())?;
    // Day of year (1-366)
    result.set("yday", dt.ordinal())?;
    Ok(result)
}

pub fn create_module(lua: &Lua) -> Result<Table> {
    let date = lua.create_table()?;

    // now() - Get current Unix timestamp
    let now_fn = lua.create_function(|_, ()| Ok(Utc::now().timestamp()))?;
    date.set("now", now_fn)?;

    // from_timestamp(ts) - Convert Unix timestamp to table
    let from_timestamp_fn =
        lua.create_function(|lua, ts: i64| match DateTime::from_timestamp(ts, 0) {
            Some(dt) => {
                let result = create_datetime_table(lua, dt.naive_utc())?;
                Ok(Value::Table(result))
            }
            None => Ok(Value::Nil),
        })?;
    date.set("from_timestamp", from_timestamp_fn)?;

    // to_timestamp(date) - Convert date/datetime to Unix timestamp
    let to_timestamp_fn =
        lua.create_function(|_, date_val: Value| match parse_datetime(&date_val) {
            Some(dt) => Ok(Value::Integer(Utc.from_utc_datetime(&dt).timestamp())),
            None => Ok(Value::Nil),
        })?;
    date.set("to_timestamp", to_timestamp_fn)?;

    // format(date, format) - Format a date/datetime string
    let format_fn = lua.create_function(|lua, (date_val, format): (Value, String)| {
        match parse_datetime(&date_val) {
            Some(dt) => {
                let formatted = dt.format(&format).to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            None => Ok(Value::Nil),
        }
    })?;
    date.set("format", format_fn)?;

    // parse(str, format?) - Parse date/datetime string to table {year, month, day, hour, min, sec}
    let parse_fn = lua.create_function(|lua, (date_str, format): (String, Option<String>)| {
        let dt = if let Some(ref fmt) = format {
            // User-provided format
            NaiveDateTime::parse_from_str(&date_str, fmt)
                .ok()
                .or_else(|| {
                    // Try as date-only format, default to midnight
                    NaiveDate::parse_from_str(&date_str, fmt)
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                })
        } else {
            // Auto-detect format
            parse_datetime(&Value::String(lua.create_string(&date_str)?))
        };

        match dt {
            Some(dt) => {
                let result = create_datetime_table(lua, dt)?;
                Ok(Value::Table(result))
            }
            None => Ok(Value::Nil),
        }
    })?;
    date.set("parse", parse_fn)?;

    // rss_format(date) - Format date for RSS feeds (RFC 2822)
    let rss_format_fn =
        lua.create_function(|lua, date_val: Value| match parse_datetime(&date_val) {
            Some(dt) => {
                let datetime = Utc.from_utc_datetime(&dt);
                let formatted = datetime.format("%a, %d %b %Y %H:%M:%S +0000").to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            None => Ok(Value::Nil),
        })?;
    date.set("rss_format", rss_format_fn)?;

    // iso_format(date) - Format date as ISO 8601
    let iso_format_fn =
        lua.create_function(|lua, date_val: Value| match parse_datetime(&date_val) {
            Some(dt) => {
                let formatted = dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            None => Ok(Value::Nil),
        })?;
    date.set("iso_format", iso_format_fn)?;

    // add(date, delta) - Add time to a date
    // delta = { years?, months?, days?, hours?, mins?, secs? }
    let add_fn = lua.create_function(|lua, (date_val, delta): (Value, Table)| {
        let dt = match parse_datetime(&date_val) {
            Some(dt) => dt,
            None => return Ok(Value::Nil),
        };

        let years: i32 = delta.get("years").unwrap_or(0);
        let months: i32 = delta.get("months").unwrap_or(0);
        let days: i64 = delta.get("days").unwrap_or(0);
        let hours: i64 = delta.get("hours").unwrap_or(0);
        let mins: i64 = delta.get("mins").unwrap_or(0);
        let secs: i64 = delta.get("secs").unwrap_or(0);

        // Add years and months
        let mut new_year = dt.year() + years;
        let mut new_month = dt.month() as i32 + months;

        // Handle month overflow/underflow
        while new_month > 12 {
            new_month -= 12;
            new_year += 1;
        }
        while new_month < 1 {
            new_month += 12;
            new_year -= 1;
        }

        // Clamp day to valid range for the new month
        let max_day = NaiveDate::from_ymd_opt(new_year, new_month as u32 + 1, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(new_year + 1, 1, 1).unwrap())
            .pred_opt()
            .unwrap()
            .day();
        let new_day = dt.day().min(max_day);

        let new_date = match NaiveDate::from_ymd_opt(new_year, new_month as u32, new_day) {
            Some(d) => d,
            None => return Ok(Value::Nil),
        };

        let new_dt = match new_date.and_hms_opt(dt.hour(), dt.minute(), dt.second()) {
            Some(dt) => dt,
            None => return Ok(Value::Nil),
        };

        // Add days, hours, mins, secs using Duration
        let duration = chrono::Duration::days(days)
            + chrono::Duration::hours(hours)
            + chrono::Duration::minutes(mins)
            + chrono::Duration::seconds(secs);

        let final_dt = new_dt + duration;
        let result = create_datetime_table(lua, final_dt)?;
        Ok(Value::Table(result))
    })?;
    date.set("add", add_fn)?;

    // diff(date1, date2, opts?) - Get difference between two dates in seconds
    // opts: { format1?: string, format2?: string }
    let diff_fn =
        lua.create_function(|_, (date1, date2, opts): (Value, Value, Option<Table>)| {
            let format1: Option<String> = opts.as_ref().and_then(|t| t.get("format1").ok());
            let format2: Option<String> = opts.as_ref().and_then(|t| t.get("format2").ok());

            let parse_with_format = |val: &Value, fmt: &Option<String>| -> Option<NaiveDateTime> {
                if let Some(fmt) = fmt
                    && let Value::String(s) = val
                {
                    let s = s.to_str().ok()?.to_string();
                    return NaiveDateTime::parse_from_str(&s, fmt).ok().or_else(|| {
                        NaiveDate::parse_from_str(&s, fmt)
                            .ok()
                            .and_then(|d| d.and_hms_opt(0, 0, 0))
                    });
                }
                parse_datetime(val)
            };

            let dt1 = match parse_with_format(&date1, &format1) {
                Some(dt) => dt,
                None => return Ok(Value::Nil),
            };
            let dt2 = match parse_with_format(&date2, &format2.or(format1.clone())) {
                Some(dt) => dt,
                None => return Ok(Value::Nil),
            };
            let diff = dt1.signed_duration_since(dt2);
            Ok(Value::Integer(diff.num_seconds()))
        })?;
    date.set("diff", diff_fn)?;

    Ok(date)
}
