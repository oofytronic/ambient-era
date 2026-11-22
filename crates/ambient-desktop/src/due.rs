//! When a task is due, and whether it repeats. All of it is the end of the task's own line, as
//! words a person could type: `call the clinic 2026-11-20 14:30 repeat:weekly`.
//!
//! The date is a real day written `YYYY-MM-DD`. The time, if there is one, is `HH:MM` on a 24
//! hour clock and means the wall clock wherever the person is. A repeat is `repeat:daily`,
//! `repeat:weekdays`, `repeat:weekly`, `repeat:monthly`, `repeat:yearly`, or an interval:
//! `repeat:3d`, `repeat:2w`, `repeat:2m`, `repeat:2y`. A repeat with no date is not read as
//! one, so it stays in the words. Nothing here reads a clock. The caller says what now is.

use crate::dates::Date;

/// A time of day.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Time {
    pub hour: u32,
    pub minute: u32,
}

impl Time {
    pub(crate) fn new(hour: u32, minute: u32) -> Option<Self> {
        (hour < 24 && minute < 60).then_some(Self { hour, minute })
    }

    /// Exactly `HH:MM`, two digits each.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let shape = bytes.len() == 5
            && bytes.get(2) == Some(&b':')
            && bytes
                .iter()
                .enumerate()
                .all(|(at, byte)| at == 2 || byte.is_ascii_digit());
        if !shape {
            return None;
        }
        Self::new(text.get(..2)?.parse().ok()?, text.get(3..)?.parse().ok()?)
    }

    pub(crate) fn minutes(self) -> u32 {
        self.hour * 60 + self.minute
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{:02}:{:02}", self.hour, self.minute)
    }
}

/// How often a repeat comes round.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Unit {
    Day,
    /// Monday to Friday.
    Weekday,
    Week,
    Month,
    Year,
}

/// Every `every` of a unit. `Weekday` is only ever every one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Repeat {
    pub every: u32,
    pub unit: Unit,
}

impl Repeat {
    /// `repeat:weekly`, `repeat:2w`, and the rest.
    pub(crate) fn parse(token: &str) -> Option<Self> {
        let word = token.strip_prefix("repeat:")?;
        let (every, unit) = match word {
            "daily" => (1, Unit::Day),
            "weekdays" => (1, Unit::Weekday),
            "weekly" => (1, Unit::Week),
            "monthly" => (1, Unit::Month),
            "yearly" => (1, Unit::Year),
            _ => {
                let digits = word.get(..word.len().checked_sub(1)?)?;
                let unit = match word.chars().last()? {
                    'd' => Unit::Day,
                    'w' => Unit::Week,
                    'm' => Unit::Month,
                    'y' => Unit::Year,
                    _ => return None,
                };
                // Plain digits, no sign and no leading zero, and more than one: one is a name.
                let plain = !digits.is_empty()
                    && !digits.starts_with('0')
                    && digits.bytes().all(|byte| byte.is_ascii_digit());
                let every: u32 = digits.parse().ok().filter(|_| plain)?;
                if !(2..=99).contains(&every) {
                    return None;
                }
                (every, unit)
            }
        };
        Some(Self { every, unit })
    }

    /// The token as written in a line.
    pub(crate) fn token(self) -> String {
        let letter = match self.unit {
            Unit::Day => 'd',
            Unit::Weekday => return "repeat:weekdays".to_owned(),
            Unit::Week => 'w',
            Unit::Month => 'm',
            Unit::Year => 'y',
        };
        if self.every == 1 {
            let name = match self.unit {
                Unit::Day => "daily",
                Unit::Week => "weekly",
                Unit::Month => "monthly",
                _ => "yearly",
            };
            format!("repeat:{name}")
        } else {
            format!("repeat:{}{letter}", self.every)
        }
    }

    /// For a person: `Every day`, `Every weekday`, `Every 2 weeks`.
    pub(crate) fn label(self) -> String {
        let (one, many) = match self.unit {
            Unit::Day => ("day", "days"),
            Unit::Weekday => return "Every weekday".to_owned(),
            Unit::Week => ("week", "weeks"),
            Unit::Month => ("month", "months"),
            Unit::Year => ("year", "years"),
        };
        if self.every == 1 {
            format!("Every {one}")
        } else {
            format!("Every {} {many}", self.every)
        }
    }

    /// The occurrence after `date`.
    fn step(self, date: Date) -> Option<Date> {
        let every = i64::from(self.every);
        match self.unit {
            Unit::Day => date.plus_days(every),
            Unit::Week => date.plus_days(7 * every),
            Unit::Month => date.plus_months(i32::try_from(self.every).ok()?),
            Unit::Year => date.plus_months(i32::try_from(self.every).ok()?.checked_mul(12)?),
            Unit::Weekday => {
                let mut next = date.plus_days(1)?;
                while matches!(next.weekday(), 0 | 6) {
                    next = next.plus_days(1)?;
                }
                Some(next)
            }
        }
    }
}

/// Where a due moment is compared to now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum State {
    /// The day or the minute has passed.
    Late,
    /// This day, and the minute has not come, or there is no minute.
    Today,
    /// A later day.
    Later,
}

/// A due day, with a time of day and a repeat if it has them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Due {
    pub date: Date,
    pub time: Option<Time>,
    pub repeat: Option<Repeat>,
}

/// Words, then the due tokens, with where the tokens start.
fn tokens(label: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, ch) in label.char_indices() {
        match (ch.is_whitespace(), start) {
            (false, None) => start = Some(at),
            (true, Some(from)) => {
                found.push((from, label.get(from..at).unwrap_or("")));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        found.push((from, label.get(from..).unwrap_or("")));
    }
    found
}

impl Due {
    /// The words of a task label, and its due moment, read from the end. A due moment is the last
    /// words: a day, then optionally a time, then optionally a repeat, in that order.
    pub(crate) fn split(label: &str) -> (&str, Option<Self>) {
        let found = tokens(label);
        let mut end = found.len();
        let repeat = end
            .checked_sub(1)
            .and_then(|at| found.get(at))
            .and_then(|(_, token)| Repeat::parse(token));
        if repeat.is_some() {
            end -= 1;
        }
        let at = |back: usize| end.checked_sub(back).and_then(|index| found.get(index));
        let with_time = at(1)
            .and_then(|(_, token)| Time::parse(token))
            .zip(at(2).and_then(|(_, token)| Date::parse(token)));
        let (date, time, first) = if let Some((time, date)) = with_time {
            (date, Some(time), at(2).map(|(start, _)| *start))
        } else if let Some(date) = at(1).and_then(|(_, token)| Date::parse(token)) {
            (date, None, at(1).map(|(start, _)| *start))
        } else {
            return (label.trim(), None);
        };
        let words = first
            .and_then(|start| label.get(..start))
            .unwrap_or(label)
            .trim();
        (words, Some(Self { date, time, repeat }))
    }

    pub(crate) fn of(label: &str) -> Option<Self> {
        Self::split(label).1
    }

    /// The words and the due moment as one line, in the one order.
    pub(crate) fn write(words: &str, due: Option<&Self>) -> String {
        let mut line = words.trim().replace(['\n', '\r'], " ");
        let Some(due) = due else {
            return line;
        };
        let mut push = |piece: &str| {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(piece);
        };
        push(&due.date.to_string());
        if let Some(time) = due.time {
            push(&time.to_string());
        }
        if let Some(repeat) = due.repeat {
            push(&repeat.token());
        }
        line
    }

    /// Late, today, or later, at `minutes` after midnight on `today`. A day with no time is late
    /// only once the day is over. A time is late once its minute is past.
    pub(crate) fn state(&self, today: Date, minutes: u32) -> State {
        match self.date.cmp(&today) {
            std::cmp::Ordering::Less => State::Late,
            std::cmp::Ordering::Greater => State::Later,
            std::cmp::Ordering::Equal => match self.time {
                Some(time) if time.minutes() < minutes => State::Late,
                _ => State::Today,
            },
        }
    }

    /// The day this comes round next after being done, or `None` if it does not repeat. It is the
    /// first occurrence after the due day that is also after `today`, so a task left for weeks
    /// comes back for the next day it falls on and not for one already gone.
    pub(crate) fn next_date(&self, today: Date) -> Option<Date> {
        let repeat = self.repeat?;
        let mut next = repeat.step(self.date)?;
        for _ in 0..50_000 {
            if next > today {
                return Some(next);
            }
            next = repeat.step(next)?;
        }
        None
    }

    /// This due moment on its next day.
    pub(crate) fn advanced(&self, today: Date) -> Option<Self> {
        Some(Self {
            date: self.next_date(today)?,
            ..*self
        })
    }

    /// For sorting: the day, then the time, with no time first on its day.
    pub(crate) fn key(&self) -> (i64, i64) {
        (
            self.date.serial(),
            self.time.map_or(-1, |time| i64::from(time.minutes())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Due, Repeat, State, Time, Unit};
    use crate::dates::Date;

    fn date(year: i32, month: u32, day: u32) -> Date {
        Date::new(year, month, day).expect("a day")
    }

    fn due(text: &str) -> Due {
        Due::of(text).expect(text)
    }

    #[test]
    fn a_time_is_two_digits_a_colon_and_two_digits() {
        assert_eq!(Time::parse("09:05"), Time::new(9, 5));
        assert_eq!(Time::parse("23:59").map(Time::minutes), Some(23 * 60 + 59));
        assert_eq!(
            Time::new(9, 5).map(|t| t.to_string()).as_deref(),
            Some("09:05")
        );
        for bad in [
            "9:05", "09:5", "24:00", "12:60", "12-30", "1230", "", "ab:cd", "-1:00", "12:30 ",
        ] {
            assert_eq!(Time::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn every_repeat_reads_back_as_it_was_written() {
        for token in [
            "repeat:daily",
            "repeat:weekdays",
            "repeat:weekly",
            "repeat:monthly",
            "repeat:yearly",
            "repeat:3d",
            "repeat:2w",
            "repeat:2m",
            "repeat:12m",
            "repeat:2y",
            "repeat:99d",
        ] {
            let repeat = Repeat::parse(token).expect(token);
            assert_eq!(repeat.token(), token, "{token}");
        }
        for bad in [
            "repeat:",
            "repeat:hourly",
            "repeat:1d",
            "repeat:0w",
            "repeat:02w",
            "repeat:100d",
            "repeat:-2w",
            "repeat:+2w",
            "repeat:2x",
            "repeat:w",
            "Repeat:daily",
            "repeat: daily",
            "daily",
            "repeat:2weekdays",
        ] {
            assert_eq!(Repeat::parse(bad), None, "{bad:?}");
        }
        assert_eq!(
            Repeat::parse("repeat:2w").map(Repeat::label).as_deref(),
            Some("Every 2 weeks")
        );
        assert_eq!(
            Repeat {
                every: 1,
                unit: Unit::Day
            }
            .label(),
            "Every day"
        );
        assert_eq!(
            Repeat::parse("repeat:weekdays")
                .map(Repeat::label)
                .as_deref(),
            Some("Every weekday")
        );
    }

    #[test]
    fn the_end_of_a_line_is_read_as_a_day_a_time_and_a_repeat() {
        let (words, found) = Due::split("call the clinic 2026-11-20");
        assert_eq!(words, "call the clinic");
        assert_eq!(
            found,
            Some(Due {
                date: date(2026, 11, 20),
                time: None,
                repeat: None
            })
        );
        let (words, found) = Due::split("call  the clinic 2026-11-20 14:30");
        assert_eq!(
            words, "call  the clinic",
            "the spacing in the words is left"
        );
        assert_eq!(found.and_then(|due| due.time), Time::new(14, 30));
        let (words, found) = Due::split("water the plants 2026-11-20 08:00 repeat:weekly");
        assert_eq!(words, "water the plants");
        let found = found.expect("due");
        assert_eq!(
            found.repeat.map(Repeat::label).as_deref(),
            Some("Every week")
        );
        assert_eq!(found.time, Time::new(8, 0));
        // A day and a repeat with no time.
        assert_eq!(due("pay rent 2026-12-01 repeat:monthly").time, None);
        // Things that are not due moments are left in the words.
        for text in [
            "call tomorrow",
            "call 2026-02-30",
            "just 14:30",
            "call 14:30 2026-11-20",
            "repeat:weekly",
            "call repeat:weekly",
            "call 2026-11-20 9:00",
            "call 2026-11-20 repeat:weekly 14:30",
            "",
        ] {
            let (words, found) = Due::split(text);
            // A time before the day, or a trailing time, can still leave a day: only check no
            // repeat and no time was invented.
            if let Some(found) = found {
                assert!(
                    found.time.is_none() && found.repeat.is_none(),
                    "{text:?} {found:?}"
                );
            }
            assert!(text.starts_with(words) || words.is_empty(), "{text:?}");
        }
        assert_eq!(Due::of("call repeat:weekly"), None, "a repeat needs a day");
        assert_eq!(Due::split("call repeat:weekly").0, "call repeat:weekly");
        assert_eq!(
            Due::of("call 2026-11-20 9:00"),
            None,
            "a time that is not a time is not read"
        );
    }

    #[test]
    fn a_due_moment_is_written_in_the_one_order() {
        let full = Due {
            date: date(2026, 11, 20),
            time: Time::new(8, 5),
            repeat: Repeat::parse("repeat:2w"),
        };
        assert_eq!(
            Due::write("water the plants", Some(&full)),
            "water the plants 2026-11-20 08:05 repeat:2w"
        );
        assert_eq!(Due::write("  plain  ", None), "plain");
        assert_eq!(
            Due::write(
                "",
                Some(&Due {
                    time: None,
                    repeat: None,
                    ..full
                })
            ),
            "2026-11-20"
        );
        // What is written is what is read.
        let line = Due::write("water the plants", Some(&full));
        assert_eq!(Due::split(&line), ("water the plants", Some(full)));
    }

    #[test]
    fn a_task_is_late_today_or_later_down_to_the_minute() {
        let today = date(2026, 11, 20);
        let state = |text: &str, minutes: u32| due(text).state(today, minutes);
        assert_eq!(state("a 2026-11-19", 0), State::Late);
        assert_eq!(state("a 2026-11-21", 23 * 60 + 59), State::Later);
        assert_eq!(
            state("a 2026-11-20", 23 * 60 + 59),
            State::Today,
            "a day with no time is today all day"
        );
        assert_eq!(state("a 2026-11-20 09:00", 8 * 60 + 59), State::Today);
        assert_eq!(
            state("a 2026-11-20 09:00", 9 * 60),
            State::Today,
            "at the minute, not yet late"
        );
        assert_eq!(state("a 2026-11-20 09:00", 9 * 60 + 1), State::Late);
    }

    #[test]
    fn a_repeat_comes_round_on_the_next_day_it_falls_on() {
        let today = date(2026, 11, 20);
        let next = |text: &str| due(text).next_date(today).map(|day| day.to_string());
        assert_eq!(
            next("a 2026-11-20 repeat:daily").as_deref(),
            Some("2026-11-21")
        );
        assert_eq!(
            next("a 2026-11-20 repeat:weekly").as_deref(),
            Some("2026-11-27")
        );
        assert_eq!(
            next("a 2026-11-20 repeat:2w").as_deref(),
            Some("2026-12-04")
        );
        assert_eq!(
            next("a 2026-11-20 repeat:monthly").as_deref(),
            Some("2026-12-20")
        );
        assert_eq!(
            next("a 2026-11-20 repeat:yearly").as_deref(),
            Some("2027-11-20")
        );
        assert_eq!(next("a 2026-11-20"), None, "no repeat, no next");
        // Left for a while: the next one is after today, on the same rhythm.
        assert_eq!(
            next("a 2026-11-10 repeat:weekly").as_deref(),
            Some("2026-11-24")
        );
        assert_eq!(
            next("a 2026-01-01 repeat:daily").as_deref(),
            Some("2026-11-21")
        );
        assert_eq!(
            next("a 2025-11-01 repeat:monthly").as_deref(),
            Some("2026-12-01")
        );
        // Done early, the next is still after the one it was for.
        assert_eq!(
            next("a 2026-11-25 repeat:daily").as_deref(),
            Some("2026-11-26")
        );
        // Weekdays skip the weekend. 2026-11-20 is a Friday.
        assert_eq!(
            next("a 2026-11-20 repeat:weekdays").as_deref(),
            Some("2026-11-23")
        );
        assert_eq!(
            next("a 2026-11-18 repeat:weekdays").as_deref(),
            Some("2026-11-23")
        );
        // The end of a month is clamped, and a leap day falls back to the 28th.
        let jan = Due::of("a 2026-01-31 repeat:monthly").expect("due");
        assert_eq!(jan.next_date(date(2026, 1, 31)), Some(date(2026, 2, 28)));
        let leap = Due::of("a 2028-02-29 repeat:yearly").expect("due");
        assert_eq!(leap.next_date(date(2028, 2, 29)), Some(date(2029, 2, 28)));
        // The time and the repeat stay, and the line is written again with the new day.
        let moved = due("a 2026-11-20 09:30 repeat:weekly")
            .advanced(today)
            .expect("next");
        assert_eq!(
            Due::write("a", Some(&moved)),
            "a 2026-11-27 09:30 repeat:weekly"
        );
        assert_eq!(
            Repeat {
                every: 1,
                unit: Unit::Weekday
            }
            .token(),
            "repeat:weekdays"
        );
    }

    #[test]
    fn due_moments_sort_by_day_then_time_with_no_time_first() {
        let key = |text: &str| due(text).key();
        assert!(key("a 2026-11-20") < key("a 2026-11-21"));
        assert!(key("a 2026-11-20") < key("a 2026-11-20 00:00"));
        assert!(key("a 2026-11-20 08:00") < key("a 2026-11-20 09:00"));
        assert!(key("a 2026-11-20 23:59") < key("a 2026-11-21"));
    }
}
