//! What can be said about a task besides its words and its due moment: how much it matters, and
//! the plain-language way of typing a due moment.
//!
//! A priority is a token in the words of the task line, `!1`, `!2`, or `!3`, ahead of the due
//! moment: `call the clinic !1 2026-11-20 14:30`. `Due::split` reads the date from the end of
//! the line, so the token sits before it and the date still reads.
//!
//! `parse` reads a line typed by a person, `call vet tomorrow 3pm every week !2`, and says what
//! it found: the words left, and the due moment and priority. It reads nothing but the text and
//! what `today` and `minutes` are, and it only ever takes a phrase it is sure of. What it does
//! not know stays in the words, so the worst it can do is leave something as it was typed.

use crate::dates::{Date, MONTHS};
use crate::due::{Due, Repeat, Time};

/// How much a task matters. One is the most.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Priority {
    One,
    Two,
    Three,
}

impl Priority {
    /// `!1`, `!2`, or `!3`, as written in a line.
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::One => "!1",
            Self::Two => "!2",
            Self::Three => "!3",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "!1" => Some(Self::One),
            "!2" => Some(Self::Two),
            "!3" => Some(Self::Three),
            _ => None,
        }
    }

    /// For a person.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::One => "Priority 1",
            Self::Two => "Priority 2",
            Self::Three => "Priority 3",
        }
    }

    /// What one press on the flag does: none, then one, two, three, then none again.
    pub(crate) fn next(current: Option<Self>) -> Option<Self> {
        match current {
            None => Some(Self::One),
            Some(Self::One) => Some(Self::Two),
            Some(Self::Two) => Some(Self::Three),
            Some(Self::Three) => None,
        }
    }
}

/// The words with any priority token taken out, and the first priority found. Words with no token
/// are given back as they were.
pub(crate) fn split_priority(words: &str) -> (String, Option<Priority>) {
    let mut found = None;
    let mut kept: Vec<&str> = Vec::new();
    let mut any = false;
    for token in words.split_whitespace() {
        if let Some(priority) = Priority::from_token(token) {
            any = true;
            found = found.or(Some(priority));
        } else {
            kept.push(token);
        }
    }
    if any {
        (kept.join(" "), found)
    } else {
        (words.trim().to_owned(), None)
    }
}

/// The words with the priority token on the end. No priority leaves the words as they are.
pub(crate) fn with_priority(words: &str, priority: Option<Priority>) -> String {
    let (words, _) = split_priority(words);
    match priority {
        None => words,
        Some(priority) if words.is_empty() => priority.token().to_owned(),
        Some(priority) => format!("{words} {}", priority.token()),
    }
}

/// What was found in a typed line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Quick {
    /// The words that are left, which are the task.
    pub words: String,
    pub due: Option<Due>,
    pub priority: Option<Priority>,
    /// Whether anything was taken out of the line.
    pub found: bool,
}

/// One thing a phrase says.
enum Piece {
    Date(Date),
    Time(Time),
    Both(Date, Time),
    Repeat(Repeat),
    /// `every monday`: the repeat, and the first day it falls on.
    RepeatFrom(Date, Repeat),
    Priority(Priority),
    Tonight,
}

const DAYS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

/// A typed token, lowercase and without the marks that end a phrase.
fn key_of(token: &str) -> String {
    token.trim_end_matches([',', '.', ';', ':']).to_lowercase()
}

/// The weekday, 0 for Sunday, that this names. A short name only counts after a word like `on`.
fn weekday_of(key: &str, short_ok: bool) -> Option<u32> {
    for (index, name) in DAYS.iter().enumerate() {
        if key == *name {
            return u32::try_from(index).ok();
        }
        if short_ok
            && let Some(short) = name.get(..3)
            && (key == short || (key.len() > 3 && name.starts_with(key) && key.len() < name.len()))
        {
            return u32::try_from(index).ok();
        }
    }
    None
}

/// The month, 1 to 12, that this names. A short name needs a day after it, which the caller
/// checks, so `may` and `mar` alone are never a month.
fn month_of(key: &str) -> Option<u32> {
    let key = if key == "sept" { "sep" } else { key };
    for (index, name) in MONTHS.iter().enumerate() {
        let lower = name.to_lowercase();
        let short = lower.get(..3)?;
        if key == lower || key == short {
            return u32::try_from(index).ok()?.checked_add(1);
        }
    }
    None
}

/// The first day with this weekday: from today on, or from tomorrow on.
fn day_on(today: Date, weekday: u32, after_today: bool) -> Option<Date> {
    let now = today.weekday();
    let mut ahead = (weekday + 7 - now) % 7;
    if ahead == 0 && after_today {
        ahead = 7;
    }
    today.plus_days(i64::from(ahead))
}

fn number(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 4 || !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    key.parse().ok()
}

/// A day of the month: `5`, `5th`, `22nd`.
fn day_number(key: &str) -> Option<u32> {
    let digits = key.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = key.get(digits.len()..).unwrap_or("");
    if !matches!(suffix, "" | "st" | "nd" | "rd" | "th") {
        return None;
    }
    number(digits).filter(|day| (1..=31).contains(day))
}

/// This year's day, or next year's when this year's has gone by.
fn month_day(today: Date, month: u32, day: u32) -> Option<Date> {
    let this = Date::new(today.year, month, day)?;
    if this >= today {
        Some(this)
    } else {
        Date::new(today.year.checked_add(1)?, month, day)
    }
}

fn the(keys: &[String], at: usize) -> &str {
    keys.get(at).map_or("", String::as_str)
}

/// `3` or `3:30`, which is a time once `am` or `pm` follows it.
fn clock_like(key: &str) -> bool {
    match key.split_once(':') {
        Some((hour, minute)) => {
            number(hour).is_some() && minute.len() == 2 && number(minute).is_some()
        }
        None => number(key).is_some(),
    }
}

/// A time written on one or two tokens: `3pm`, `3:30pm`, `15:00`, `noon`, `3 pm`.
/// The second is how many tokens it took.
fn time_at(keys: &[String], at: usize) -> Option<(Time, usize)> {
    let key = the(keys, at);
    if key == "noon" {
        return Some((Time::new(12, 0)?, 1));
    }
    let (body, meridiem, took) = if let Some(rest) = key.strip_suffix("am") {
        (rest, Some(false), 1)
    } else if let Some(rest) = key.strip_suffix("pm") {
        (rest, Some(true), 1)
    } else if matches!(the(keys, at + 1), "am" | "pm") && clock_like(key) {
        (key, Some(the(keys, at + 1) == "pm"), 2)
    } else {
        (key, None, 1)
    };
    let (hour, minute) = match body.split_once(':') {
        Some((hour, minute)) if minute.len() == 2 => (number(hour)?, number(minute)?),
        None if meridiem.is_some() => (number(body)?, 0),
        _ => return None,
    };
    let hour = match meridiem {
        Some(pm) => {
            if !(1..=12).contains(&hour) {
                return None;
            }
            hour % 12 + if pm { 12 } else { 0 }
        }
        // `15:00` is a time and `3:00` with no am or pm is too early to be sure of.
        None if body.contains(':') && body.len() >= 4 => hour,
        None => return None,
    };
    Some((Time::new(hour, minute)?, took))
}

/// `every day`, `every 3 weeks`, `every other month`, `every monday`.
fn every_at(keys: &[String], at: usize, today: Date) -> Option<(Piece, usize)> {
    if the(keys, at) != "every" {
        return None;
    }
    let unit_of = |word: &str| match word.trim_end_matches('s') {
        "day" => Some('d'),
        "week" => Some('w'),
        "month" => Some('m'),
        "year" => Some('y'),
        _ => None,
    };
    let word = the(keys, at + 1);
    let make = |text: &str| Repeat::parse(&format!("repeat:{text}"));
    if matches!(word, "weekday" | "weekdays") {
        return Some((Piece::Repeat(make("weekdays")?), 2));
    }
    if word == "day" || word == "week" || word == "month" || word == "year" {
        let name = match word {
            "day" => "daily",
            "week" => "weekly",
            "month" => "monthly",
            _ => "yearly",
        };
        return Some((Piece::Repeat(make(name)?), 2));
    }
    if word == "other"
        && let Some(letter) = unit_of(the(keys, at + 2))
    {
        return Some((Piece::Repeat(make(&format!("2{letter}"))?), 3));
    }
    if let (Some(count), Some(letter)) = (number(word), unit_of(the(keys, at + 2)))
        && (2..=99).contains(&count)
    {
        return Some((Piece::Repeat(make(&format!("{count}{letter}"))?), 3));
    }
    let weekday = weekday_of(word, true)?;
    Some((
        Piece::RepeatFrom(day_on(today, weekday, false)?, make("weekly")?),
        2,
    ))
}

/// `in 3 days`, `in a week`, `in 2 hours`, `in 30 minutes`.
fn in_at(keys: &[String], at: usize, today: Date, minutes: u32) -> Option<(Piece, usize)> {
    if the(keys, at) != "in" {
        return None;
    }
    let count = match the(keys, at + 1) {
        "a" | "an" => 1,
        other => number(other).filter(|n| (1..=999).contains(n))?,
    };
    let unit = the(keys, at + 2).trim_end_matches('s');
    let piece = match unit {
        "day" => Piece::Date(today.plus_days(i64::from(count))?),
        "week" => Piece::Date(today.plus_days(i64::from(count) * 7)?),
        "month" => Piece::Date(today.plus_months(i32::try_from(count).ok()?)?),
        "year" => Piece::Date(today.plus_months(i32::try_from(count).ok()?.checked_mul(12)?)?),
        "hour" | "hr" | "minute" | "min" => {
            let delta = if unit.starts_with('h') {
                count.checked_mul(60)?
            } else {
                count
            };
            let total = minutes.checked_add(delta)?;
            let days = i64::from(total / 1440);
            let rest = total % 1440;
            Piece::Both(today.plus_days(days)?, Time::new(rest / 60, rest % 60)?)
        }
        _ => return None,
    };
    Some((piece, 3))
}

/// One phrase, from `at` on, and how many tokens it was. `lead` says a word like `on` came first.
fn piece_at(
    keys: &[String],
    at: usize,
    today: Date,
    minutes: u32,
    lead: bool,
) -> Option<(Piece, usize)> {
    let key = the(keys, at);
    if let Some(priority) = Priority::from_token(key) {
        return Some((Piece::Priority(priority), 1));
    }
    if let Some(date) = Date::parse(key) {
        return Some((Piece::Date(date), 1));
    }
    // The way a repeat is written in a line is read as well, so a line copied from a note adds.
    if let Some(every) = Repeat::parse(key) {
        return Some((Piece::Repeat(every), 1));
    }
    match key {
        "today" => return Some((Piece::Date(today), 1)),
        "tomorrow" | "tmrw" | "tmr" => return Some((Piece::Date(today.plus_days(1)?), 1)),
        "tonight" => return Some((Piece::Tonight, 1)),
        _ => {}
    }
    if key == "next" {
        match the(keys, at + 1) {
            "week" => {
                let ahead = (8 - today.weekday()) % 7;
                let ahead = if ahead == 0 { 7 } else { ahead };
                return Some((Piece::Date(today.plus_days(i64::from(ahead))?), 2));
            }
            "month" => return Some((Piece::Date(today.plus_months(1)?), 2)),
            word => {
                let weekday = weekday_of(word, true)?;
                return Some((Piece::Date(day_on(today, weekday, true)?), 2));
            }
        }
    }
    if key == "this" && the(keys, at + 1) == "weekend" {
        return Some((Piece::Date(day_on(today, 6, false)?), 2));
    }
    if let Some(weekday) = weekday_of(key, lead) {
        return Some((Piece::Date(day_on(today, weekday, false)?), 1));
    }
    if let Some(found) = in_at(keys, at, today, minutes) {
        return Some(found);
    }
    if let Some(found) = every_at(keys, at, today) {
        return Some(found);
    }
    if let Some(month) = month_of(key)
        && let Some(day) = day_number(the(keys, at + 1))
    {
        return Some((Piece::Date(month_day(today, month, day)?), 2));
    }
    if let Some(day) = day_number(key)
        && let Some(month) = month_of(the(keys, at + 1))
    {
        return Some((Piece::Date(month_day(today, month, day)?), 2));
    }
    if let Some((time, took)) = time_at(keys, at) {
        return Some((Piece::Time(time), took));
    }
    // `daily` and its kind only count as the last word, so `read daily log` stays as typed.
    let last = at + 1 == keys.len();
    let name = match key {
        "daily" | "everyday" => "daily",
        "weekly" => "weekly",
        "monthly" => "monthly",
        "yearly" | "annually" => "yearly",
        _ => return None,
    };
    last.then(|| Repeat::parse(&format!("repeat:{name}")))
        .flatten()
        .map(|repeat| (Piece::Repeat(repeat), 1))
}

/// Read a typed task line. See the module.
pub(crate) fn parse(text: &str, today: Date, minutes: u32) -> Quick {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let keys: Vec<String> = tokens.iter().map(|token| key_of(token)).collect();
    let mut used = vec![false; tokens.len()];
    let mut date: Option<Date> = None;
    let mut time: Option<Time> = None;
    let mut repeat: Option<Repeat> = None;
    let mut priority: Option<Priority> = None;
    let mut tonight = false;
    let mut at = 0;
    while at < keys.len() {
        let lead = matches!(the(&keys, at), "on" | "by" | "due" | "at" | "this");
        let found =
            piece_at(&keys, at, today, minutes, false).map(|(piece, took)| (piece, took, 0));
        let found = found.or_else(|| {
            lead.then(|| piece_at(&keys, at + 1, today, minutes, true))
                .flatten()
                .map(|(piece, took)| (piece, took, 1))
        });
        let Some((piece, took, skip)) = found else {
            at += 1;
            continue;
        };
        // One of each kind. A second is left in the words.
        let taken = match piece {
            Piece::Date(day) if date.is_none() => {
                date = Some(day);
                true
            }
            Piece::Time(when) if time.is_none() => {
                time = Some(when);
                true
            }
            Piece::Both(day, when) if date.is_none() && time.is_none() => {
                date = Some(day);
                time = Some(when);
                true
            }
            Piece::Repeat(every) if repeat.is_none() => {
                repeat = Some(every);
                true
            }
            Piece::RepeatFrom(day, every) if repeat.is_none() => {
                repeat = Some(every);
                date = date.or(Some(day));
                true
            }
            Piece::Priority(level) if priority.is_none() => {
                priority = Some(level);
                true
            }
            Piece::Tonight if date.is_none() => {
                date = Some(today);
                tonight = true;
                true
            }
            _ => false,
        };
        if !taken {
            at += 1;
            continue;
        }
        let span = skip + took;
        for slot in used.iter_mut().skip(at).take(span) {
            *slot = true;
        }
        at += span;
    }
    let words: Vec<&str> = tokens
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(token, _)| *token)
        .collect();
    let nothing = Quick {
        words: text.split_whitespace().collect::<Vec<_>>().join(" "),
        due: None,
        priority: None,
        found: false,
    };
    if words.is_empty() || !used.iter().any(|used| *used) {
        return nothing;
    }
    if tonight && time.is_none() {
        time = Time::new(20, 0);
    }
    // A time alone is today's, unless the minute has gone by. A repeat alone starts today.
    let day = date.or_else(|| match (time, repeat) {
        (Some(when), _) if when.minutes() <= minutes => today.plus_days(1),
        (Some(_), _) | (None, Some(_)) => Some(today),
        (None, None) => None,
    });
    Quick {
        words: words.join(" "),
        due: day.map(|date| Due { date, time, repeat }),
        priority,
        found: true,
    }
}

#[cfg(test)]
mod tests {
    use super::{Priority, parse, split_priority, with_priority};
    use crate::dates::Date;
    use crate::due::Time;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::new(y, m, d).expect("a day")
    }

    /// Friday, 20 November 2026, ten in the morning.
    fn read(text: &str) -> super::Quick {
        parse(text, day(2026, 11, 20), 10 * 60)
    }

    #[test]
    fn a_priority_is_a_token_in_the_words_and_comes_out_and_goes_back() {
        assert_eq!(
            split_priority("call !2 the clinic"),
            ("call the clinic".to_owned(), Some(Priority::Two))
        );
        assert_eq!(
            split_priority("plain  words"),
            ("plain  words".to_owned(), None)
        );
        assert_eq!(
            split_priority("wow!1"),
            ("wow!1".to_owned(), None),
            "only a whole token"
        );
        assert_eq!(with_priority("call", Some(Priority::One)), "call !1");
        assert_eq!(with_priority("call !3", Some(Priority::One)), "call !1");
        assert_eq!(with_priority("call !3", None), "call");
        assert_eq!(with_priority("", Some(Priority::Three)), "!3");
        assert_eq!(Priority::next(None), Some(Priority::One));
        assert_eq!(Priority::next(Some(Priority::Three)), None);
    }

    #[test]
    fn a_day_and_a_time_are_read_from_anywhere_in_the_line() {
        let found = read("call vet tomorrow 3pm");
        assert_eq!(found.words, "call vet");
        let due = found.due.expect("due");
        assert_eq!(due.date, day(2026, 11, 21));
        assert_eq!(due.time, Time::new(15, 0));
        assert_eq!(read("tomorrow call vet").words, "call vet");
        assert_eq!(
            read("lunch at noon").due.expect("due").time,
            Time::new(12, 0)
        );
        assert_eq!(
            read("call at 15:30").due.expect("due").time,
            Time::new(15, 30)
        );
        assert_eq!(
            read("call at 3:30 pm").due.expect("due").time,
            Time::new(15, 30)
        );
        assert_eq!(
            read("write 2026-12-01 report").due.expect("due").date,
            day(2026, 12, 1)
        );
    }

    #[test]
    fn a_time_alone_is_today_unless_it_has_gone_by() {
        assert_eq!(
            read("stretch at 11am").due.expect("due").date,
            day(2026, 11, 20)
        );
        assert_eq!(
            read("stretch at 9am").due.expect("due").date,
            day(2026, 11, 21)
        );
        let soon = read("call mom in 2 hours").due.expect("due");
        assert_eq!(
            (soon.date, soon.time),
            (day(2026, 11, 20), Time::new(12, 0))
        );
        let late = parse("call mom in 3 hours", day(2026, 11, 20), 22 * 60)
            .due
            .expect("due");
        assert_eq!((late.date, late.time), (day(2026, 11, 21), Time::new(1, 0)));
        let night = read("read tonight").due.expect("due");
        assert_eq!(
            (night.date, night.time),
            (day(2026, 11, 20), Time::new(20, 0))
        );
    }

    #[test]
    fn weekdays_months_and_spans_of_days() {
        // 2026-11-20 is a Friday.
        assert_eq!(
            read("submit by friday").due.expect("due").date,
            day(2026, 11, 20)
        );
        assert_eq!(read("submit by friday").words, "submit");
        assert_eq!(
            read("submit on monday").due.expect("due").date,
            day(2026, 11, 23)
        );
        assert_eq!(
            read("submit next friday").due.expect("due").date,
            day(2026, 11, 27)
        );
        assert_eq!(
            read("clean next week").due.expect("due").date,
            day(2026, 11, 23)
        );
        assert_eq!(
            read("plan next month").due.expect("due").date,
            day(2026, 12, 20)
        );
        assert_eq!(
            read("hike this weekend").due.expect("due").date,
            day(2026, 11, 21)
        );
        assert_eq!(
            read("party nov 30").due.expect("due").date,
            day(2026, 11, 30)
        );
        assert_eq!(
            read("dentist on 5 dec").due.expect("due").date,
            day(2026, 12, 5)
        );
        assert_eq!(
            read("party nov 3").due.expect("due").date,
            day(2027, 11, 3),
            "that day has gone"
        );
        assert_eq!(
            read("renew in 2 weeks").due.expect("due").date,
            day(2026, 12, 4)
        );
        assert_eq!(
            read("renew in a month").due.expect("due").date,
            day(2026, 12, 20)
        );
    }

    #[test]
    fn a_repeat_is_read_and_starts_today_or_on_its_day() {
        let monthly = read("pay rent every month").due.expect("due");
        assert_eq!(monthly.repeat.expect("repeat").token(), "repeat:monthly");
        assert_eq!(monthly.date, day(2026, 11, 20));
        assert_eq!(
            read("water every 3 days")
                .due
                .expect("due")
                .repeat
                .expect("r")
                .token(),
            "repeat:3d"
        );
        assert_eq!(
            read("sweep every other week")
                .due
                .expect("due")
                .repeat
                .expect("r")
                .token(),
            "repeat:2w"
        );
        assert_eq!(
            read("standup every weekday 9:30")
                .due
                .expect("due")
                .repeat
                .expect("r")
                .token(),
            "repeat:weekdays"
        );
        let weekly = read("bins every monday").due.expect("due");
        assert_eq!(
            (weekly.date, weekly.repeat.expect("r").token()),
            (day(2026, 11, 23), "repeat:weekly".to_owned())
        );
        assert_eq!(
            read("meditate daily")
                .due
                .expect("due")
                .repeat
                .expect("r")
                .token(),
            "repeat:daily"
        );
    }

    #[test]
    fn what_is_not_sure_stays_as_typed() {
        for text in [
            "buy sun cream",
            "read daily log",
            "march of the penguins",
            "tomorrow",
            "call 5 people",
            "may the best win",
            "meet at 3",
        ] {
            let found = read(text);
            assert!(!found.found, "{text}");
            assert_eq!(found.words, text);
            assert!(found.due.is_none());
        }
        // A second day is not a day: it stays in the words.
        let two = read("compare friday and monday");
        assert_eq!(two.due.expect("due").date, day(2026, 11, 20));
        assert_eq!(two.words, "compare and monday");
    }

    #[test]
    fn a_due_moment_already_written_as_a_line_reads_the_same() {
        let found = read("buy milk 2026-11-28 08:30 repeat:weekly");
        assert_eq!(found.words, "buy milk");
        let due = found.due.expect("due");
        assert_eq!(due.date, day(2026, 11, 28));
        assert_eq!(due.time, Time::new(8, 30));
        assert_eq!(due.repeat.expect("repeat").token(), "repeat:weekly");
    }

    #[test]
    fn a_priority_is_read_with_the_rest() {
        let found = read("file taxes !1 by friday");
        assert_eq!(found.priority, Some(Priority::One));
        assert_eq!(found.words, "file taxes");
        assert!(found.found);
    }
}
