use chrono::{DateTime, NaiveDate, NaiveTime, SecondsFormat, Utc};

pub fn canonical_date_days(days: i32) -> String {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid Unix epoch");
    epoch
        .checked_add_signed(chrono::Duration::days(i64::from(days)))
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| days.to_string())
}

pub fn canonical_time_units(value: i64, units_per_second: i64, precision: usize) -> String {
    let units_per_day = 86_400_i64.saturating_mul(units_per_second);
    let normalized = value.rem_euclid(units_per_day.max(1));
    let seconds = normalized / units_per_second.max(1);
    let fractional = normalized % units_per_second.max(1);
    let time =
        NaiveTime::from_num_seconds_from_midnight_opt(seconds as u32, 0).unwrap_or(NaiveTime::MIN);
    if precision == 0 {
        time.format("%H:%M:%S").to_string()
    } else {
        format!(
            "{}.{:0width$}",
            time.format("%H:%M:%S"),
            fractional,
            width = precision
        )
    }
}

pub fn canonical_timestamp_units(
    value: i64,
    units_per_second: i64,
    precision: usize,
    utc: bool,
) -> String {
    let seconds = value.div_euclid(units_per_second.max(1));
    let fractional = value.rem_euclid(units_per_second.max(1));
    let nanos = match precision {
        0 => 0,
        1..=3 => fractional.saturating_mul(1_000_000),
        4..=6 => fractional.saturating_mul(1_000),
        _ => fractional,
    } as u32;
    let Some(timestamp) = DateTime::<Utc>::from_timestamp(seconds, nanos) else {
        return value.to_string();
    };
    if utc {
        timestamp.to_rfc3339_opts(
            match precision {
                0 => SecondsFormat::Secs,
                1..=3 => SecondsFormat::Millis,
                4..=6 => SecondsFormat::Micros,
                _ => SecondsFormat::Nanos,
            },
            true,
        )
    } else {
        timestamp
            .naive_utc()
            .format(match precision {
                0 => "%Y-%m-%d %H:%M:%S",
                1..=3 => "%Y-%m-%d %H:%M:%S%.3f",
                4..=6 => "%Y-%m-%d %H:%M:%S%.6f",
                _ => "%Y-%m-%d %H:%M:%S%.9f",
            })
            .to_string()
    }
}
