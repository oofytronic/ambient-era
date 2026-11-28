//! How habits look: on the Today page, and on a page of their own.
//!
//! What a habit is, and how it is going, is in `habits.rs`. This only draws it, in the tone it
//! is written in: a day missed is a quiet ring and not a mark against anyone, and the page says
//! how many of the last seven days were done and not how many were failed.

use iced::widget::{button, column, container, row, space, text, text_input};
use iced::{Color, Element, Fill, Theme};

use super::habits::{self, Day, Habit};
use super::{
    App, Message, Mini, field_style, ghost_label, ghost_style, hairline, ink_of, mini_button,
    mini_handle, serif, vscroll,
};

/// The look of a day's dot.
fn dot_style(state: Day) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let ink = ink_of(theme);
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let (fill, line, words) = match state {
            Day::Done => (Some(ink.accent), ink.accent, ink.on_accent),
            // A day that was missed is a ring, as quiet as one that has not come.
            Day::Missed => (None, ink.muted, ink.muted),
            Day::Open => (None, ink.accent, ink.accent),
            Day::Ahead => (None, ink.hairline, ink.muted),
            Day::Rest => (
                Some(Color {
                    a: 0.5,
                    ..ink.hairline
                }),
                Color::TRANSPARENT,
                ink.muted,
            ),
        };
        let fill = if hovered && fill.is_none() {
            Some(ink.surface)
        } else {
            fill
        };
        button::Style {
            background: fill.map(iced::Background::Color),
            text_color: words,
            border: iced::Border {
                radius: 100.0.into(),
                width: if state == Day::Open { 2.0 } else { 1.0 },
                color: line,
            },
            ..Default::default()
        }
    }
}

/// A round button for one day of one habit.
fn dot(state: Day, label: String, size: f32, message: Message) -> Element<'static, Message> {
    button(
        container(text(label).size(11))
            .width(Fill)
            .height(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(size)
    .height(size)
    .padding(0)
    .on_press(message)
    .style(dot_style(state))
    .into()
}

/// `5 of the last 7`, or `3 of 3 this week` for a habit of some days a week.
fn how_it_is_going(habit: &Habit, done: u32, asked: u32) -> String {
    if asked == 0 {
        return String::new();
    }
    match habit.schedule {
        habits::Schedule::PerWeek(_) => format!("{done} of {asked} this week"),
        habits::Schedule::Days(_) => format!("{done} of the last {asked} days"),
    }
}

impl App {
    fn counts_of(&self, habit: &Habit) -> habits::Counts {
        self.habit_log
            .get(&habit.key())
            .cloned()
            .unwrap_or_default()
    }

    /// The habits asked for today, with the ones not done first.
    pub(super) fn habits_due_today(&self) -> Vec<&Habit> {
        let today = self.clock.today;
        let mut due: Vec<&Habit> = self
            .habits
            .iter()
            .filter(|habit| {
                habits::due_today(habit, &self.counts_of(habit), today, self.week_monday)
            })
            .collect();
        due.sort_by_key(|habit| habits::done_on(habit, &self.counts_of(habit), today));
        due
    }

    /// How many habits are asked for today and not done.
    pub(super) fn habits_left(&self) -> usize {
        let today = self.clock.today;
        self.habits_due_today()
            .into_iter()
            .filter(|habit| !habits::done_on(habit, &self.counts_of(habit), today))
            .count()
    }

    /// The habits on the Today page: a round button for each, and how it is going.
    pub(super) fn today_habits(&self) -> Option<Element<'_, Message>> {
        let due = self.habits_due_today();
        if due.is_empty() {
            return None;
        }
        let ink = self.ink();
        let today = self.clock.today;
        let mut list = column![text("Habits").size(13).color(ink.muted)]
            .spacing(6)
            .width(Fill);
        for habit in due {
            let counts = self.counts_of(habit);
            let done = habits::done_on(habit, &counts, today);
            let now = habits::count_on(&counts, today);
            let state = if done { Day::Done } else { Day::Open };
            let label = if habit.target > 1 && !done && now > 0 {
                now.to_string()
            } else {
                String::new()
            };
            let (week_done, asked) = habits::last_week(habit, &counts, today, self.week_monday);
            let mut small = how_it_is_going(habit, week_done, asked);
            if habit.target > 1 {
                small = format!("{now} of {} today · {small}", habit.target);
            }
            let words = if done { ink.muted } else { ink.text };
            let face = column![
                text(habit.name.clone()).size(16).color(words),
                text(small).size(12).color(ink.muted),
            ]
            .spacing(2)
            .width(Fill);
            let check = dot(
                state,
                label,
                28.0,
                Message::HabitCheck {
                    key: habit.key(),
                    day: today.to_string(),
                },
            );
            list = list.push(
                column![
                    container(
                        row![check, face]
                            .spacing(12)
                            .align_y(iced::Alignment::Center)
                            .width(Fill),
                    )
                    .padding([10.0, 6.0]),
                    hairline(),
                ]
                .width(Fill),
            );
        }
        Some(list.into())
    }

    /// The page of habits: one to add, and each with its week.
    pub(super) fn habits_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut page = column![].spacing(12).width(Fill).height(Fill);
        if self.compact() {
            page = page.push(Self::detail_header("‹ Tasks", Vec::new()));
        }
        let field = text_input("Add a habit, like run 3 times a week", &self.habit_draft)
            .id(iced::widget::Id::new("habit-input"))
            .on_input(Message::HabitDraft)
            .on_submit(Message::HabitAdd)
            .padding(10)
            .width(Fill)
            .style(field_style);
        page = page.push(
            row![field, super::action_label("Add", Message::HabitAdd)]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        );
        page = page.push(self.habit_preview());
        let mut list = column![
            text("Habits").font(serif()).size(32),
            text("Check one off when you do it. A missed day does not undo the others.")
                .size(14)
                .color(ink.muted),
        ]
        .spacing(6)
        .width(Fill);
        if self.habits.is_empty() {
            list = list.push(
                text("Try “read every weekday”, “run 3 times a week”, “stretch mon wed fri”, or “water 8 times a day”.")
                    .size(14)
                    .color(ink.muted),
            );
        }
        for habit in &self.habits {
            list = list.push(self.habit_card(habit));
        }
        page.push(vscroll(list).height(Fill).width(Fill)).into()
    }

    /// Under the field: what the typed words will be added as.
    fn habit_preview(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let typed = self.habit_draft.trim();
        if typed.is_empty() {
            return container(text("").size(12)).into();
        }
        let (name, schedule, target) = habits::parse_phrase(typed);
        let mut said = format!("“{name}” · {}", schedule.label());
        if target > 1 {
            said.push_str(" · ");
            said.push_str(&target.to_string());
            said.push_str(" times a day");
        }
        container(text(said).size(12).color(ink.accent)).into()
    }

    /// One habit: its name and schedule, how it is going, and its last seven days to press.
    fn habit_card(&self, habit: &Habit) -> Element<'_, Message> {
        let ink = self.ink();
        let today = self.clock.today;
        let counts = self.counts_of(habit);
        let key = habit.key();
        let (done, asked) = habits::last_week(habit, &counts, today, self.week_monday);
        let (run, _) = habits::streaks(habit, &counts, today, self.week_monday);
        let mut going = how_it_is_going(habit, done, asked);
        if run >= 2 {
            if !going.is_empty() {
                going.push_str(" · ");
            }
            going.push_str(&habits::streak_label(habit, run));
        }
        let mut strip = row![].spacing(6).align_y(iced::Alignment::Center);
        for (day, state) in habits::recent_days(habit, &counts, today, 7, self.week_monday) {
            let letter = day.weekday_name().chars().take(1).collect::<String>();
            strip = strip.push(
                column![
                    text(letter).size(10).color(ink.muted),
                    dot(
                        state,
                        String::new(),
                        24.0,
                        Message::HabitCheck {
                            key: key.clone(),
                            day: day.to_string(),
                        },
                    ),
                ]
                .spacing(2)
                .align_x(iced::Alignment::Center),
            );
        }
        let head = row![
            button(
                column![
                    text(habit.name.clone()).size(16),
                    text(habit.schedule.label()).size(12).color(ink.muted),
                ]
                .spacing(2),
            )
            .padding(0)
            .on_press(Message::HabitOpen(key.clone()))
            .style(ghost_style),
            space().width(Fill),
            strip,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        let mut card = column![head].spacing(6).width(Fill);
        if !going.is_empty() {
            card = card.push(text(going).size(12).color(ink.muted));
        }
        if self.habit_open.as_deref() == Some(key.as_str()) {
            card = card.push(self.habit_month_view(habit, &counts));
        }
        column![container(card).padding([12.0, 6.0]), hairline()]
            .width(Fill)
            .into()
    }

    /// A month of a habit, a dot for each day to press, and what is kept of it.
    fn habit_month_view(&self, habit: &Habit, counts: &habits::Counts) -> Element<'_, Message> {
        let ink = self.ink();
        let today = self.clock.today;
        let (year, month) = self.habit_month;
        let name = super::dates::MONTHS
            .get(usize::try_from(month.saturating_sub(1)).unwrap_or(0))
            .copied()
            .unwrap_or("");
        let step = |glyph: &'static str, delta: i32| {
            button(text(glyph).size(18))
                .padding([2, 10])
                .on_press(Message::HabitMonth(delta))
                .style(ghost_style)
        };
        let letters = if self.week_monday {
            ["M", "T", "W", "T", "F", "S", "S"]
        } else {
            ["S", "M", "T", "W", "T", "F", "S"]
        };
        let mut grid = column![
            row![
                step("‹", -1),
                text(format!("{name} {year}")).size(14).width(Fill).center(),
                step("›", 1),
            ]
            .align_y(iced::Alignment::Center),
            row(letters.iter().map(|letter| {
                container(text(*letter).size(10).color(ink.muted))
                    .width(Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .into()
            })),
        ]
        .spacing(4)
        .width(Fill);
        for week in super::dates::month_grid(year, month, self.week_monday) {
            let cells = week.iter().map(|cell| match cell {
                Some(day) => {
                    let state = habits::day_state(habit, counts, *day, today, self.week_monday);
                    container(dot(
                        state,
                        day.day.to_string(),
                        28.0,
                        Message::HabitCheck {
                            key: habit.key(),
                            day: day.to_string(),
                        },
                    ))
                    .width(Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .into()
                }
                None => space().width(Fill).height(28).into(),
            });
            grid = grid.push(row(cells).spacing(2));
        }
        let (_, best) = habits::streaks(habit, counts, today, self.week_monday);
        let total = counts
            .values()
            .filter(|count| **count >= habit.target)
            .count();
        let kept = format!(
            "{total} {} in all{}",
            if total == 1 { "day" } else { "days" },
            if best >= 2 {
                format!(" · best run {}", habits::streak_label(habit, best))
            } else {
                String::new()
            },
        );
        grid = grid.push(text(kept).size(12).color(ink.muted));
        grid = grid.push(
            row![
                ghost_label("Edit in the note", Message::HabitEdit(habit.key())),
                mini_button(
                    mini_handle(Mini::Trash, self.is_dark()),
                    "Remove this habit",
                    Message::HabitRemove(habit.key()),
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
        container(grid).padding([6.0, 0.0]).width(Fill).into()
    }

    /// The word in the list on the left, with how many habits are left today.
    pub(super) fn habits_view_row(&self) -> Element<'_, Message> {
        self.task_view_row("Habits", self.habits_left(), 0, super::TaskView::Habits)
    }
}
