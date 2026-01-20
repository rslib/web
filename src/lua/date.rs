//! Date module (rs.date)

use chrono::Datelike;
use mlua::{Lua, Result, Table, Value};

pub fn create_module(lua: &Lua) -> Result<Table> {
    let date = lua.create_table()?;

    // format(date, format) - Format a date string
    let format_fn = lua.create_function(|lua, (date_val, format): (Value, String)| {
        let parsed = match &date_val {
            Value::String(s) => {
                let s = s.to_str().map_err(mlua::Error::external)?;
                chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()
            }
            Value::Table(t) => {
                let year: i32 = t.get("year").unwrap_or(2000);
                let month: u32 = t.get("month").unwrap_or(1);
                let day: u32 = t.get("day").unwrap_or(1);
                chrono::NaiveDate::from_ymd_opt(year, month, day)
            }
            _ => None,
        };

        match parsed {
            Some(d) => {
                let formatted = d.format(&format).to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            None => Ok(Value::Nil),
        }
    })?;
    date.set("format", format_fn)?;

    // parse(str) - Parse date string to table {year, month, day}
    let parse_fn = lua.create_function(|lua, date_str: String| {
        let formats = ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d"];
        for fmt in &formats {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&date_str, fmt) {
                let result = lua.create_table()?;
                result.set("year", d.year())?;
                result.set("month", d.month())?;
                result.set("day", d.day())?;
                return Ok(Value::Table(result));
            }
        }
        Ok(Value::Nil)
    })?;
    date.set("parse", parse_fn)?;

    // rss_format(date) - Format date for RSS feeds (RFC 2822)
    let rss_format_fn = lua.create_function(|lua, date_val: Value| {
        let parsed = match &date_val {
            Value::String(s) => {
                let s = s.to_str().map_err(mlua::Error::external)?;
                chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()
            }
            Value::Table(t) => {
                let year: i32 = t.get("year").unwrap_or(2000);
                let month: u32 = t.get("month").unwrap_or(1);
                let day: u32 = t.get("day").unwrap_or(1);
                chrono::NaiveDate::from_ymd_opt(year, month, day)
            }
            _ => None,
        };

        match parsed {
            Some(d) => {
                let datetime = d.and_hms_opt(12, 0, 0).unwrap().and_utc();
                let formatted = datetime.format("%a, %d %b %Y %H:%M:%S +0000").to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            None => Ok(Value::Nil),
        }
    })?;
    date.set("rss_format", rss_format_fn)?;

    Ok(date)
}
