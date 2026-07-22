use crate::squiggle::{DateValue, Diagnostic, DurationValue, Value, ast::Span};

use super::Runtime;

builtins! {
    context(runtime, span);
    "Date.make"(value: String) => finish_date(DateValue::parse(value), span),
    "Date.make"(year: Number) => fractional_year(year, span),
    "Date.make"(year: Integer, month: NonNegativeInteger, day: NonNegativeInteger) => {
        gregorian_date(year, month, day, span)
    },
    "Date.fromUnixTime"(seconds: Number) => {
        finish_date(DateValue::from_unix_seconds(seconds), span)
    },
    "Date.toUnixTime"(value: Date) => Ok(Value::Number(value.unix_seconds())),
    "Duration.fromMinutes" | fromMinutes(value: Number) => {
        finish_duration(DurationValue::from_minutes(value), span)
    },
    "Duration.fromHours" | fromHours(value: Number) => {
        finish_duration(DurationValue::from_hours(value), span)
    },
    "Duration.fromDays" | fromDays(value: Number) => {
        finish_duration(DurationValue::from_days(value), span)
    },
    "Duration.fromYears" | fromYears(value: Number) => {
        finish_duration(DurationValue::from_years(value), span)
    },
    "Duration.toMinutes" | toMinutes(value: Duration) => Ok(Value::Number(value.as_minutes())),
    "Duration.toHours" | toHours(value: Duration) => Ok(Value::Number(value.as_hours())),
    "Duration.toDays" | toDays(value: Duration) => Ok(Value::Number(value.as_days())),
    "Duration.toYears" | toYears(value: Duration) => Ok(Value::Number(value.as_years())),
}

fn fractional_year(year: f64, span: Span) -> Result<Value, Diagnostic> {
    let whole = year.floor();
    if !whole.is_finite() || whole < i32::MIN as f64 || whole > i32::MAX as f64 {
        return Err(Diagnostic::runtime(
            "year is outside the supported range",
            span,
        ));
    }
    let start = DateValue::from_ymd(whole as i32, 1, 1)
        .map_err(|error| Diagnostic::runtime(error, span))?;
    let offset = DurationValue::from_years(year - whole)
        .map_err(|error| Diagnostic::runtime(error, span))?;
    Ok(Value::Date(start.add(offset)))
}

fn gregorian_date(year: i64, month: u64, day: u64, span: Span) -> Result<Value, Diagnostic> {
    let year = i32::try_from(year)
        .map_err(|_| Diagnostic::runtime("year is outside the supported range", span))?;
    let month = u32::try_from(month)
        .map_err(|_| Diagnostic::runtime("month is outside the supported range", span))?;
    let day = u32::try_from(day)
        .map_err(|_| Diagnostic::runtime("day is outside the supported range", span))?;
    finish_date(DateValue::from_ymd(year, month, day), span)
}

fn finish_date(result: Result<DateValue, String>, span: Span) -> Result<Value, Diagnostic> {
    result
        .map(Value::Date)
        .map_err(|error| Diagnostic::runtime(error, span))
}

fn finish_duration(result: Result<DurationValue, String>, span: Span) -> Result<Value, Diagnostic> {
    result
        .map(Value::Duration)
        .map_err(|error| Diagnostic::runtime(error, span))
}
