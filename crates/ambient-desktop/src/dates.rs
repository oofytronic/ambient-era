//! Calendar days, for a task's due date. A day is a real day of the real calendar, and it is
//! written one way only: `2026-11-20`. Nothing here reads a clock or a time zone. The caller says
//! what today is.

use std::fmt;

/// A day of the Gregorian calendar.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl fmt::Display for Date {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

pub(crate) const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(crate) fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

impl Date {
    /// The day, if it exists. `2026-02-30` does not.
    pub(crate) fn new(year: i32, month: u32, day: u32) -> Option<Self> {
        ((1900..=9999).contains(&year) && day >= 1 && day <= days_in_month(year, month))
            .then_some(Self { year, month, day })
    }

    /// Exactly `YYYY-MM-DD`, and a day that exists.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let shape = bytes.len() == 10
            && bytes.get(4) == Some(&b'-')
            && bytes.get(7) == Some(&b'-')
            && bytes
                .iter()
                .enumerate()
                .all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit());
        if !shape {
            return None;
        }
        Self::new(
            text.get(..4)?.parse().ok()?,
            text.get(5..7)?.parse().ok()?,
            text.get(8..)?.parse().ok()?,
        )
    }

    /// Days since 1970-01-01. Negative before it.
    pub(crate) fn serial(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let yoe = year - era * 400;
        let month = i64::from(self.month);
        let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5
            + i64::from(self.day)
            - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The day `serial` days after 1970-01-01.
    pub(crate) fn from_serial(serial: i64) -> Option<Self> {
        let z = serial + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let year = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = year + i64::from(month <= 2);
        Self::new(
            i32::try_from(year).ok()?,
            u32::try_from(month).ok()?,
            u32::try_from(day).ok()?,
        )
    }

    pub(crate) fn plus_days(self, days: i64) -> Option<Self> {
        Self::from_serial(self.serial().checked_add(days)?)
    }

    /// 0 for Sunday through 6 for Saturday.
    pub(crate) fn weekday(self) -> u32 {
        // 1970-01-01 was a Thursday.
        u32::try_from((self.serial() + 4).rem_euclid(7)).unwrap_or(0)
    }

    /// `Nov 20`, or `Nov 20, 2027` when it is not this year.
    pub(crate) fn short(self, today: Self) -> String {
        let name = MONTHS
            .get(usize::try_from(self.month.saturating_sub(1)).unwrap_or(0))
            .map_or("", |name| name.get(..3).unwrap_or(name));
        if self.year == today.year {
            format!("{name} {}", self.day)
        } else {
            format!("{name} {}, {}", self.day, self.year)
        }
    }
}

/// A month, as the rows of a calendar: seven cells to a row, Sunday first, `None` for the cells
/// before the first and after the last.
pub(crate) fn month_grid(year: i32, month: u32) -> Vec<[Option<Date>; 7]> {
    let Some(first) = Date::new(year, month, 1) else {
        return Vec::new();
    };
    let lead = usize::try_from(first.weekday()).unwrap_or(0);
    let length = usize::try_from(days_in_month(year, month)).unwrap_or(0);
    let mut rows = Vec::new();
    let mut cell = 0_usize;
    while cell < lead + length {
        let mut row = [None; 7];
        for slot in &mut row {
            *slot = cell
                .checked_sub(lead)
                .filter(|day| *day < length)
                .and_then(|day| Date::new(year, month, u32::try_from(day + 1).ok()?));
            cell += 1;
        }
        rows.push(row);
    }
    rows
}

/// The month `delta` months from this one, as year and month.
pub(crate) fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let index = year * 12 + i32::try_from(month).unwrap_or(1) - 1 + delta;
    (
        index.div_euclid(12),
        u32::try_from(index.rem_euclid(12) + 1).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::{Date, days_in_month, month_grid, shift_month};

    fn day(year: i32, month: u32, day: u32) -> Date {
        Date::new(year, month, day).expect("a real day")
    }

    #[test]
    fn a_day_is_written_one_way_and_must_exist() {
        assert_eq!(day(2026, 11, 20).to_string(), "2026-11-20");
        assert_eq!(
            day(2026, 1, 5).to_string(),
            "2026-01-05",
            "always two digits"
        );
        assert_eq!(Date::parse("2026-11-20"), Some(day(2026, 11, 20)));
        for bad in [
            "2026-2-30",
            "2026-02-30",
            "2026-13-01",
            "2026-00-10",
            "2026-11-31",
            "26-11-20",
            "2026/11/20",
            "2026-11-2x",
            "",
            " 2026-11-20",
            "2026-11-20 ",
            "1899-12-31",
        ] {
            assert_eq!(Date::parse(bad), None, "{bad:?}");
        }
        assert_eq!(
            Date::parse("2028-02-29"),
            Some(day(2028, 2, 29)),
            "a leap day"
        );
        assert_eq!(Date::parse("2027-02-29"), None);
        assert_eq!(
            Date::parse("2100-02-29"),
            None,
            "not every century year is a leap year"
        );
        assert_eq!(Date::parse("2000-02-29"), Some(day(2000, 2, 29)));
    }

    #[test]
    fn days_count_forward_across_months_and_years() {
        let epoch = day(1970, 1, 1);
        assert_eq!(epoch.serial(), 0);
        assert_eq!(day(2026, 11, 20).serial(), 20_777);
        // 56 years, 14 of them leap, then 323 days into 2026.
        assert_eq!(Date::from_serial(20_777), Some(day(2026, 11, 20)));
        assert_eq!(day(2026, 11, 20).plus_days(1), Some(day(2026, 11, 21)));
        assert_eq!(day(2026, 11, 30).plus_days(1), Some(day(2026, 12, 1)));
        assert_eq!(day(2026, 12, 31).plus_days(1), Some(day(2027, 1, 1)));
        assert_eq!(day(2028, 2, 28).plus_days(1), Some(day(2028, 2, 29)));
        assert_eq!(day(2027, 2, 28).plus_days(1), Some(day(2027, 3, 1)));
        assert_eq!(day(2026, 3, 1).plus_days(-1), Some(day(2026, 2, 28)));
        assert_eq!(day(2026, 11, 20).plus_days(7), Some(day(2026, 11, 27)));
        // Every day from 1900 to 2100 round-trips through its number.
        let mut at = day(1900, 1, 1);
        while at < day(2100, 1, 1) {
            assert_eq!(Date::from_serial(at.serial()), Some(at));
            at = at.plus_days(1).expect("next");
        }
    }

    #[test]
    fn the_weekday_is_right() {
        assert_eq!(day(1970, 1, 1).weekday(), 4, "a Thursday");
        assert_eq!(day(2026, 11, 20).weekday(), 5, "a Friday");
        assert_eq!(day(2000, 1, 1).weekday(), 6, "a Saturday");
        assert_eq!(day(2024, 2, 29).weekday(), 4, "a Thursday");
    }

    #[test]
    fn a_month_is_laid_out_in_weeks() {
        let grid = month_grid(2026, 11);
        // November 2026 starts on a Sunday and has 30 days: five rows, the last of two days.
        assert_eq!(grid.len(), 5);
        assert_eq!(grid[0][0], Some(day(2026, 11, 1)));
        assert_eq!(grid[4][1], Some(day(2026, 11, 30)));
        assert_eq!(grid[4][2], None);
        let days: Vec<Date> = grid.iter().flatten().flatten().copied().collect();
        assert_eq!(days.len(), 30);
        // A month that starts midweek has empty cells first, and is never empty or ragged.
        let grid = month_grid(2026, 2);
        assert_eq!(
            grid[0].iter().filter(|cell| cell.is_none()).count(),
            0,
            "starts on a Sunday"
        );
        let grid = month_grid(2027, 5);
        assert_eq!(grid[0][6], Some(day(2027, 5, 1)), "a Saturday");
        assert!(grid[0][..6].iter().all(Option::is_none));
        for month in 1..=12 {
            let count = month_grid(2028, month).iter().flatten().flatten().count();
            assert_eq!(count, usize::try_from(days_in_month(2028, month)).unwrap());
        }
        assert!(month_grid(2026, 13).is_empty());
    }

    #[test]
    fn months_roll_over_the_year() {
        assert_eq!(shift_month(2026, 11, 1), (2026, 12));
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 3, -14), (2025, 1));
        assert_eq!(shift_month(2026, 3, 0), (2026, 3));
    }

    #[test]
    fn a_short_day_leaves_out_this_year() {
        let today = day(2026, 11, 20);
        assert_eq!(day(2026, 12, 3).short(today), "Dec 3");
        assert_eq!(day(2027, 1, 9).short(today), "Jan 9, 2027");
    }
}
