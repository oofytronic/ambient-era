//! Local clock labels for chat. Display only. Not an order.
//!
//! The envelope stores advisory unix milliseconds. The window turns that into
//! the machine's local time so a message reads like a clock, not a hash.

use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct When {
    /// Chip above the first message of a local day. Empty when there is no time.
    pub day: String,
    /// `HH:MM` on the bubble. Empty when there is no time.
    pub time: String,
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(0))
}

pub(super) fn load_tz() -> Vec<u8> {
    std::fs::read("/etc/localtime").unwrap_or_default()
}

pub(super) fn when(unix_ms: Option<u64>, tz: &[u8], now: u64) -> When {
    let Some(unix_ms) = unix_ms else {
        return When {
            day: String::new(),
            time: String::new(),
        };
    };
    let offset = offset_at(tz, i64::try_from(unix_ms / 1000).unwrap_or(0)).unwrap_or(0);
    let now_offset = offset_at(tz, i64::try_from(now / 1000).unwrap_or(0)).unwrap_or(offset);
    let local = local_secs(unix_ms, offset);
    let today = local_secs(now, now_offset);
    let (year, month, day, hour, minute) = ymd_hm(local);
    let time = format!("{hour:02}:{minute:02}");
    let day_label = day_label(local, today, year, month, day);
    When {
        day: day_label,
        time,
    }
}

fn local_secs(unix_ms: u64, offset: i32) -> i64 {
    let secs = i64::try_from(unix_ms / 1000).unwrap_or(0);
    secs.saturating_add(i64::from(offset))
}

fn day_label(local: i64, today: i64, year: i32, month: u32, day: u32) -> String {
    let local_day = local.div_euclid(86_400);
    let today_day = today.div_euclid(86_400);
    if local_day == today_day {
        return "Today".to_owned();
    }
    if local_day + 1 == today_day {
        return "Yesterday".to_owned();
    }
    let (now_year, _, _, _, _) = ymd_hm(today);
    let name = month_name(month);
    if year == now_year {
        format!("{name} {day}")
    } else {
        format!("{name} {day}, {year}")
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

/// UTC civil time from unix seconds, using Howard Hinnant's algorithm.
fn ymd_hm(secs: i64) -> (i32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let tod = u64::try_from(secs.rem_euclid(86_400)).unwrap_or(0);
    let hour = u32::try_from(tod / 3_600).unwrap_or(0);
    let minute = u32::try_from((tod % 3_600) / 60).unwrap_or(0);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = u64::try_from(z - era * 146_097).unwrap_or(0);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::try_from(yoe).unwrap_or(0) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = i32::try_from(if month <= 2 { y + 1 } else { y }).unwrap_or(0);
    (
        year,
        u32::try_from(month).unwrap_or(0),
        u32::try_from(day).unwrap_or(0),
        hour,
        minute,
    )
}

/// Seconds east of UTC from a `TZif` file. `None` when the file is not `TZif`.
fn offset_at(data: &[u8], unix: i64) -> Option<i32> {
    let (version, counts) = tz_header(data, 0)?;
    if version >= b'2' {
        let first = tz_data_len(counts, 4)?;
        let at = 44usize.checked_add(first)?;
        let (next, counts) = tz_header(data, at)?;
        if next < b'2' {
            return None;
        }
        let body = at.checked_add(44)?;
        return tz_offset(data, body, counts, 8, unix);
    }
    tz_offset(data, 44, counts, 4, unix)
}

fn tz_header(data: &[u8], at: usize) -> Option<(u8, [i32; 6])> {
    if data.get(at..at.checked_add(4)?)? != b"TZif" {
        return None;
    }
    let version = *data.get(at.checked_add(4)?)?;
    let mut counts = [0_i32; 6];
    for (index, slot) in counts.iter_mut().enumerate() {
        let off = at.checked_add(20)?.checked_add(index.checked_mul(4)?)?;
        let bytes = data.get(off..off.checked_add(4)?)?;
        let raw: [u8; 4] = bytes.try_into().ok()?;
        *slot = i32::from_be_bytes(raw);
    }
    Some((version, counts))
}

fn tz_data_len(counts: [i32; 6], time_size: usize) -> Option<usize> {
    let ut = usize::try_from(*counts.first()?).ok()?;
    let std = usize::try_from(*counts.get(1)?).ok()?;
    let leap = usize::try_from(*counts.get(2)?).ok()?;
    let times = usize::try_from(*counts.get(3)?).ok()?;
    let types = usize::try_from(*counts.get(4)?).ok()?;
    let chars = usize::try_from(*counts.get(5)?).ok()?;
    times
        .checked_mul(time_size)?
        .checked_add(times)?
        .checked_add(types.checked_mul(6)?)?
        .checked_add(chars)?
        .checked_add(leap.checked_mul(time_size.checked_add(4)?)?)?
        .checked_add(std)?
        .checked_add(ut)
}

fn tz_offset(data: &[u8], at: usize, counts: [i32; 6], time_size: usize, unix: i64) -> Option<i32> {
    let times = usize::try_from(*counts.get(3)?).ok()?;
    let types = usize::try_from(*counts.get(4)?).ok()?;
    if types == 0 {
        return None;
    }
    let mut index = 0_usize;
    let mut cursor = at;
    for nth in 0..times {
        let raw = data.get(cursor..cursor.checked_add(time_size)?)?;
        let transition = if time_size == 8 {
            let bytes: [u8; 8] = raw.try_into().ok()?;
            i64::from_be_bytes(bytes)
        } else {
            let bytes: [u8; 4] = raw.try_into().ok()?;
            i64::from(i32::from_be_bytes(bytes))
        };
        if transition <= unix {
            index = nth.saturating_add(1);
        }
        cursor = cursor.checked_add(time_size)?;
    }
    let types_at = cursor;
    let chosen = if index == 0 {
        0
    } else {
        let at = types_at.checked_add(index - 1)?;
        usize::from(*data.get(at)?)
    };
    if chosen >= types {
        return None;
    }
    let rec = types_at
        .checked_add(times)?
        .checked_add(chosen.checked_mul(6)?)?;
    let bytes = data.get(rec..rec.checked_add(4)?)?;
    let raw: [u8; 4] = bytes.try_into().ok()?;
    Some(i32::from_be_bytes(raw))
}

#[cfg(test)]
mod tests {
    use super::{offset_at, when, ymd_hm};

    #[test]
    fn unix_epoch_and_a_known_second() {
        assert_eq!(ymd_hm(0), (1970, 1, 1, 0, 0));
        assert_eq!(ymd_hm(1_700_000_000), (2023, 11, 14, 22, 13));
    }

    #[test]
    fn version_one_zone_offset() -> Result<(), String> {
        let data = tz_v1(-7 * 3_600);
        let offset = offset_at(&data, 1_700_000_000).ok_or("offset")?;
        if offset != -7 * 3_600 {
            return Err(offset.to_string());
        }
        Ok(())
    }

    #[test]
    fn same_day_is_today() {
        let tz = tz_v1(0);
        let now = 1_700_000_000_000_u64;
        let labeled = when(Some(now), &tz, now);
        assert_eq!(labeled.day, "Today");
        assert_eq!(labeled.time, "22:13");
    }

    fn tz_v1(utoff: i32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(b"TZif");
        data.push(0);
        data.extend([0; 15]);
        for count in [0_i32, 0, 0, 0, 1, 3] {
            data.extend(count.to_be_bytes());
        }
        data.extend(utoff.to_be_bytes());
        data.push(0);
        data.push(0);
        data.extend(b"UTC");
        data
    }
}
