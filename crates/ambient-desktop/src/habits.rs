//! Habits: what is kept, when one is due, and how it is going.
//!
//! A habit is a line in a note called Habits, in the words a person could type:
//! `- Meditate habit:daily`, `- Run habit:3/week`, `- Stretch habit:mon,wed,fri`,
//! `- Water habit:daily*8`. The `habit:` word is the last on the line. A line without it is
//! just words, so the note can hold anything else too. The check-ins are not in the note. They
//! are kept on this computer, by the habit's name and the day.
//!
//! Nothing here reads a clock or a file. The caller says what today is, and gives the check-ins.
//! It is built to be kind: how a habit is going is said as "5 of the last 7", a missed day does
//! not take away what came before it, and a streak is counted from the last day it was due.

use std::collections::BTreeMap;

use crate::dates::Date;

/// The check-ins of one habit: a day as `YYYY-MM-DD`, and how many times it was done that day.
pub(crate) type Counts = BTreeMap<String, u32>;

/// When a habit is due.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Schedule {
    /// On these weekdays, bit 0 for Sunday through bit 6 for Saturday.
    Days(u8),
    /// This many days in any week, whichever days they are.
    PerWeek(u32),
}

const ALL: u8 = 0b111_1111;
const WEEKDAYS: u8 = 0b011_1110;
const WEEKENDS: u8 = 0b100_0001;
const NAMES: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const LONG: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

impl Schedule {
    pub(crate) const DAILY: Self = Self::Days(ALL);

    /// The token, as written after `habit:`.
    fn token(self) -> String {
        match self {
            Self::Days(ALL) => "daily".to_owned(),
            Self::Days(WEEKDAYS) => "weekdays".to_owned(),
            Self::Days(WEEKENDS) => "weekends".to_owned(),
            Self::Days(mask) => NAMES
                .iter()
                .enumerate()
                .filter(|(at, _)| mask & (1 << at) != 0)
                .map(|(_, name)| *name)
                .collect::<Vec<_>>()
                .join(","),
            Self::PerWeek(count) => format!("{count}/week"),
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "daily" => return Some(Self::DAILY),
            "weekdays" => return Some(Self::Days(WEEKDAYS)),
            "weekends" => return Some(Self::Days(WEEKENDS)),
            _ => {}
        }
        if let Some(count) = token.strip_suffix("/week") {
            let count: u32 = count.parse().ok()?;
            return (1..=7).contains(&count).then_some(Self::PerWeek(count));
        }
        let mut mask = 0_u8;
        for name in token.split(',') {
            let at = NAMES.iter().position(|day| *day == name)?;
            mask |= 1 << at;
        }
        (mask != 0).then_some(Self::Days(mask))
    }

    /// For a person.
    pub(crate) fn label(self) -> String {
        match self {
            Self::Days(ALL) => "Every day".to_owned(),
            Self::Days(WEEKDAYS) => "Weekdays".to_owned(),
            Self::Days(WEEKENDS) => "Weekends".to_owned(),
            Self::Days(mask) => NAMES
                .iter()
                .enumerate()
                .filter(|(at, _)| mask & (1 << at) != 0)
                .map(|(_, name)| {
                    let mut day = (*name).to_owned();
                    if let Some(first) = day.get_mut(..1) {
                        first.make_ascii_uppercase();
                    }
                    day
                })
                .collect::<Vec<_>>()
                .join(", "),
            Self::PerWeek(1) => "Once a week".to_owned(),
            Self::PerWeek(count) => format!("{count} times a week"),
        }
    }
}

/// One habit, as read from its line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Habit {
    pub name: String,
    pub schedule: Schedule,
    /// How many times in a day counts as done: 1, or more for a count such as eight glasses.
    pub target: u32,
    /// The line in the note.
    pub line: usize,
}

impl Habit {
    /// What its check-ins are kept under.
    pub(crate) fn key(&self) -> String {
        key_of(&self.name)
    }

    /// The line as it is written in the note.
    pub(crate) fn write(&self) -> String {
        let mut token = format!("habit:{}", self.schedule.token());
        if self.target > 1 {
            token.push('*');
            token.push_str(&self.target.to_string());
        }
        format!("- {} {token}", self.name)
    }
}

/// A habit's name as a key: capitals and extra space do not tell two apart.
pub(crate) fn key_of(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The habit on one line of a note, if it is one.
fn parse_line(line: &str, at: usize) -> Option<Habit> {
    let rest = line.trim_start();
    let rest = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))?
        .trim();
    let (name, token) = rest.rsplit_once(char::is_whitespace)?;
    let token = token.strip_prefix("habit:")?;
    let (schedule, target) = match token.split_once('*') {
        Some((schedule, target)) => (schedule, target.parse::<u32>().ok()?),
        None => (token, 1),
    };
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || !(1..=99).contains(&target) {
        return None;
    }
    Some(Habit {
        name,
        schedule: Schedule::from_token(schedule)?,
        target,
        line: at,
    })
}

/// Every habit in a note, in the order written. A name that is written twice is one habit.
pub(crate) fn parse_note(body: &str) -> Vec<Habit> {
    let mut found: Vec<Habit> = Vec::new();
    for (at, line) in body.split('\n').enumerate() {
        if let Some(habit) = parse_line(line, at)
            && !found.iter().any(|have| have.key() == habit.key())
        {
            found.push(habit);
        }
    }
    found
}

/// A typed phrase, read for a schedule: `run 3 times a week`, `read every weekday`,
/// `stretch mon wed fri`, `water 8 times a day`. The words left are the name, and a phrase with no
/// schedule in it is every day.
pub(crate) fn parse_phrase(text: &str) -> (String, Schedule, u32) {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let keys: Vec<String> = tokens
        .iter()
        .map(|token| token.trim_matches([',', '.']).to_lowercase())
        .collect();
    let mut used = vec![false; tokens.len()];
    let mut schedule: Option<Schedule> = None;
    let mut target = 1_u32;
    let key = |at: usize| keys.get(at).map_or("", String::as_str);
    let mut at = 0;
    while at < keys.len() {
        let mut took = 0;
        // `3 times a week`, `3x a week`, `3 per week`, `3/week`
        if let Some(count) = count_before(key(at))
            && let Some(next) = week_or_day(&keys, at)
        {
            took = next.0;
            match next.1 {
                Per::Week if schedule.is_none() && (1..=7).contains(&count) => {
                    schedule = Some(Schedule::PerWeek(count));
                }
                Per::Day if target == 1 && (2..=99).contains(&count) => target = count,
                _ => took = 0,
            }
        } else if let Some(count) = key(at)
            .strip_suffix("/week")
            .and_then(|count| count.parse::<u32>().ok())
            .filter(|count| (1..=7).contains(count))
            .filter(|_| schedule.is_none())
        {
            schedule = Some(Schedule::PerWeek(count));
            took = 1;
        } else if matches!(key(at), "daily" | "everyday") && schedule.is_none() {
            schedule = Some(Schedule::DAILY);
            took = 1;
        } else if key(at) == "every" && matches!(key(at + 1), "day") && schedule.is_none() {
            schedule = Some(Schedule::DAILY);
            took = 2;
        } else if matches!(key(at), "weekdays") && schedule.is_none() {
            schedule = Some(Schedule::Days(WEEKDAYS));
            took = 1;
        } else if key(at) == "every" && key(at + 1) == "weekday" && schedule.is_none() {
            schedule = Some(Schedule::Days(WEEKDAYS));
            took = 2;
        } else if matches!(key(at), "weekends") && schedule.is_none() {
            schedule = Some(Schedule::Days(WEEKENDS));
            took = 1;
        } else if schedule.is_none() {
            let lead = usize::from(matches!(key(at), "every" | "on"));
            let (mask, days) = day_group(&keys, at + lead, lead > 0);
            if mask != 0 {
                schedule = Some(Schedule::Days(mask));
                took = lead + days;
            }
        }
        if took == 0 {
            at += 1;
        } else {
            for slot in used.iter_mut().skip(at).take(took) {
                *slot = true;
            }
            at += took;
        }
    }
    let name: Vec<&str> = tokens
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(token, _)| *token)
        .collect();
    let name = name.join(" ");
    if name.is_empty() {
        // Everything was taken for a schedule, so none of it was.
        return (tokens.join(" "), Schedule::DAILY, 1);
    }
    (name, schedule.unwrap_or(Schedule::DAILY), target)
}

enum Per {
    Week,
    Day,
}

/// `3`, `3x`: a number that may be a count.
fn count_before(key: &str) -> Option<u32> {
    let digits = key.strip_suffix('x').unwrap_or(key);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// What follows a count: `times a week`, `x a day`, `per week`, `a week`, `/week`.
fn week_or_day(keys: &[String], at: usize) -> Option<(usize, Per)> {
    let key = |back: usize| keys.get(at + back).map_or("", String::as_str);
    let mut look = 1;
    if matches!(key(look), "times" | "time") {
        look += 1;
    }
    if matches!(key(look), "a" | "per" | "each") {
        look += 1;
    }
    match key(look) {
        "week" | "weekly" if look > 1 || key(0).ends_with('x') => Some((look + 1, Per::Week)),
        "day" | "daily" if look > 1 || key(0).ends_with('x') => Some((look + 1, Per::Day)),
        _ => None,
    }
}

/// Weekday names, joined by spaces, commas, `and`, or `/`: how many tokens, and which days. A short
/// name only counts after `every` or `on`, or beside another, so `sat` in `sat nav` is not one.
fn day_group(keys: &[String], from: usize, led: bool) -> (u8, usize) {
    let mut mask = 0_u8;
    let mut count = 0;
    let mut at = from;
    while let Some(key) = keys.get(at) {
        if matches!(key.as_str(), "and" | "&") && count > 0 {
            at += 1;
            continue;
        }
        let parts: Vec<&str> = key.split('/').collect();
        let mut here = 0_u8;
        for part in &parts {
            let part = part.trim_end_matches('s');
            let found = LONG
                .iter()
                .position(|day| *day == part)
                .or_else(|| NAMES.iter().position(|day| *day == part));
            if let Some(index) = found {
                here |= 1 << index;
            } else {
                here = 0;
                break;
            }
        }
        if here == 0 {
            break;
        }
        mask |= here;
        count += 1;
        at += 1;
    }
    // Two or more names, or a name that was led in, are days. A bare short name is not.
    let short_only = count == 1
        && keys
            .get(from)
            .is_some_and(|key| key.len() == 3 && !LONG.contains(&key.as_str()));
    if count == 0 || (short_only && !led) {
        return (0, 0);
    }
    // The `and` between names is taken with them, the one after the last is not.
    let mut end = at;
    while end > from
        && keys
            .get(end - 1)
            .is_some_and(|key| key == "and" || key == "&")
    {
        end -= 1;
    }
    (mask, end - from)
}

fn day_key(day: Date) -> String {
    day.to_string()
}

/// How many times it was done on a day.
pub(crate) fn count_on(counts: &Counts, day: Date) -> u32 {
    counts.get(&day_key(day)).copied().unwrap_or(0)
}

/// Whether the day counts as done: it was done as many times as the habit asks.
pub(crate) fn done_on(habit: &Habit, counts: &Counts, day: Date) -> bool {
    count_on(counts, day) >= habit.target
}

/// The first day of the week `day` is in.
pub(crate) fn week_start(day: Date, monday_first: bool) -> Date {
    let weekday = day.weekday();
    let back = if monday_first {
        (weekday + 6) % 7
    } else {
        weekday
    };
    day.plus_days(-i64::from(back)).unwrap_or(day)
}

/// How many days of the week `day` is in were done, for a habit that asks for some days a week.
pub(crate) fn week_done(habit: &Habit, counts: &Counts, day: Date, monday_first: bool) -> u32 {
    let start = week_start(day, monday_first);
    let mut done = 0;
    for step in 0..7 {
        if let Some(each) = start.plus_days(step)
            && done_on(habit, counts, each)
        {
            done += 1;
        }
    }
    done
}

/// Whether the day is one it is asked for. A habit of some days a week is asked for on any day
/// of the week until the days are met.
pub(crate) fn scheduled(habit: &Habit, counts: &Counts, day: Date, monday_first: bool) -> bool {
    match habit.schedule {
        Schedule::Days(mask) => mask & (1 << day.weekday()) != 0,
        Schedule::PerWeek(goal) => {
            done_on(habit, counts, day) || week_done(habit, counts, day, monday_first) < goal
        }
    }
}

/// Whether it belongs on the list for `today`: asked for today, or done today.
pub(crate) fn due_today(habit: &Habit, counts: &Counts, today: Date, monday_first: bool) -> bool {
    scheduled(habit, counts, today, monday_first) || done_on(habit, counts, today)
}

/// What a day was, for the dots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Day {
    Done,
    /// Asked for, and not done, on a day that is over.
    Missed,
    /// A day it was not asked for.
    Rest,
    /// Today, not yet done.
    Open,
    /// Not here yet.
    Ahead,
}

pub(crate) fn day_state(
    habit: &Habit,
    counts: &Counts,
    day: Date,
    today: Date,
    monday_first: bool,
) -> Day {
    if done_on(habit, counts, day) {
        return Day::Done;
    }
    if day > today {
        return Day::Ahead;
    }
    match habit.schedule {
        Schedule::Days(mask) => {
            if mask & (1 << day.weekday()) == 0 {
                Day::Rest
            } else if day == today {
                Day::Open
            } else {
                Day::Missed
            }
        }
        // A habit of some days a week misses nothing in the middle of a week. It is open until the
        // week is over, and a day not done is a day of rest.
        Schedule::PerWeek(_) => {
            if day == today && scheduled(habit, counts, day, monday_first) {
                Day::Open
            } else {
                Day::Rest
            }
        }
    }
}

/// The last `n` days, ending today, with what each was.
pub(crate) fn recent_days(
    habit: &Habit,
    counts: &Counts,
    today: Date,
    n: u32,
    monday_first: bool,
) -> Vec<(Date, Day)> {
    let mut out = Vec::new();
    for back in (0..n).rev() {
        if let Some(day) = today.plus_days(-i64::from(back)) {
            out.push((day, day_state(habit, counts, day, today, monday_first)));
        }
    }
    out
}

/// How the last seven days went: how many were done, out of how many were asked for. Today is
/// left out of what was asked for until it is done, so an unfinished day is not a miss.
pub(crate) fn last_week(
    habit: &Habit,
    counts: &Counts,
    today: Date,
    monday_first: bool,
) -> (u32, u32) {
    match habit.schedule {
        Schedule::PerWeek(goal) => {
            let mut done = 0;
            for back in 0..7 {
                if let Some(day) = today.plus_days(-back)
                    && done_on(habit, counts, day)
                {
                    done += 1;
                }
            }
            (done.min(goal), goal)
        }
        Schedule::Days(mask) => {
            let (mut done, mut asked) = (0, 0);
            for back in 0..7 {
                let Some(day) = today.plus_days(-back) else {
                    continue;
                };
                let is_done = done_on(habit, counts, day);
                let was_asked = mask & (1 << day.weekday()) != 0;
                if was_asked && (day != today || is_done) {
                    asked += 1;
                }
                if was_asked && is_done {
                    done += 1;
                }
            }
            let _ = monday_first;
            (done, asked)
        }
    }
}

/// The run it is on and the best it has had: days in a row for a habit asked for on days, weeks in a
/// row for one asked for some days a week. A day that was not asked for does not break a run, and
/// a day not yet done does not either.
pub(crate) fn streaks(
    habit: &Habit,
    counts: &Counts,
    today: Date,
    monday_first: bool,
) -> (u32, u32) {
    let Some(first) = counts
        .keys()
        .filter_map(|key| Date::parse(key))
        .min()
        .filter(|first| *first <= today)
    else {
        return (0, 0);
    };
    match habit.schedule {
        Schedule::Days(mask) => day_streaks(habit, counts, mask, first, today),
        Schedule::PerWeek(goal) => week_streaks(habit, counts, goal, first, today, monday_first),
    }
}

fn day_streaks(habit: &Habit, counts: &Counts, mask: u8, first: Date, today: Date) -> (u32, u32) {
    let (mut run, mut best) = (0_u32, 0_u32);
    let mut day = first;
    // From the first check-in to today, so the run is what it is on the last day.
    while day <= today {
        if mask & (1 << day.weekday()) != 0 {
            if done_on(habit, counts, day) {
                run += 1;
                best = best.max(run);
            } else if day != today {
                run = 0;
            }
        }
        let Some(next) = day.plus_days(1) else {
            break;
        };
        day = next;
    }
    (run, best)
}

fn week_streaks(
    habit: &Habit,
    counts: &Counts,
    goal: u32,
    first: Date,
    today: Date,
    monday_first: bool,
) -> (u32, u32) {
    let (mut run, mut best) = (0_u32, 0_u32);
    let mut start = week_start(first, monday_first);
    let this = week_start(today, monday_first);
    while start <= this {
        let met = week_done(habit, counts, start, monday_first) >= goal;
        if met {
            run += 1;
            best = best.max(run);
        } else if start != this {
            run = 0;
        }
        let Some(next) = start.plus_days(7) else {
            break;
        };
        start = next;
    }
    (run, best)
}

/// The unit a run is counted in, for a person.
pub(crate) fn streak_label(habit: &Habit, run: u32) -> String {
    let (one, many) = match habit.schedule {
        Schedule::Days(_) => ("day", "days"),
        Schedule::PerWeek(_) => ("week", "weeks"),
    };
    format!("{run} {} in a row", if run == 1 { one } else { many })
}

/// The next number of times for a check: one more, and back to none after the last. A habit done
/// once a day is done or not.
pub(crate) fn next_count(habit: &Habit, now: u32) -> u32 {
    if now >= habit.target { 0 } else { now + 1 }
}

#[cfg(test)]
mod tests {
    use super::{
        Counts, Day, Habit, Schedule, count_on, day_state, done_on, due_today, key_of, last_week,
        next_count, parse_note, parse_phrase, recent_days, streaks, week_start,
    };
    use crate::dates::Date;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::new(y, m, d).expect("a day")
    }

    /// Friday 2026-11-20.
    fn today() -> Date {
        day(2026, 11, 20)
    }

    fn habit(schedule: Schedule, target: u32) -> Habit {
        Habit {
            name: "Run".to_owned(),
            schedule,
            target,
            line: 0,
        }
    }

    fn done(days: &[(i32, u32, u32)]) -> Counts {
        days.iter()
            .map(|(y, m, d)| (day(*y, *m, *d).to_string(), 1))
            .collect()
    }

    #[test]
    fn a_habit_is_a_line_and_the_line_reads_back_the_same() {
        let body = "My habits\n- Meditate habit:daily\n- Run habit:3/week\n* Stretch habit:mon,wed,fri\n- Water habit:daily*8\n- Read habit:weekdays\n- plain item\n- Bad habit:funday\n- Sleep early habit:weekends\n";
        let found = parse_note(body);
        let names: Vec<&str> = found.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(
            names,
            ["Meditate", "Run", "Stretch", "Water", "Read", "Sleep early"]
        );
        assert_eq!(found[1].schedule, Schedule::PerWeek(3));
        assert_eq!(found[3].target, 8);
        assert_eq!(found[2].line, 3);
        for habit in &found {
            let line = habit.write();
            let again = parse_note(&line).remove(0);
            assert_eq!(
                (again.name, again.schedule, again.target),
                (habit.name.clone(), habit.schedule, habit.target)
            );
        }
        assert_eq!(found[2].write(), "- Stretch habit:mon,wed,fri");
        assert_eq!(found[3].write(), "- Water habit:daily*8");
        // A name written twice is one habit, whatever its capitals.
        assert_eq!(
            parse_note("- Run habit:daily\n- run  habit:weekdays").len(),
            1
        );
        assert_eq!(key_of("  Sleep   EARLY "), "sleep early");
    }

    #[test]
    fn a_typed_phrase_is_read_for_its_schedule_and_count() {
        assert_eq!(
            parse_phrase("meditate"),
            ("meditate".to_owned(), Schedule::DAILY, 1)
        );
        assert_eq!(
            parse_phrase("read every weekday").1,
            Schedule::Days(0b011_1110)
        );
        assert_eq!(parse_phrase("read every weekday").0, "read");
        assert_eq!(parse_phrase("run 3 times a week").1, Schedule::PerWeek(3));
        assert_eq!(parse_phrase("run 3x a week").0, "run");
        assert_eq!(parse_phrase("run 3 per week").1, Schedule::PerWeek(3));
        assert_eq!(
            parse_phrase("water 8 times a day"),
            ("water".to_owned(), Schedule::DAILY, 8)
        );
        assert_eq!(
            parse_phrase("stretch mon wed fri").1,
            Schedule::Days(0b010_1010)
        );
        assert_eq!(
            parse_phrase("stretch every monday and thursday").1,
            Schedule::Days(0b001_0010)
        );
        assert_eq!(
            parse_phrase("stretch every monday and thursday").0,
            "stretch"
        );
        assert_eq!(parse_phrase("call mom sundays").1, Schedule::Days(1));
        assert_eq!(parse_phrase("clean on sat").1, Schedule::Days(0b100_0000));
        assert_eq!(parse_phrase("walk weekends").1, Schedule::Days(0b100_0001));
        // What is not sure is a name. `sat` alone is a word.
        assert_eq!(
            parse_phrase("fix sat nav"),
            ("fix sat nav".to_owned(), Schedule::DAILY, 1)
        );
        assert_eq!(
            parse_phrase("every day"),
            ("every day".to_owned(), Schedule::DAILY, 1),
            "all schedule, no name"
        );
        assert_eq!(
            parse_phrase("run 9 times a week").1,
            Schedule::DAILY,
            "no such week"
        );
    }

    #[test]
    fn a_habit_is_due_on_its_days_and_a_week_habit_until_its_days_are_met() {
        let weekdays = habit(Schedule::Days(0b011_1110), 1);
        let counts = Counts::new();
        assert!(due_today(&weekdays, &counts, today(), true));
        assert!(
            !due_today(&weekdays, &counts, day(2026, 11, 21), true),
            "a Saturday"
        );
        // Done on a day it was not asked for, it is on that day's list, done.
        let bonus = done(&[(2026, 11, 21)]);
        assert!(due_today(&weekdays, &bonus, day(2026, 11, 21), true));

        let thrice = habit(Schedule::PerWeek(3), 1);
        // Monday the 16th, Tuesday, and Wednesday are done: the week is met on Friday.
        let met = done(&[(2026, 11, 16), (2026, 11, 17), (2026, 11, 18)]);
        assert!(!due_today(&thrice, &met, today(), true));
        let two = done(&[(2026, 11, 16), (2026, 11, 17)]);
        assert!(due_today(&thrice, &two, today(), true));
        // A week starting on Sunday is a different week: Sunday the 15th starts this one.
        assert_eq!(week_start(today(), true), day(2026, 11, 16));
        assert_eq!(week_start(today(), false), day(2026, 11, 15));
        assert_eq!(week_start(day(2026, 11, 15), true), day(2026, 11, 9));
    }

    #[test]
    fn a_count_of_times_a_day_is_done_when_it_is_reached() {
        let water = habit(Schedule::DAILY, 3);
        let mut counts = Counts::new();
        assert_eq!(next_count(&water, 0), 1);
        counts.insert(today().to_string(), 2);
        assert!(!done_on(&water, &counts, today()));
        assert_eq!(count_on(&counts, today()), 2);
        assert_eq!(next_count(&water, 2), 3);
        assert_eq!(next_count(&water, 3), 0, "one more press clears it");
        counts.insert(today().to_string(), 3);
        assert!(done_on(&water, &counts, today()));
        let once = habit(Schedule::DAILY, 1);
        assert_eq!(next_count(&once, 0), 1);
        assert_eq!(next_count(&once, 1), 0);
    }

    #[test]
    fn the_week_is_said_kindly_and_today_is_not_a_miss_until_it_is_over() {
        let daily = habit(Schedule::DAILY, 1);
        // Done on the 14th, 15th, 17th, 18th, and 19th. Missed the 16th and 20th is today.
        let counts = done(&[
            (2026, 11, 14),
            (2026, 11, 15),
            (2026, 11, 17),
            (2026, 11, 18),
            (2026, 11, 19),
        ]);
        assert_eq!(
            last_week(&daily, &counts, today(), true),
            (5, 6),
            "today is left out until done"
        );
        let counts_today = {
            let mut with = counts.clone();
            with.insert(today().to_string(), 1);
            with
        };
        assert_eq!(last_week(&daily, &counts_today, today(), true), (6, 7));
        let states: Vec<Day> = recent_days(&daily, &counts, today(), 7, true)
            .into_iter()
            .map(|(_, state)| state)
            .collect();
        assert_eq!(
            states,
            [
                Day::Done,
                Day::Done,
                Day::Missed,
                Day::Done,
                Day::Done,
                Day::Done,
                Day::Open
            ]
        );
        let weekdays = habit(Schedule::Days(0b011_1110), 1);
        assert_eq!(
            day_state(&weekdays, &Counts::new(), day(2026, 11, 21), today(), true),
            Day::Ahead
        );
        assert_eq!(
            day_state(&weekdays, &Counts::new(), day(2026, 11, 15), today(), true),
            Day::Rest
        );
        let week = habit(Schedule::PerWeek(3), 1);
        assert_eq!(
            last_week(
                &week,
                &done(&[(2026, 11, 18), (2026, 11, 19)]),
                today(),
                true
            ),
            (2, 3)
        );
        assert_eq!(
            last_week(
                &week,
                &done(&[
                    (2026, 11, 17),
                    (2026, 11, 18),
                    (2026, 11, 19),
                    (2026, 11, 20)
                ]),
                today(),
                true
            ),
            (3, 3),
            "more than asked is still met"
        );
    }

    #[test]
    fn a_run_counts_from_the_last_day_it_was_asked_for_and_rest_days_do_not_break_it() {
        let daily = habit(Schedule::DAILY, 1);
        let counts = done(&[
            (2026, 11, 16),
            (2026, 11, 17),
            (2026, 11, 18),
            (2026, 11, 19),
        ]);
        assert_eq!(
            streaks(&daily, &counts, today(), true),
            (4, 4),
            "today is not done, and not a break"
        );
        let mut with_today = counts.clone();
        with_today.insert(today().to_string(), 1);
        assert_eq!(streaks(&daily, &with_today, today(), true), (5, 5));
        // A miss ends the run, and the best is kept.
        let broken = done(&[
            (2026, 11, 10),
            (2026, 11, 11),
            (2026, 11, 12),
            (2026, 11, 14),
            (2026, 11, 19),
        ]);
        assert_eq!(streaks(&daily, &broken, today(), true), (1, 3));
        // Weekdays only: Friday the 13th, then Monday the 16th, is a run.
        let weekdays = habit(Schedule::Days(0b011_1110), 1);
        let over_the_weekend = done(&[
            (2026, 11, 12),
            (2026, 11, 13),
            (2026, 11, 16),
            (2026, 11, 17),
        ]);
        assert_eq!(
            streaks(&weekdays, &over_the_weekend, day(2026, 11, 17), true),
            (4, 4)
        );
        // Nothing done at all is no run.
        assert_eq!(streaks(&daily, &Counts::new(), today(), true), (0, 0));
        // Three a week, counted in weeks. The week of the 9th met it, the 16th has two so far.
        let week = habit(Schedule::PerWeek(2), 1);
        let weeks = done(&[
            (2026, 11, 9),
            (2026, 11, 11),
            (2026, 11, 16),
            (2026, 11, 17),
        ]);
        assert_eq!(streaks(&week, &weeks, today(), true), (2, 2));
        let missed_week = done(&[(2026, 11, 2), (2026, 11, 3), (2026, 11, 16)]);
        assert_eq!(
            streaks(&week, &missed_week, today(), true),
            (0, 1),
            "the week of the 9th had none"
        );
    }
}
