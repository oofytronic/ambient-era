//! A time zone, as the system keeps it in a `TZif` file, read once and answered fast.
//!
//! A zone file lists the changes it knows about, and then says in its last line, as a POSIX
//! rule such as `PST8PDT,M3.2.0,M11.1.0`, how the clock changes every year after them. Files
//! made in the last few years list only the changes up to the last time a government changed
//! the rule, so for any date after that the offset comes from the rule and not from the list.
//! Reading only the list gives the wrong hour for the whole summer. This reads both.
//!
//! Nothing here reads a file or a clock. The caller gives the bytes.

use crate::dates::{Date, days_in_month};

/// One way the clock can be set in a zone.
#[derive(Clone, Debug)]
struct Kind {
    /// Seconds east of UTC.
    offset: i32,
    abbr: String,
}

/// Where a day falls in a year, as a POSIX rule writes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleDay {
    /// `Mm.w.d`: week `w` (5 is the last) of month `m`, on weekday `d` (0 is Sunday).
    Month { month: u32, week: u32, weekday: u32 },
    /// `Jn`: day 1 to 365 of the year, never counting February 29.
    Julian(i64),
    /// `n`: day 0 to 365 of the year, counting February 29.
    Zero(i64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Dst {
    abbr: String,
    /// Seconds east of UTC.
    east: i32,
    start: RuleDay,
    /// Seconds after midnight, standard time.
    start_at: i32,
    end: RuleDay,
    /// Seconds after midnight, daylight time.
    end_at: i32,
}

/// The rule for every year after the last listed change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rule {
    std_abbr: String,
    /// Seconds east of UTC.
    std_east: i32,
    dst: Option<Dst>,
}

fn name(text: &mut &str) -> Option<String> {
    if let Some(rest) = text.strip_prefix('<') {
        let end = rest.find('>')?;
        let found = rest.get(..end)?.to_owned();
        *text = rest.get(end + 1..)?;
        return (!found.is_empty()).then_some(found);
    }
    let end = text
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    if end < 3 {
        return None;
    }
    let found = text.get(..end)?.to_owned();
    *text = text.get(end..)?;
    Some(found)
}

/// `[+-]hh[:mm[:ss]]`, as seconds.
fn clock_seconds(text: &mut &str) -> Option<i32> {
    let (sign, rest) = match text.as_bytes().first()? {
        b'-' => (-1, text.get(1..)?),
        b'+' => (1, text.get(1..)?),
        _ => (1, *text),
    };
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == ':'))
        .unwrap_or(rest.len());
    let mut total = 0_i32;
    let mut scale = 3_600_i32;
    let parts = rest.get(..end)?;
    if parts.is_empty() {
        return None;
    }
    for part in parts.split(':') {
        total = total.checked_add(part.parse::<i32>().ok()?.checked_mul(scale)?)?;
        scale /= 60;
    }
    *text = rest.get(end..)?;
    Some(sign * total)
}

fn rule_day(text: &mut &str) -> Option<RuleDay> {
    if let Some(rest) = text.strip_prefix('M') {
        let end = rest.find([',', '/']).unwrap_or(rest.len());
        let mut parts = rest.get(..end)?.split('.');
        let month: u32 = parts.next()?.parse().ok()?;
        let week: u32 = parts.next()?.parse().ok()?;
        let weekday: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some()
            || !(1..=12).contains(&month)
            || !(1..=5).contains(&week)
            || weekday > 6
        {
            return None;
        }
        *text = rest.get(end..)?;
        return Some(RuleDay::Month {
            month,
            week,
            weekday,
        });
    }
    let (julian, rest) = match text.strip_prefix('J') {
        Some(rest) => (true, rest),
        None => (false, *text),
    };
    let end = rest.find([',', '/']).unwrap_or(rest.len());
    let n: i64 = rest.get(..end)?.parse().ok()?;
    *text = rest.get(end..)?;
    if julian {
        (1..=365).contains(&n).then_some(RuleDay::Julian(n))
    } else {
        (0..=365).contains(&n).then_some(RuleDay::Zero(n))
    }
}

/// `day[/time]`, the time defaulting to 02:00.
fn rule_change(text: &mut &str) -> Option<(RuleDay, i32)> {
    let day = rule_day(text)?;
    let at = if let Some(rest) = text.strip_prefix('/') {
        let mut rest = rest;
        let found = clock_seconds(&mut rest)?;
        *text = rest;
        found
    } else {
        7_200
    };
    Some((day, at))
}

impl Rule {
    /// Read a footer such as `PST8PDT,M3.2.0,M11.1.0` or `IST-5:30` or `<-03>3`.
    pub(crate) fn parse(footer: &str) -> Option<Self> {
        let mut text = footer.trim();
        let std_abbr = name(&mut text)?;
        // The offset is hours west of Greenwich, so east is its opposite.
        let std_east = clock_seconds(&mut text)?.checked_neg()?;
        if text.is_empty() {
            return Some(Self {
                std_abbr,
                std_east,
                dst: None,
            });
        }
        let dst_abbr = name(&mut text)?;
        let east = if text.starts_with(',') || text.is_empty() {
            std_east.checked_add(3_600)?
        } else {
            clock_seconds(&mut text)?.checked_neg()?
        };
        let ((start, start_at), (end, end_at)) = if let Some(rest) = text.strip_prefix(',') {
            let mut rest = rest;
            let first = rule_change(&mut rest)?;
            rest = rest.strip_prefix(',')?;
            let second = rule_change(&mut rest)?;
            (first, second)
        } else {
            // No rule given: the United States one, as the C library does.
            (
                (
                    RuleDay::Month {
                        month: 3,
                        week: 2,
                        weekday: 0,
                    },
                    7_200,
                ),
                (
                    RuleDay::Month {
                        month: 11,
                        week: 1,
                        weekday: 0,
                    },
                    7_200,
                ),
            )
        };
        Some(Self {
            std_abbr,
            std_east,
            dst: Some(Dst {
                abbr: dst_abbr,
                east,
                start,
                start_at,
                end,
                end_at,
            }),
        })
    }

    /// Days since 1970-01-01 of the day the rule names, in `year`.
    fn serial(day: RuleDay, year: i32) -> Option<i64> {
        let january = Date::new(year, 1, 1)?.serial();
        match day {
            RuleDay::Month {
                month,
                week,
                weekday,
            } => {
                let first = Date::new(year, month, 1)?;
                let ahead = (weekday + 7 - first.weekday()) % 7;
                let mut at = 1 + ahead + (week - 1) * 7;
                if at > days_in_month(year, month) {
                    at -= 7;
                }
                Some(first.serial() + i64::from(at) - 1)
            }
            RuleDay::Julian(n) => {
                let leap = days_in_month(year, 2) == 29;
                Some(january + n - 1 + i64::from(leap && n >= 60))
            }
            RuleDay::Zero(n) => Some(january + n),
        }
    }

    /// The kind of clock at this moment: seconds east of UTC, and its name.
    fn at(&self, unix: i64) -> (i32, &str) {
        let standard = (self.std_east, self.std_abbr.as_str());
        let Some(dst) = &self.dst else {
            return standard;
        };
        let local = unix.saturating_add(i64::from(self.std_east));
        let Some(year) = Date::from_serial(local.div_euclid(86_400)).map(|day| day.year) else {
            return standard;
        };
        let (Some(start), Some(end)) = (Self::serial(dst.start, year), Self::serial(dst.end, year))
        else {
            return standard;
        };
        // The change to daylight time is on the standard clock, the change back on the daylight one.
        let start = start * 86_400 + i64::from(dst.start_at) - i64::from(self.std_east);
        let end = end * 86_400 + i64::from(dst.end_at) - i64::from(dst.east);
        let summer = if start < end {
            unix >= start && unix < end
        } else {
            // South of the equator the summer runs over New Year.
            unix >= start || unix < end
        };
        if summer {
            (dst.east, dst.abbr.as_str())
        } else {
            standard
        }
    }
}

/// A zone: its listed changes, and the rule for after them.
#[derive(Clone, Debug)]
pub(crate) struct Zone {
    /// The file this was read from, to tell whether it has changed.
    raw: Vec<u8>,
    times: Vec<i64>,
    kinds: Vec<u8>,
    types: Vec<Kind>,
    rule: Option<Rule>,
}

type Counts = [usize; 6];

fn be32(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_be_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn header(data: &[u8], at: usize) -> Option<(u8, Counts)> {
    if data.get(at..at.checked_add(4)?)? != b"TZif" {
        return None;
    }
    let version = *data.get(at.checked_add(4)?)?;
    let mut counts = [0_usize; 6];
    for (slot, count) in counts.iter_mut().enumerate() {
        let start = at.checked_add(20)?.checked_add(slot * 4)?;
        *count = usize::try_from(be32(data, start)?).ok()?;
    }
    Some((version, counts))
}

/// Bytes in a data block with this header: isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt.
fn block_len(counts: Counts, time_size: usize) -> Option<usize> {
    let [ut, std, leap, times, types, chars] = counts;
    times
        .checked_mul(time_size)?
        .checked_add(times)?
        .checked_add(types.checked_mul(6)?)?
        .checked_add(chars)?
        .checked_add(leap.checked_mul(time_size.checked_add(4)?)?)?
        .checked_add(std)?
        .checked_add(ut)
}

impl Zone {
    /// UTC, for a computer that gave no zone.
    pub(crate) fn utc() -> Self {
        Self {
            raw: Vec::new(),
            times: Vec::new(),
            kinds: Vec::new(),
            types: vec![Kind {
                offset: 0,
                abbr: "UTC".to_owned(),
            }],
            rule: None,
        }
    }

    /// Read a `TZif` file. `None` when it is not one, or is cut short.
    pub(crate) fn parse(data: &[u8]) -> Option<Self> {
        let (version, first) = header(data, 0)?;
        let (counts, body, time_size, footer_at) = if version >= b'2' {
            let second = 44_usize.checked_add(block_len(first, 4)?)?;
            let (_, counts) = header(data, second)?;
            let body = second.checked_add(44)?;
            let end = body.checked_add(block_len(counts, 8)?)?;
            (counts, body, 8, Some(end))
        } else {
            (first, 44, 4, None)
        };
        let [_, _, _, time_count, type_count, char_count] = counts;
        let mut times = Vec::with_capacity(time_count);
        for nth in 0..time_count {
            let at = body.checked_add(nth.checked_mul(time_size)?)?;
            let raw = data.get(at..at.checked_add(time_size)?)?;
            times.push(if time_size == 8 {
                i64::from_be_bytes(raw.try_into().ok()?)
            } else {
                i64::from(i32::from_be_bytes(raw.try_into().ok()?))
            });
        }
        let kinds_at = body.checked_add(time_count.checked_mul(time_size)?)?;
        let kinds = data
            .get(kinds_at..kinds_at.checked_add(time_count)?)?
            .to_vec();
        let types_at = kinds_at.checked_add(time_count)?;
        let chars_at = types_at.checked_add(type_count.checked_mul(6)?)?;
        let chars = data.get(chars_at..chars_at.checked_add(char_count)?)?;
        let mut types = Vec::with_capacity(type_count);
        for nth in 0..type_count {
            let at = types_at.checked_add(nth * 6)?;
            let offset = be32(data, at)?;
            let index = usize::from(*data.get(at.checked_add(5)?)?);
            let abbr = chars
                .get(index..)
                .map(|rest| rest.split(|byte| *byte == 0).next().unwrap_or(&[]))
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                .unwrap_or_default();
            types.push(Kind { offset, abbr });
        }
        if types.is_empty() || kinds.iter().any(|kind| usize::from(*kind) >= types.len()) {
            return None;
        }
        let rule = footer_at
            .and_then(|at| data.get(at..))
            .and_then(|rest| rest.strip_prefix(b"\n"))
            .and_then(|rest| rest.split(|byte| *byte == b'\n').next())
            .and_then(|line| std::str::from_utf8(line).ok())
            .and_then(Rule::parse);
        Some(Self {
            raw: data.to_vec(),
            times,
            kinds,
            types,
            rule,
        })
    }

    /// Whether `data` is the file this zone was read from.
    pub(crate) fn is_from(&self, data: &[u8]) -> bool {
        !self.raw.is_empty() && self.raw == data
    }

    fn listed(&self, unix: i64) -> Option<&Kind> {
        let at = self.times.partition_point(|time| *time <= unix);
        match at.checked_sub(1) {
            None => self.types.first(),
            Some(last) => self
                .kinds
                .get(last)
                .and_then(|kind| self.types.get(usize::from(*kind))),
        }
    }

    /// Whether the moment is after every change the file lists, so the rule decides.
    fn after_list(&self, unix: i64) -> bool {
        self.times.last().is_none_or(|last| unix >= *last)
    }

    /// Seconds east of UTC at this moment.
    pub(crate) fn offset_at(&self, unix: i64) -> i32 {
        if let Some(rule) = self.rule.as_ref().filter(|_| self.after_list(unix)) {
            return rule.at(unix).0;
        }
        self.listed(unix).map_or(0, |kind| kind.offset)
    }

    /// The zone's short name at this moment, such as `PST`.
    pub(crate) fn abbr_at(&self, unix: i64) -> String {
        if let Some(rule) = self.rule.as_ref().filter(|_| self.after_list(unix)) {
            return rule.at(unix).1.to_owned();
        }
        self.listed(unix)
            .map_or_else(String::new, |kind| kind.abbr.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{Rule, RuleDay, Zone};

    /// Footer, moment, and the offset a correct implementation gives, made with Python's `zoneinfo`.
    const VECTORS: &[(&str, i64, i32)] = &[
        ("PST8PDT,M3.2.0,M11.1.0", 1_768_478_400, -28_800),
        ("PST8PDT,M3.2.0,M11.1.0", 1_772_963_999, -28_800),
        ("PST8PDT,M3.2.0,M11.1.0", 1_772_964_000, -25_200),
        ("PST8PDT,M3.2.0,M11.1.0", 1_784_116_800, -25_200),
        ("PST8PDT,M3.2.0,M11.1.0", 1_793_523_599, -25_200),
        ("PST8PDT,M3.2.0,M11.1.0", 1_793_523_600, -28_800),
        ("PST8PDT,M3.2.0,M11.1.0", 2_368_094_400, -28_800),
        ("PST8PDT,M3.2.0,M11.1.0", 2_372_925_599, -28_800),
        ("PST8PDT,M3.2.0,M11.1.0", 2_372_925_600, -25_200),
        ("PST8PDT,M3.2.0,M11.1.0", 2_383_732_800, -25_200),
        ("PST8PDT,M3.2.0,M11.1.0", 2_393_485_199, -25_200),
        ("PST8PDT,M3.2.0,M11.1.0", 2_393_485_200, -28_800),
        ("GMT0BST,M3.5.0/1,M10.5.0", 1_768_478_400, 0),
        ("GMT0BST,M3.5.0/1,M10.5.0", 1_774_745_999, 0),
        ("GMT0BST,M3.5.0/1,M10.5.0", 1_774_746_000, 3_600),
        ("GMT0BST,M3.5.0/1,M10.5.0", 1_784_116_800, 3_600),
        ("GMT0BST,M3.5.0/1,M10.5.0", 1_792_889_999, 3_600),
        ("GMT0BST,M3.5.0/1,M10.5.0", 1_792_890_000, 0),
        ("GMT0BST,M3.5.0/1,M10.5.0", 2_374_102_799, 0),
        ("GMT0BST,M3.5.0/1,M10.5.0", 2_374_102_800, 3_600),
        ("GMT0BST,M3.5.0/1,M10.5.0", 2_392_851_599, 3_600),
        ("GMT0BST,M3.5.0/1,M10.5.0", 2_392_851_600, 0),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 1_768_478_400, 39_600),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 1_775_318_399, 39_600),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 1_775_318_400, 36_000),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 1_784_116_800, 36_000),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 1_791_043_199, 36_000),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 1_791_043_200, 39_600),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 2_374_675_199, 39_600),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 2_374_675_200, 36_000),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 2_390_399_999, 36_000),
        ("AEST-10AEDT,M10.1.0,M4.1.0/3", 2_390_400_000, 39_600),
        ("IST-5:30", 1_768_478_400, 19_800),
        ("IST-5:30", 2_383_732_800, 19_800),
        ("<-03>3", 1_768_478_400, -10_800),
        ("<-03>3", 2_383_732_800, -10_800),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 1_768_478_400, 46_800),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 1_775_311_199, 46_800),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 1_775_311_200, 43_200),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 1_790_431_199, 43_200),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 1_790_431_200, 46_800),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 2_374_667_999, 46_800),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 2_374_668_000, 43_200),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 2_389_787_999, 43_200),
        ("NZST-12NZDT,M9.5.0,M4.1.0/3", 2_389_788_000, 46_800),
    ];

    #[test]
    fn the_rule_gives_the_offset_a_real_zone_gives_on_each_side_of_every_change() {
        for (footer, unix, want) in VECTORS {
            let rule = Rule::parse(footer).expect(footer);
            assert_eq!(rule.at(*unix).0, *want, "{footer} at {unix}");
        }
    }

    #[test]
    fn a_rule_is_read_in_each_of_its_forms() {
        let la = Rule::parse("PST8PDT,M3.2.0,M11.1.0").expect("rule");
        assert_eq!(la.std_east, -28_800);
        let dst = la.dst.expect("daylight time");
        assert_eq!(dst.east, -25_200, "an hour ahead when none is given");
        assert_eq!(dst.start_at, 7_200, "02:00 when no time is given");
        assert_eq!(
            dst.start,
            RuleDay::Month {
                month: 3,
                week: 2,
                weekday: 0
            }
        );
        // A half hour zone, and a zone with no daylight time and a number for a name.
        assert_eq!(Rule::parse("IST-5:30").expect("rule").std_east, 19_800);
        let sao = Rule::parse("<-03>3").expect("rule");
        assert_eq!((sao.std_east, sao.std_abbr.as_str()), (-10_800, "-03"));
        // Julian and zero-based days, a negative time, and a daylight offset that is not an hour.
        let odd = Rule::parse("AAA3BBB2:30,J60/-1,300/26").expect("rule");
        let dst = odd.dst.expect("daylight time");
        assert_eq!(dst.east, -9_000);
        assert_eq!((dst.start, dst.start_at), (RuleDay::Julian(60), -3_600));
        assert_eq!((dst.end, dst.end_at), (RuleDay::Zero(300), 93_600));
        // No rule after the daylight name means the one the C library uses.
        assert!(Rule::parse("EST5EDT").and_then(|rule| rule.dst).is_some());
        // Things that are not rules.
        for bad in [
            "",
            "PST",
            "8",
            "PST8PDT,M13.1.0,M11.1.0",
            "PST8PDT,M3.2.0",
            "AB5",
        ] {
            assert!(Rule::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn the_last_week_of_a_month_is_the_last_one_that_exists() {
        // The last Sunday of March 2026 is the 29th, and of March 2027 the 28th.
        let day = RuleDay::Month {
            month: 3,
            week: 5,
            weekday: 0,
        };
        let march_first_2026 = crate::dates::Date::new(2026, 3, 1).expect("day").serial();
        assert_eq!(Rule::serial(day, 2026), Some(march_first_2026 + 28));
        let march_first_2027 = crate::dates::Date::new(2027, 3, 1).expect("day").serial();
        assert_eq!(Rule::serial(day, 2027), Some(march_first_2027 + 27));
    }

    /// A version 2 file with no listed changes and one kind of clock, and this footer.
    fn file_with_footer(footer: &str) -> Vec<u8> {
        let mut data = Vec::new();
        for block in 0..2 {
            data.extend(b"TZif2");
            data.extend([0; 15]);
            for count in [0_i32, 0, 0, 0, 1, 4] {
                data.extend(count.to_be_bytes());
            }
            // One kind: offset zero, standard, abbreviation at the start of "UTC\0".
            data.extend(0_i32.to_be_bytes());
            data.extend([0, 0]);
            data.extend(b"UTC\0");
            if block == 1 {
                data.push(b'\n');
                data.extend(footer.as_bytes());
                data.push(b'\n');
            }
        }
        data
    }

    #[test]
    fn a_file_that_lists_nothing_uses_its_rule_for_all_time() {
        let zone = Zone::parse(&file_with_footer("PST8PDT,M3.2.0,M11.1.0")).expect("zone");
        // Summer, winter, and a summer long after any file would list.
        assert_eq!(zone.offset_at(1_784_116_800), -25_200);
        assert_eq!(zone.offset_at(1_768_478_400), -28_800);
        assert_eq!(zone.offset_at(2_383_732_800), -25_200);
        assert_eq!(zone.abbr_at(1_784_116_800), "PDT");
        assert_eq!(zone.abbr_at(1_768_478_400), "PST");
        assert!(zone.is_from(&file_with_footer("PST8PDT,M3.2.0,M11.1.0")));
        assert!(!zone.is_from(&file_with_footer("IST-5:30")));
    }

    #[test]
    fn a_file_with_no_rule_uses_what_it_lists_and_a_bad_file_is_refused() {
        let zone = Zone::parse(&file_with_footer("")).expect("zone");
        assert_eq!(zone.offset_at(1_784_116_800), 0);
        assert_eq!(zone.abbr_at(0), "UTC");
        assert!(Zone::parse(b"").is_none());
        assert!(Zone::parse(b"not a zone file at all, not even a little").is_none());
        let whole = file_with_footer("PST8PDT,M3.2.0,M11.1.0");
        for cut in [10, 43, 60, 100, whole.len() - 30] {
            assert!(
                Zone::parse(whole.get(..cut).unwrap_or(&[]))
                    .is_none_or(|zone| zone.offset_at(0) == 0),
                "cut at {cut}"
            );
        }
        assert_eq!(Zone::utc().offset_at(1_784_116_800), 0);
    }
}
