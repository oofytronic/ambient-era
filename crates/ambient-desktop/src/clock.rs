//! Local clock labels for chat. Display only. Not an order.
//!
//! The envelope stores advisory unix milliseconds. The window turns that into
//! the machine's local time so a message reads like a clock, not a hash.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::zone::Zone;

pub(crate) struct When {
    /// Chip above the first message of a local day. Empty when there is no time.
    pub day: String,
    /// `HH:MM` on the bubble. Empty when there is no time.
    pub time: String,
}

/// Whether times read `3:42 PM`, not `15:42`. One answer for the whole window, set from the
/// person's choice, so the thread that makes the chat list says times the same way as the page.
static HOUR12: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub(crate) fn set_hour12(on: bool) {
    HOUR12.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn hour12() -> bool {
    HOUR12.load(std::sync::atomic::Ordering::Relaxed)
}

/// A time of day for a person: `3:42 PM`, or `15:42` on a 24 hour clock.
pub(crate) fn time_text(hour: u32, minute: u32, twelve: bool) -> String {
    if twelve {
        let shown = (hour + 11) % 12 + 1;
        let half = if hour < 12 { "AM" } else { "PM" };
        format!("{shown}:{minute:02} {half}")
    } else {
        format!("{hour:02}:{minute:02}")
    }
}

/// `time_text`, the way the person asked to read time.
pub(crate) fn clock_text(hour: u32, minute: u32) -> String {
    time_text(hour, minute, hour12())
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(0))
}

/// This computer's time zone, from the host, or UTC when the host has none. Reading it again is
/// cheap, so it is read each time it might have changed, not once.
pub(crate) fn load_zone() -> (Zone, Option<String>) {
    match ambient_cli::host().time_zone() {
        Some(found) => (
            Zone::parse(&found.tzif).unwrap_or_else(Zone::utc),
            found.name,
        ),
        None => (Zone::utc(), None),
    }
}

/// What the clock says on this computer right now: the day, the minute of it, and which zone.
/// The window keeps one and refreshes it, so everything on a screen agrees about now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Clock {
    pub today: crate::dates::Date,
    /// Minutes since local midnight.
    pub minutes: u32,
    /// Seconds east of UTC.
    pub offset: i32,
    /// The zone's name such as `America/Los_Angeles`, or its short name such as `PST`.
    pub zone: String,
}

impl Clock {
    pub(crate) fn read(zone: &Zone, name: Option<&str>, now_ms: u64) -> Self {
        let secs = i64::try_from(now_ms / 1000).unwrap_or(0);
        let offset = zone.offset_at(secs);
        let (year, month, day, hour, minute) = ymd_hm(secs.saturating_add(i64::from(offset)));
        Self {
            today: crate::dates::Date::new(year, month, day).unwrap_or(crate::dates::Date {
                year: 1970,
                month: 1,
                day: 1,
            }),
            minutes: hour * 60 + minute,
            offset,
            zone: name.map_or_else(|| zone.abbr_at(secs), str::to_owned),
        }
    }

    /// This reading's time of day, as the person reads time.
    pub(crate) fn time_label(&self) -> String {
        clock_text(self.minutes / 60, self.minutes % 60)
    }

    /// `UTC-8`, `UTC+5:30`, or `UTC`.
    pub(crate) fn offset_label(&self) -> String {
        if self.offset == 0 {
            return "UTC".to_owned();
        }
        let sign = if self.offset < 0 { '-' } else { '+' };
        let total = self.offset.unsigned_abs() / 60;
        let (hours, minutes) = (total / 60, total % 60);
        if minutes == 0 {
            format!("UTC{sign}{hours}")
        } else {
            format!("UTC{sign}{hours}:{minutes:02}")
        }
    }
}

pub(crate) fn when(unix_ms: Option<u64>, zone: &Zone, now: u64) -> When {
    let Some(unix_ms) = unix_ms else {
        return When {
            day: String::new(),
            time: String::new(),
        };
    };
    let offset = zone.offset_at(i64::try_from(unix_ms / 1000).unwrap_or(0));
    let now_offset = zone.offset_at(i64::try_from(now / 1000).unwrap_or(0));
    let local = local_secs(unix_ms, offset);
    let today = local_secs(now, now_offset);
    let (year, month, day, hour, minute) = ymd_hm(local);
    let time = clock_text(hour, minute);
    let day_label = day_label(local, today, year, month, day);
    When {
        day: day_label,
        time,
    }
}

/// `YYYY-MM-DD-HHMM` in local time. For naming things so they sort by when they were made.
pub(crate) fn stamp(unix_ms: u64, zone: &Zone) -> String {
    let secs = i64::try_from(unix_ms / 1000).unwrap_or(0);
    let offset = zone.offset_at(secs);
    let (year, month, day, hour, minute) = ymd_hm(secs.saturating_add(i64::from(offset)));
    format!("{year:04}-{month:02}-{day:02}-{hour:02}{minute:02}")
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

/// How far two clocks are apart, in words: `3 hours ahead`, `5½ hours behind`, `the same`.
/// `east` and `home` are seconds east of UTC.
pub(crate) fn gap_words(east: i32, home: i32) -> String {
    let apart = i64::from(east) - i64::from(home);
    if apart == 0 {
        return "the same time".to_owned();
    }
    let minutes = apart.unsigned_abs() / 60;
    let (hours, rest) = (minutes / 60, minutes % 60);
    let amount = match (hours, rest) {
        (1, 0) => "1 hour".to_owned(),
        (h, 0) => format!("{h} hours"),
        (h, 30) => format!("{h}½ hours"),
        (h, m) => format!("{h} hours {m} minutes"),
    };
    format!("{amount} {}", if apart > 0 { "ahead" } else { "behind" })
}

/// A person who is not where they live: the zone they call home, and what its clock says now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Trip {
    pub home_name: String,
    /// The home zone's clock now, when its zone file could be read.
    pub home: Option<Clock>,
}

/// Whether this computer is somewhere other than home. It is when the zone has a different name
/// and sets the clock differently at some point in the next year, so a second name for the same
/// place is not a trip. When the home zone cannot be read, a different name is taken as a trip,
/// since it cannot be shown to be the same.
pub(crate) fn away_from_home(
    here_name: Option<&str>,
    here: &Zone,
    home_name: &str,
    home: Option<&Zone>,
    now_ms: u64,
) -> Option<Trip> {
    if home_name.is_empty() || here_name == Some(home_name) {
        return None;
    }
    let now = i64::try_from(now_ms / 1000).unwrap_or(0);
    match home {
        Some(zone) if here.same_as(zone, now) => None,
        Some(zone) => Some(Trip {
            home_name: home_name.to_owned(),
            home: Some(Clock::read(zone, Some(home_name), now_ms)),
        }),
        None => here_name.map(|_| Trip {
            home_name: home_name.to_owned(),
            home: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, stamp, when, ymd_hm};
    use crate::zone::Zone;

    #[test]
    fn a_stamp_names_a_moment_so_names_sort_by_time() {
        let utc = Zone::utc();
        assert_eq!(stamp(0, &utc), "1970-01-01-0000");
        assert_eq!(stamp(1_700_000_000_000, &utc), "2023-11-14-2213");
        // In a zone seven hours behind, the same moment is earlier on the same day.
        let seven_behind = zone_at(-7 * 3_600);
        assert_eq!(stamp(1_700_000_000_000, &seven_behind), "2023-11-14-1513");
        assert!(stamp(1_700_000_000_000, &utc) < stamp(1_700_000_060_000, &utc));
    }

    #[test]
    fn unix_epoch_and_a_known_second() {
        assert_eq!(ymd_hm(0), (1970, 1, 1, 0, 0));
        assert_eq!(ymd_hm(1_700_000_000), (2023, 11, 14, 22, 13));
    }

    #[test]
    fn same_day_is_today() {
        let zone = Zone::utc();
        let now = 1_700_000_000_000_u64;
        let labeled = when(Some(now), &zone, now);
        assert_eq!(labeled.day, "Today");
        assert_eq!(labeled.time, super::clock_text(22, 13));
    }

    #[test]
    fn the_clock_names_the_day_and_minute_in_the_zone_it_is_in() {
        let zone = zone_with_footer("PST8PDT,M3.2.0,M11.1.0");
        // 2026-11-20 03:42 UTC is the evening before in Los Angeles, in standard time.
        let at = 1_795_146_120_000_u64;
        let clock = Clock::read(&zone, Some("America/Los_Angeles"), at);
        assert_eq!(clock.today.to_string(), "2026-11-19");
        assert_eq!(clock.minutes, 19 * 60 + 42);
        assert_eq!(clock.offset_label(), "UTC-8");
        assert_eq!(clock.zone, "America/Los_Angeles");
        // The same moment in summer is an hour different: 2026-07-20 03:42 UTC is 20:42 the day before.
        let summer = Clock::read(&zone, None, 1_784_518_920_000);
        assert_eq!(summer.today.to_string(), "2026-07-19");
        assert_eq!(summer.minutes, 20 * 60 + 42);
        assert_eq!(summer.zone, "PDT", "with no name, the short one");
        assert_eq!(summer.offset_label(), "UTC-7");
        // A zone half an hour off, and none.
        let india = Clock::read(&zone_with_footer("IST-5:30"), None, at);
        assert_eq!(india.offset_label(), "UTC+5:30");
        assert_eq!(
            (india.today.to_string(), india.minutes),
            ("2026-11-20".to_owned(), 9 * 60 + 12)
        );
        assert_eq!(Clock::read(&Zone::utc(), None, at).offset_label(), "UTC");
    }

    #[test]
    fn a_time_reads_on_a_twelve_or_a_twenty_four_hour_clock() {
        use super::time_text;
        for (hour, minute, twelve, twenty_four) in [
            (0, 5, "12:05 AM", "00:05"),
            (9, 0, "9:00 AM", "09:00"),
            (11, 59, "11:59 AM", "11:59"),
            (12, 0, "12:00 PM", "12:00"),
            (13, 30, "1:30 PM", "13:30"),
            (15, 42, "3:42 PM", "15:42"),
            (23, 59, "11:59 PM", "23:59"),
        ] {
            assert_eq!(time_text(hour, minute, true), twelve);
            assert_eq!(time_text(hour, minute, false), twenty_four);
        }
    }

    #[test]
    fn a_gap_between_clocks_is_said_in_hours_and_which_way() {
        use super::gap_words;
        assert_eq!(gap_words(-5 * 3_600, -8 * 3_600), "3 hours ahead");
        assert_eq!(gap_words(-8 * 3_600, -5 * 3_600), "3 hours behind");
        assert_eq!(gap_words(3_600, 0), "1 hour ahead");
        assert_eq!(gap_words(19_800, 0), "5½ hours ahead");
        assert_eq!(gap_words(20_700, 0), "5 hours 45 minutes ahead");
        assert_eq!(gap_words(0, 0), "the same time");
    }

    #[test]
    fn a_trip_is_somewhere_that_sets_the_clock_differently_from_home() {
        use super::away_from_home;
        let la = zone_with_footer("PST8PDT,M3.2.0,M11.1.0");
        let alias = zone_with_footer("PST8PDT,M3.2.0,M11.1.0");
        let ny = zone_with_footer("EST5EDT,M3.2.0,M11.1.0");
        let now = 1_795_146_120_000_u64;
        let trip = |here_name: Option<&str>, here: &super::Zone, home: Option<&super::Zone>| {
            away_from_home(here_name, here, "America/Los_Angeles", home, now)
        };
        // At home: the same name, and nothing is read.
        assert!(trip(Some("America/Los_Angeles"), &la, None).is_none());
        // Another name for the same clock is not a trip.
        assert!(trip(Some("US/Pacific"), &alias, Some(&la)).is_none());
        // Somewhere else is, and says what the clock at home reads.
        let away = trip(Some("America/New_York"), &ny, Some(&la)).expect("a trip");
        let home = away.home.expect("home clock");
        assert_eq!(home.offset, -8 * 3_600);
        assert_eq!(home.zone, "America/Los_Angeles");
        // A zone that is not named cannot be compared by name, only by its clock.
        assert!(trip(None, &ny, Some(&la)).is_some());
        assert!(trip(None, &alias, Some(&la)).is_none());
        // With no home file, a different name is a trip with no home clock, and no name is none.
        let blind = trip(Some("America/New_York"), &ny, None).expect("a trip");
        assert!(blind.home.is_none());
        assert!(trip(None, &ny, None).is_none());
        // No home yet is no trip.
        assert!(away_from_home(Some("America/New_York"), &ny, "", None, now).is_none());
    }

    #[test]
    fn midnight_is_the_moment_the_day_changes() {
        let zone = zone_with_footer("PST8PDT,M3.2.0,M11.1.0");
        // 2026-11-20 00:00 in Los Angeles is 08:00 UTC.
        let midnight = 1_795_161_600_000_u64;
        let before = Clock::read(&zone, None, midnight - 1_000);
        let after = Clock::read(&zone, None, midnight);
        assert_eq!(before.today.to_string(), "2026-11-19");
        assert_eq!((before.minutes, after.minutes), (23 * 60 + 59, 0));
        assert_eq!(after.today.to_string(), "2026-11-20");
        // The night the clocks go forward has no 02:30: 01:59:59 is followed by 03:00.
        let forward = 1_772_964_000_000_u64;
        let one = Clock::read(&zone, None, forward - 1_000);
        let three = Clock::read(&zone, None, forward);
        assert_eq!((one.minutes, three.minutes), (60 + 59, 3 * 60));
    }

    fn zone_at(utoff: i32) -> Zone {
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
        Zone::parse(&data).expect("zone")
    }

    /// A version 2 file that lists no changes, so the footer decides.
    fn zone_with_footer(footer: &str) -> Zone {
        let mut data = Vec::new();
        for block in 0..2 {
            data.extend(b"TZif2");
            data.extend([0; 15]);
            for count in [0_i32, 0, 0, 0, 1, 4] {
                data.extend(count.to_be_bytes());
            }
            data.extend(0_i32.to_be_bytes());
            data.extend([0, 0]);
            data.extend(b"UTC\0");
            if block == 1 {
                data.push(b'\n');
                data.extend(footer.as_bytes());
                data.push(b'\n');
            }
        }
        Zone::parse(&data).expect("zone")
    }
}
