//! One editor for a note. A settled word is drawn, and the caret stays on the word
//! being typed.
//!
//! Opening `**` bolds the following text. The closing `**` ends it, and that word
//! leaves the marks immediately. An unclosed mark stays visible and continues onto
//! the next line until a blank line. The marks stay in the saved text. This is not
//! a second pane.
//!
//! The construct the caret is in is shown as typed, whole: the outermost run of marks around
//! it, or a whole link, image, or note link, so its marks can be read, changed, or taken out.
//! A caret moved to just after a construct touches it and shows it too. A caret that got there
//! by typing does not, so the closing mark settles at once.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use iced::advanced::Renderer as _;
use iced::advanced::text::Paragraph as _;
use iced::advanced::text::highlighter::{self, Highlighter};
use iced::widget::text_editor::{self, Action};
use iced::widget::{checkbox, column, container, row, rule, space, text};
use iced::{Background, Color, Element, Font, Length, Padding, Size, Theme};

use super::{Message, binding, editor_style, sans, serif};

pub(super) struct NoteBody {
    editor: text_editor::Content,
    /// The slice under the caret. The page editor shows only this.
    live: text_editor::Content,
    /// Absolute byte range of `live` inside the document.
    live_at: Option<(usize, usize)>,
    /// Last line we scrolled to. Arrowing to a new line reveals it.
    shown_line: usize,
    /// Vertical offset of that scroll. `NaN` until the next key asks again.
    shown_top: f32,
    /// Parsed once per text change. A drag must not parse the page again.
    cache: Vec<LineCache>,
    cached: String,
    /// The last thing done was typing or deleting. A caret just after a closing mark then stays
    /// outside it. A caret moved there with a key or the pointer is treated as touching it.
    typed: bool,
}

const OPEN_CAP: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    StrongStar,
    StrongUnder,
    EmStar,
    EmUnder,
    BothStar,
    BothUnder,
    Strike,
    Highlight,
    Code,
    Comment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct Carry {
    fence: bool,
    tilde: bool,
    math: bool,
    indent_code: bool,
    blank: bool,
    frontmatter: bool,
    opens: [Open; OPEN_CAP],
    len: u8,
}

impl Default for Carry {
    fn default() -> Self {
        Self {
            fence: false,
            tilde: false,
            math: false,
            indent_code: false,
            blank: false,
            frontmatter: false,
            opens: [Open::Code; OPEN_CAP],
            len: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Face {
    Text,
    Marker,
    Code,
    Link,
    Image,
    Heading(u8),
    Quote,
    Check,
    Checked,
    Wiki,
    Math,
    Meta,
    Callout(u8),
}

const STRONG: u8 = 1;
const EM: u8 = 2;
const STRIKE: u8 = 4;
const HIGH: u8 = 8;
const MISSING: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mark {
    face: Face,
    flags: u8,
}

fn text_mark() -> Mark {
    Mark {
        face: Face::Text,
        flags: 0,
    }
}

fn marker() -> Mark {
    Mark {
        face: Face::Marker,
        flags: 0,
    }
}

#[cfg(test)]
fn strong() -> Mark {
    Mark {
        face: Face::Text,
        flags: STRONG,
    }
}

#[cfg(test)]
fn em() -> Mark {
    Mark {
        face: Face::Text,
        flags: EM,
    }
}

fn strike_mark() -> Mark {
    Mark {
        face: Face::Text,
        flags: STRIKE,
    }
}

fn heading_mark(level: u8) -> Mark {
    Mark {
        face: Face::Heading(level),
        flags: 0,
    }
}

fn shown(mark: Mark) -> bool {
    mark.face != Face::Text || mark.flags != 0
}

struct Lead {
    skip: usize,
    mark: Mark,
    base: Mark,
    /// Start of a `[ ]` or `[x]` inside the prefix, and whether it is done.
    box_at: Option<(usize, bool)>,
}

struct Live {
    at: usize,
    carry: Vec<Carry>,
    spans: Vec<(Range<usize>, Mark)>,
}

struct SpanIter<'a> {
    spans: &'a [(Range<usize>, Mark)],
    index: usize,
}

impl Iterator for SpanIter<'_> {
    type Item = (Range<usize>, Mark);

    fn next(&mut self) -> Option<Self::Item> {
        let (range, mark) = self.spans.get(self.index)?;
        self.index = self.index.saturating_add(1);
        Some((range.clone(), *mark))
    }
}

impl Highlighter for Live {
    type Settings = ();
    type Highlight = Mark;
    type Iterator<'a> = SpanIter<'a>;

    fn new(_settings: &Self::Settings) -> Self {
        Self {
            at: 0,
            carry: Vec::new(),
            spans: Vec::new(),
        }
    }

    fn update(&mut self, _new_settings: &Self::Settings) {}

    fn change_line(&mut self, line: usize) {
        self.at = line;
        let keep = line.saturating_add(1);
        if self.carry.len() > keep {
            self.carry.truncate(keep);
        }
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if self.at == 0 {
            self.carry.clear();
            self.carry.push(Carry::default());
        }
        let incoming = self.carry.get(self.at).copied().unwrap_or_default();
        let at_start = self.at == 0;
        let incoming = if at_start {
            Carry {
                blank: true,
                ..incoming
            }
        } else {
            incoming
        };
        let (spans, outgoing) = paint_in(line, incoming, at_start);
        let next = self.at.saturating_add(1);
        if self.carry.len() == next {
            self.carry.push(outgoing);
        } else if let Some(slot) = self.carry.get_mut(next) {
            *slot = outgoing;
            self.carry.truncate(next.saturating_add(1));
        } else {
            while self.carry.len() < next {
                self.carry.push(Carry::default());
            }
            self.carry.push(outgoing);
        }
        self.spans = spans;
        self.at = next;
        SpanIter {
            spans: &self.spans,
            index: 0,
        }
    }

    fn current_line(&self) -> usize {
        self.at
    }
}

impl NoteBody {
    pub(super) fn new() -> Self {
        let mut body = Self {
            editor: text_editor::Content::new(),
            live: text_editor::Content::new(),
            live_at: None,
            shown_line: 0,
            shown_top: f32::NAN,
            cache: Vec::new(),
            cached: String::new(),
            typed: false,
        };
        body.sync_live();
        body
    }

    pub(super) fn text(&self) -> String {
        self.editor.text()
    }

    /// Selected document text, if the caret is holding a range.
    pub(super) fn selection(&self) -> Option<String> {
        self.editor.selection()
    }

    /// Document bytes of the selection, if it covers more than a caret.
    pub(super) fn selection_bounds(&self) -> Option<(usize, usize)> {
        let cursor = self.editor.cursor();
        let _ = cursor.selection?;
        let text = self.editor.text();
        let (from, to) = range_bytes(&text, &cursor);
        (from != to).then_some((from, to))
    }

    pub(super) fn caret_byte(&self) -> usize {
        pos_byte(&self.editor.text(), self.editor.cursor().position)
    }

    /// Selects the source between two bytes without rebuilding the page cache.
    pub(super) fn select_between(&mut self, anchor: usize, focus: usize) {
        let text = self.editor.text();
        let len = text.len();
        let anchor = anchor.min(len);
        let focus = focus.min(len);
        let position = byte_pos(&text, focus);
        let anchor_pos = byte_pos(&text, anchor);
        let selection = (anchor_pos != position).then_some(anchor_pos);
        self.editor.move_to(text_editor::Cursor {
            position,
            selection,
        });
        if selection.is_none() {
            // Moving the caret does not drop the anchor. Clear it or the
            // range follows the click.
            self.quench();
        }
        self.shown_line = position.line;
        self.shown_top = f32::NAN;
        if let Some((start, end)) = self.live_at {
            let mut local = local_cursor(&text, &self.editor.cursor(), start, end);
            // The page paints the highlight. A second range here does not match it.
            local.selection = None;
            self.live.move_to(local);
        }
    }

    /// Opens the word at the caret after a drag ends. The selection stays.
    pub(super) fn end_select(&mut self) {
        self.sync_live();
    }

    /// Replaces the text. The same text keeps the cursor where it is.
    pub(super) fn load(&mut self, source: &str) {
        if self.editor.text() == source {
            self.sync_live();
            return;
        }
        self.editor = text_editor::Content::with_text(source);
        self.shown_line = 0;
        self.shown_top = f32::NAN;
        self.typed = false;
        self.sync_live();
    }

    /// Puts the caret back after `load` replaced the document.
    pub(super) fn place(&mut self, cursor: text_editor::Cursor) {
        self.typed = false;
        self.shown_line = cursor.position.line;
        self.shown_top = f32::NAN;
        let clear = cursor.selection.is_none();
        self.editor.move_to(cursor);
        if clear {
            self.quench();
        }
        self.sync_live();
    }

    pub(super) fn perform(&mut self, action: &Action) {
        self.typed = matches!(action, Action::Edit(_));
        self.editor.perform(action.clone());
        self.sync_live();
    }

    pub(super) fn cursor(&self) -> text_editor::Cursor {
        self.editor.cursor()
    }

    /// Moves the caret to a byte in the document and opens that word.
    pub(super) fn place_byte(&mut self, byte: usize) {
        self.typed = false;
        let text = self.editor.text();
        let position = byte_pos(&text, byte.min(text.len()));
        self.editor.move_to(text_editor::Cursor {
            position,
            selection: None,
        });
        self.quench();
        self.shown_line = position.line;
        self.shown_top = f32::NAN;
        self.sync_live();
    }

    /// Moves the caret without opening that word. A drag keeps the page still.
    pub(super) fn place_byte_soft(&mut self, byte: usize) {
        let text = self.editor.text();
        let position = byte_pos(&text, byte.min(text.len()));
        self.editor.move_to(text_editor::Cursor {
            position,
            selection: None,
        });
        self.quench();
        self.shown_line = position.line;
        self.shown_top = f32::NAN;
    }

    /// Puts the caret at the start of a line. Used when opening a search hit or a task.
    pub(super) fn place_line(&mut self, line: usize) {
        let text = self.text();
        self.place_byte(line_start(&text, line));
    }

    /// `move_to` leaves a selection in place when the new cursor has none.
    /// A left motion is the only public way to drop that anchor, and parking
    /// both ends on the caret keeps it from stepping backward.
    fn quench(&mut self) {
        let cursor = self.editor.cursor();
        if cursor.selection.is_none() {
            return;
        }
        let position = cursor.position;
        self.editor.move_to(text_editor::Cursor {
            position,
            selection: Some(position),
        });
        self.editor
            .perform(text_editor::Action::Move(text_editor::Motion::Left));
    }

    /// The line whose word is open. Not the caret, while a drag is on another line.
    pub(super) fn open_line(&self) -> usize {
        let text = self.editor.text();
        let start = self.live_at.map_or(0, |(start, _)| start);
        byte_pos(&text, start.min(text.len())).line
    }

    /// The anchor of a selection, if the caret is holding a range.
    pub(super) fn anchor_byte(&self) -> Option<usize> {
        let cursor = self.editor.cursor();
        let anchor = cursor.selection?;
        Some(pos_byte(&self.editor.text(), anchor))
    }

    /// Scroll target for a find hit. Always the caret line, even if it just moved.
    pub(super) fn reveal(&mut self, measure: f32) -> f32 {
        self.shown_line = self.caret_line();
        let top = self.caret_top(measure);
        self.shown_top = top;
        top
    }

    /// `Some(y)` when the caret moved to another line, or wrapped down the one it is on.
    pub(super) fn follow(&mut self, measure: f32) -> Option<f32> {
        let line = self.caret_line();
        let top = self.caret_top(measure);
        if line != self.shown_line {
            self.shown_line = line;
            self.shown_top = top;
            return Some(top);
        }
        if self.shown_top.is_nan() {
            self.shown_top = top;
            return None;
        }
        if (top - self.shown_top).abs() > 1.0 {
            self.shown_top = top;
            return Some(top);
        }
        None
    }

    pub(super) fn edit(&mut self, action: &Action) -> bool {
        match action {
            Action::Click(_) | Action::Drag(_) => {
                self.typed = false;
                self.live.perform(action.clone());
                self.mirror_live_cursor();
                false
            }
            Action::Scroll { .. } => false,
            Action::Edit(_) => {
                self.typed = true;
                let previous = self.editor.text();
                self.editor.perform(action.clone());
                self.sync_live();
                previous != self.editor.text()
            }
            Action::Move(_)
            | Action::Select(_)
            | Action::SelectWord
            | Action::SelectLine
            | Action::SelectAll => {
                self.typed = false;
                self.editor.perform(action.clone());
                self.sync_live();
                false
            }
        }
    }

    fn caret_line(&self) -> usize {
        self.editor.cursor().position.line
    }

    fn caret_top(&self, measure: f32) -> f32 {
        self.point_at(self.caret_byte(), measure).0
    }

    /// Top of the selection, and the height of that visual row. `None` for a bare caret.
    pub(super) fn selection_origin(&self, measure: f32) -> Option<(f32, f32)> {
        let (from, _) = self.selection_bounds()?;
        Some(self.point_at(from, measure))
    }

    /// Y of the visual row that holds `byte`, and the height of that row.
    fn point_at(&self, byte: usize, measure: f32) -> (f32, f32) {
        let text = self.editor.text();
        let byte = byte.min(text.len());
        let target = byte_pos(&text, byte).line;
        let (win_start, win_end) = self.live_at.unwrap_or((0, 0));
        let active = byte_pos(&text, win_start.min(text.len())).line;
        let mut y = 0.0;
        let mut next_base = 0_usize;
        for (index, line) in split_lines(&text).into_iter().enumerate() {
            let base = next_base;
            next_base = base.saturating_add(line.len()).saturating_add(1);
            let local_win = if index == active {
                let start = win_start.saturating_sub(base).min(line.len());
                let end = win_end.saturating_sub(base).min(line.len()).max(start);
                start..end
            } else {
                0..0
            };
            let (font, size) = line_face(line);
            let width = content_measure(line, measure);
            let look = if index == active {
                let probe = if local_win.start < local_win.end {
                    local_win.end.saturating_sub(1)
                } else {
                    local_win.start.min(line.len())
                };
                let carry = self
                    .cache
                    .get(index)
                    .map(|row| row.carry)
                    .unwrap_or_default();
                look_at(line, probe, carry, index == 0, false)
            } else {
                Look {
                    size,
                    font,
                    color: None,
                    strike: false,
                    underline: false,
                    background: None,
                }
            };
            let ranges = wrap_ranges(
                line,
                width,
                font,
                size,
                local_win.clone(),
                look.font,
                look.size,
            );
            let rows = block_rows(line, &ranges, &local_win, width, look.font, look.size);
            if index == target {
                let local = byte.saturating_sub(base).min(line.len());
                let into = rows_before(
                    line, local, &ranges, &local_win, width, look.font, look.size,
                );
                let height = line_px(line);
                return (y + row_offset(into) * height, height);
            }
            y += row_offset(rows) * line_px(line) + LINE_GAP;
        }
        (y, line_px(""))
    }

    fn sync_live(&mut self) {
        self.park_task_caret();
        self.ensure_cache();
        let text = self.editor.text();
        let cursor = self.editor.cursor();
        let (start, end) = window_bytes(&text, &cursor, &self.cache, !self.typed);
        let slice = text.get(start..end).unwrap_or("");
        let mut local = local_cursor(&text, &cursor, start, end);
        local.selection = None;
        let same = self.live_at == Some((start, end)) && self.live.text() == slice;
        if !same {
            self.live = text_editor::Content::with_text(slice);
        }
        self.live.move_to(local);
        self.live_at = Some((start, end));
    }

    fn ensure_cache(&mut self) {
        let text = self.editor.text();
        if self.cached == text {
            return;
        }
        let old = std::mem::take(&mut self.cached);
        let rows = std::mem::take(&mut self.cache);
        self.cache = update_lines(&old, rows, &text);
        self.cached = text;
    }

    /// A task's marks are the checkbox, not text. Keep the caret in the words.
    fn park_task_caret(&mut self) {
        let text = self.editor.text();
        let cursor = self.editor.cursor();
        if cursor.selection.is_some() {
            return;
        }
        let ls = line_start(&text, cursor.position.line);
        let le = line_end(&text, cursor.position.line);
        let line = text.get(ls..le).unwrap_or("");
        let info = lead(line);
        if info.box_at.is_none() {
            return;
        }
        let skip = info.skip.min(line.len());
        if cursor.position.column >= skip {
            return;
        }
        self.editor.move_to(text_editor::Cursor {
            position: text_editor::Position {
                line: cursor.position.line,
                column: skip,
            },
            selection: None,
        });
    }

    fn mirror_live_cursor(&mut self) {
        let Some((start, _)) = self.live_at else {
            return;
        };
        let local = self.live.cursor();
        let text = self.editor.text();
        let position = abs_pos(&text, start, local.position);
        let selection = local.selection.map(|sel| abs_pos(&text, start, sel));
        self.editor.move_to(text_editor::Cursor {
            position,
            selection,
        });
    }
}

pub(super) fn view(
    body: &NoteBody,
    dark: bool,
    dragging: bool,
    measure: f32,
    spotlight: bool,
) -> Element<'_, Message> {
    let text = body.text();
    let (win_start, win_end) = body.live_at.unwrap_or((0, 0));
    let active = byte_pos(&text, win_start.min(text.len())).line;
    let lines = split_lines(&text);
    // The open word can stay on an earlier line while the caret has moved. The light follows
    // the caret's line, not that word, or it never leaves the first paragraph.
    let at = spotlight_at(&text, body.caret_byte());
    set_spotlight(spotlight.then_some((at, at)));
    let selected = body.selection_bounds();
    let paint = selection_color(dark);
    let placeholder = text.is_empty();
    let mut page = column![].spacing(LINE_GAP).width(Length::Fill);
    let mut next_base = 0_usize;
    for (index, line) in lines.into_iter().enumerate() {
        let incoming = body
            .cache
            .get(index)
            .map(|row| row.carry)
            .unwrap_or_default();
        let at_start = index == 0;
        // The start of each line, kept as a running sum. Asking for it by index scans the page
        // from the top every time.
        let base = next_base;
        next_base = base.saturating_add(line.len()).saturating_add(1);
        page = page.push(line_block(
            body,
            line,
            index,
            base,
            incoming,
            at_start,
            dark,
            placeholder && line.is_empty(),
            selected,
            paint,
            dragging,
            measure,
            index == active,
            win_start,
            win_end,
        ));
    }
    page = page.push(press_at(space().height(48).into(), text.len(), dragging));
    Release { child: page.into() }.into()
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::fn_params_excessive_bools
)]
fn line_block<'a>(
    body: &'a NoteBody,
    line: &str,
    index: usize,
    base: usize,
    carry: Carry,
    at_start: bool,
    dark: bool,
    placeholder: bool,
    selected: Option<(usize, usize)>,
    paint: Color,
    dragging: bool,
    measure: f32,
    active: bool,
    win_start: usize,
    win_end: usize,
) -> Element<'a, Message> {
    let cached = body.cache.get(index);
    if !active && (line.is_empty() || is_rule(line)) {
        return settled_line(
            line,
            index,
            base,
            carry,
            at_start,
            dark,
            selected,
            paint,
            dragging,
            cached,
            0..line.len(),
        );
    }
    let width = content_measure(line, measure);
    let (font, size) = line_face(line);
    let local_win = if active {
        let start = win_start.saturating_sub(base).min(line.len());
        let end = win_end.saturating_sub(base).min(line.len()).max(start);
        start..end
    } else {
        0..0
    };
    let probe = if local_win.start < local_win.end {
        local_win.end.saturating_sub(1)
    } else {
        local_win.start.min(line.len())
    };
    let look = look_at(line, probe, carry, at_start, dark);
    let ranges = wrap_ranges(
        line,
        width,
        font,
        size,
        local_win.clone(),
        look.font,
        look.size,
    );
    if ranges.len() <= 1 {
        return if active {
            active_line(
                body,
                line,
                index,
                base,
                local_win,
                0..line.len(),
                carry,
                at_start,
                dark,
                placeholder,
                selected,
                paint,
                dragging,
                width,
            )
        } else {
            settled_line(
                line,
                index,
                base,
                carry,
                at_start,
                dark,
                selected,
                paint,
                dragging,
                cached,
                0..line.len(),
            )
        };
    }
    let mut rows = column![].spacing(0).width(Length::Fill);
    for (row_index, range) in ranges.iter().enumerate() {
        let next = ranges
            .get(row_index.saturating_add(1))
            .map(|next| next.start);
        if active && row_has_editor(range, &local_win, next) {
            rows = rows.push(active_line(
                body,
                line,
                index,
                base,
                local_win.clone(),
                range.clone(),
                carry,
                at_start,
                dark,
                placeholder && range.start == 0,
                selected,
                paint,
                dragging,
                width,
            ));
        } else {
            rows = rows.push(settled_line(
                line,
                index,
                base,
                carry,
                at_start,
                dark,
                selected,
                paint,
                dragging,
                cached,
                range.clone(),
            ));
        }
    }
    rows.into()
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn mark_format(mark: &Mark, theme: &Theme) -> highlighter::Format<Font> {
    let dark = theme.palette().background.r < 0.5;
    let accent = if dark {
        Color::from_rgb8(0x66, 0xA5, 0xE1)
    } else {
        Color::from_rgb8(0x16, 0x44, 0x6F)
    };
    let marker = if dark {
        Color::from_rgb8(0x6F, 0x69, 0x64)
    } else {
        Color::from_rgb8(0xA8, 0xA2, 0x9A)
    };
    let struck = if dark {
        Color::from_rgb8(0xC4, 0x8A, 0x86)
    } else {
        Color::from_rgb8(0x8F, 0x45, 0x42)
    };
    let code = if dark {
        Color::from_rgb8(0xE0, 0xB3, 0x66)
    } else {
        Color::from_rgb8(0x8A, 0x5A, 0x12)
    };
    let done = if dark {
        Color::from_rgb8(0x7D, 0xC9, 0xA4)
    } else {
        Color::from_rgb8(0x1F, 0x7A, 0x4D)
    };
    let gold = if dark {
        Color::from_rgb8(0xE6, 0xC2, 0x5A)
    } else {
        Color::from_rgb8(0x8A, 0x62, 0x10)
    };
    if mark.face == Face::Code || mark.face == Face::Math {
        return highlighter::Format {
            color: Some(if mark.face == Face::Math {
                accent
            } else {
                code
            }),
            font: Some(Font::MONOSPACE),
        };
    }
    let mut color = match mark.face {
        Face::Marker | Face::Meta => Some(marker),
        Face::Link | Face::Image | Face::Check | Face::Quote => Some(accent),
        Face::Wiki => Some(if mark.flags & MISSING == 0 {
            accent
        } else {
            marker
        }),
        Face::Text | Face::Heading(_) => None,
        Face::Checked => Some(done),
        Face::Callout(kind) => Some(callout_color(kind, dark)),
        Face::Code | Face::Math => Some(code),
    };
    if mark.flags & HIGH != 0 {
        color = Some(gold);
    }
    if mark.flags & STRIKE != 0 {
        color = Some(struck);
    }
    let font = if mark.flags & STRONG != 0 && mark.flags & EM != 0 {
        Some(sans_bold_italic())
    } else if mark.flags & STRONG != 0 {
        Some(sans_bold())
    } else if mark.flags & EM != 0 {
        Some(sans_italic())
    } else {
        match mark.face {
            Face::Heading(level) if level <= 2 => Some(serif()),
            Face::Heading(_) | Face::Check | Face::Checked | Face::Image => Some(sans_bold()),
            _ => None,
        }
    };
    if color.is_none() && font.is_none() {
        return highlighter::Format {
            color: None,
            font: None,
        };
    }
    highlighter::Format { color, font }
}

fn callout_color(kind: u8, dark: bool) -> Color {
    let (dark_rgb, light_rgb): ([u8; 3], [u8; 3]) = match kind {
        1 | 2 => ([0x7D, 0xC9, 0xA4], [0x1F, 0x7A, 0x4D]),
        3 => ([0xE6, 0xC2, 0x5A], [0x8A, 0x62, 0x10]),
        4 => ([0xE0, 0x8A, 0x86], [0x8F, 0x45, 0x42]),
        5 => ([0xC4, 0xA6, 0xE0], [0x5C, 0x3D, 0x7A]),
        _ => ([0x66, 0xA5, 0xE1], [0x16, 0x44, 0x6F]),
    };
    let rgb = if dark { dark_rgb } else { light_rgb };
    Color::from_rgb8(rgb[0], rgb[1], rgb[2])
}

fn sans_bold() -> Font {
    Font {
        family: iced::font::Family::Name("DM Sans"),
        weight: iced::font::Weight::Bold,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    }
}

fn sans_italic() -> Font {
    Font {
        family: iced::font::Family::Name("DM Sans"),
        weight: iced::font::Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Italic,
    }
}

fn sans_bold_italic() -> Font {
    Font {
        family: iced::font::Family::Name("DM Sans"),
        weight: iced::font::Weight::Bold,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Italic,
    }
}

fn code_mark() -> Mark {
    Mark {
        face: Face::Code,
        flags: 0,
    }
}

fn check_mark(done: bool) -> Mark {
    Mark {
        face: if done { Face::Checked } else { Face::Check },
        flags: 0,
    }
}

#[cfg(test)]
fn paint(line: &str, carry: Carry) -> (Vec<(Range<usize>, Mark)>, Carry) {
    paint_in(line, carry, false)
}

#[allow(clippy::too_many_lines)]
fn paint_in(line: &str, carry: Carry, at_start: bool) -> (Vec<(Range<usize>, Mark)>, Carry) {
    if carry.frontmatter {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), meta_mark());
        if line.trim() == "---" {
            return (spans, Carry::default());
        }
        return (spans, carry);
    }
    if carry.fence {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), code_mark());
        if fence_closer(line, carry.tilde) {
            return (spans, Carry::default());
        }
        return (spans, carry);
    }
    if carry.math {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), math_mark());
        if line.trim() == "$$" {
            return (spans, Carry::default());
        }
        return (spans, carry);
    }
    if line.trim().is_empty() {
        return (
            Vec::new(),
            Carry {
                blank: true,
                ..Carry::default()
            },
        );
    }
    if let Some(tilde) = fence_open(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), code_mark());
        return (
            spans,
            Carry {
                fence: true,
                tilde,
                ..Carry::default()
            },
        );
    }
    if let Some(closed) = math_block(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), math_mark());
        if closed {
            return (spans, Carry::default());
        }
        return (
            spans,
            Carry {
                math: true,
                ..Carry::default()
            },
        );
    }
    if at_start && line.trim() == "---" {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), marker());
        return (
            spans,
            Carry {
                frontmatter: true,
                ..Carry::default()
            },
        );
    }
    if is_rule(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), marker());
        return (spans, Carry::default());
    }
    if indent_code_line(line, carry) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), code_mark());
        return (
            spans,
            Carry {
                indent_code: true,
                ..Carry::default()
            },
        );
    }
    if is_table_rule(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), marker());
        return (spans, Carry::default());
    }
    if is_table_line(line) {
        let (spans, next) = paint_table(line, carry);
        return (spans, finish_line(next));
    }

    let lead = lead(line);
    let mut spans = Vec::new();
    if let Some((box_at, done)) = lead.box_at {
        if box_at > 0 {
            push_span(&mut spans, 0, box_at, lead.mark);
        }
        let box_end = box_at.saturating_add(3).min(lead.skip);
        push_span(&mut spans, box_at, box_end, check_mark(done));
        if box_end < lead.skip {
            push_span(&mut spans, box_end, lead.skip, lead.mark);
        }
    } else if lead.skip > 0 {
        push_span(&mut spans, 0, lead.skip, lead.mark);
    }
    let mut from = lead.skip;
    if lead.mark.face == Face::Quote
        && let Some(rest) = line.get(from..)
        && let Some(end) = callout_end(rest)
    {
        let kind = callout_kind(rest.get(2..end.saturating_sub(1)).unwrap_or(""));
        let abs = from.saturating_add(end);
        push_span(&mut spans, from, abs, callout_mark(kind));
        from = abs;
    }
    let (inner, next) = paint_inline(line, from, line.len(), carry, lead.base);
    spans.extend(inner);
    (spans, finish_line(next))
}

fn finish_line(mut carry: Carry) -> Carry {
    if top(carry) == Some(Open::Code) {
        pop(&mut carry);
    }
    carry.blank = false;
    carry.indent_code = false;
    carry
}

fn paint_table(line: &str, mut carry: Carry) -> (Vec<(Range<usize>, Mark)>, Carry) {
    let mut spans = Vec::new();
    let mut start = 0_usize;
    for (index, byte) in line.bytes().enumerate() {
        if byte != b'|' {
            continue;
        }
        if start < index {
            let (inner, next) = paint_inline(line, start, index, carry, text_mark());
            spans.extend(inner);
            carry = next;
        }
        push_span(&mut spans, index, index.saturating_add(1), marker());
        start = index.saturating_add(1);
    }
    if start < line.len() {
        let (inner, next) = paint_inline(line, start, line.len(), carry, text_mark());
        spans.extend(inner);
        carry = next;
    }
    (spans, carry)
}

#[allow(clippy::too_many_lines)]
fn paint_inline(
    line: &str,
    mut index: usize,
    limit: usize,
    mut carry: Carry,
    base: Mark,
) -> (Vec<(Range<usize>, Mark)>, Carry) {
    let mut spans = Vec::new();
    while index < limit {
        if line.as_bytes().get(index) == Some(&b'\\') {
            push_span(&mut spans, index, index.saturating_add(1), marker());
            let escaped = char_len_at(line, index.saturating_add(1), limit);
            if escaped > 0 {
                let end = index.saturating_add(1).saturating_add(escaped);
                let face = face(carry, base);
                if shown(face) {
                    push_span(&mut spans, index.saturating_add(1), end, face);
                }
                index = end;
            } else {
                index = index.saturating_add(1);
            }
            continue;
        }
        if !in_literal(carry) {
            if let Some(end) = link_end(line, index, limit) {
                paint_link(&mut spans, line, index, end, face(carry, base));
                index = end;
                continue;
            }
            if let Some(end) = wiki_end(line, index, limit) {
                paint_wiki(&mut spans, line, index, end);
                index = end;
                continue;
            }
            if let Some(end) = auto_end(line, index, limit) {
                push_span(&mut spans, index, index.saturating_add(1), marker());
                push_span(
                    &mut spans,
                    index.saturating_add(1),
                    end.saturating_sub(1),
                    link_face(carry, base),
                );
                push_span(&mut spans, end.saturating_sub(1), end, marker());
                index = end;
                continue;
            }
            if let Some(end) = bare_url(line, index, limit) {
                push_span(&mut spans, index, end, link_face(carry, base));
                index = end;
                continue;
            }
            if let Some(end) = footnote_end(line, index, limit) {
                push_span(&mut spans, index, end, link_face(carry, base));
                index = end;
                continue;
            }
            if let Some(end) = math_end(line, index, limit) {
                push_span(&mut spans, index, index.saturating_add(1), marker());
                push_span(
                    &mut spans,
                    index.saturating_add(1),
                    end.saturating_sub(1),
                    math_mark(),
                );
                push_span(&mut spans, end.saturating_sub(1), end, marker());
                index = end;
                continue;
            }
        }
        if let Some((len, close)) = token(line, index, limit, carry) {
            let end = index.saturating_add(len);
            if close {
                pop(&mut carry);
            }
            push_span(&mut spans, index, end, marker());
            if !close {
                push_open(&mut carry, opened(line, index, limit));
            }
            index = end;
            continue;
        }
        let start = index;
        let face = face(carry, base);
        index = index.saturating_add(char_len_at(line, index, limit).max(1));
        while index < limit
            && line.as_bytes().get(index) != Some(&b'\\')
            && (in_literal(carry) || link_end(line, index, limit).is_none())
            && wiki_end(line, index, limit).is_none()
            && auto_end(line, index, limit).is_none()
            && bare_url(line, index, limit).is_none()
            && footnote_end(line, index, limit).is_none()
            && math_end(line, index, limit).is_none()
            && token(line, index, limit, carry).is_none()
        {
            index = index.saturating_add(char_len_at(line, index, limit).max(1));
        }
        if shown(face) {
            push_span(&mut spans, start, index, face);
        }
    }
    (spans, carry)
}

fn paint_link(
    spans: &mut Vec<(Range<usize>, Mark)>,
    line: &str,
    start: usize,
    end: usize,
    current: Mark,
) {
    let Some(rest) = line.get(start..end) else {
        return;
    };
    let bang = usize::from(rest.starts_with("!["));
    let label_at = start.saturating_add(1).saturating_add(bang);
    let Some(label_rel) = line.get(label_at..end).and_then(|slice| slice.find("](")) else {
        push_span(spans, start, end, link_from(current, bang == 1));
        return;
    };
    let label_end = label_at.saturating_add(label_rel);
    let url_at = label_end.saturating_add(2);
    push_span(spans, start, label_at, marker());
    push_span(spans, label_at, label_end, link_from(current, bang == 1));
    push_span(spans, label_end, url_at, marker());
    if url_at < end.saturating_sub(1) {
        push_span(spans, url_at, end.saturating_sub(1), marker());
    }
    push_span(spans, end.saturating_sub(1), end, marker());
}

fn paint_wiki(spans: &mut Vec<(Range<usize>, Mark)>, line: &str, start: usize, end: usize) {
    let bang = usize::from(
        line.get(start..end)
            .is_some_and(|rest| rest.starts_with('!')),
    );
    let open = start.saturating_add(bang);
    let Some(inner) = line.get(open.saturating_add(2)..end.saturating_sub(2)) else {
        return;
    };
    let label = inner.split_once('|').map_or(inner, |(_, label)| label);
    let label_at = if let Some((target, _)) = inner.split_once('|') {
        open.saturating_add(2)
            .saturating_add(target.len())
            .saturating_add(1)
    } else {
        open.saturating_add(2)
    };
    push_span(spans, start, label_at, marker());
    push_span(
        spans,
        label_at,
        label_at.saturating_add(label.len()),
        Mark {
            face: if bang == 1 { Face::Image } else { Face::Wiki },
            flags: if bang == 0 && wiki_missing(line, start, end) {
                MISSING
            } else {
                0
            },
        },
    );
    push_span(spans, end.saturating_sub(2), end, marker());
}

fn link_from(mut current: Mark, image: bool) -> Mark {
    current.face = if image { Face::Image } else { Face::Link };
    current
}

fn link_face(carry: Carry, base: Mark) -> Mark {
    link_from(face(carry, base), false)
}

fn link_end(line: &str, index: usize, limit: usize) -> Option<usize> {
    let rest = line.get(index..limit)?;
    let label_at = if rest.starts_with("![") {
        2
    } else if rest.starts_with('[') && !rest.starts_with("[^") {
        1
    } else {
        return None;
    };
    let label_rel = rest.get(label_at..)?.find("](")?;
    let url_at = index
        .saturating_add(label_at)
        .saturating_add(label_rel)
        .saturating_add(2);
    if url_at > limit {
        return None;
    }
    let url_rel = line.get(url_at..limit)?.find(')')?;
    Some(url_at.saturating_add(url_rel).saturating_add(1))
}

fn wiki_end(line: &str, index: usize, limit: usize) -> Option<usize> {
    let rest = line.get(index..limit)?;
    let (body, extra) = if rest.starts_with("![[") {
        (rest.get(1..)?, 1)
    } else if rest.starts_with("[[") {
        (rest, 0)
    } else {
        return None;
    };
    let close = body.find("]]")?;
    if close <= 2 {
        return None;
    }
    Some(
        index
            .saturating_add(extra)
            .saturating_add(close)
            .saturating_add(2),
    )
}

fn auto_end(line: &str, index: usize, limit: usize) -> Option<usize> {
    let rest = line.get(index..limit)?;
    if !rest.starts_with('<') {
        return None;
    }
    let close = rest.find('>')?;
    let body = rest.get(1..close)?;
    if body.is_empty() || body.contains([' ', '<', '\n']) {
        return None;
    }
    let url =
        body.starts_with("http://") || body.starts_with("https://") || body.starts_with("mailto:");
    let mail = body.contains('@') && body.contains('.');
    if !url && !mail {
        return None;
    }
    Some(index.saturating_add(close).saturating_add(1))
}

fn bare_url(line: &str, index: usize, limit: usize) -> Option<usize> {
    let rest = line.get(index..limit)?;
    let scheme = if rest.starts_with("https://") {
        8
    } else if rest.starts_with("http://") {
        7
    } else {
        return None;
    };
    if !edge_before(line, index) {
        return None;
    }
    let mut end = index.saturating_add(scheme);
    while end < limit {
        let char = line.get(end..)?.chars().next()?;
        if char.is_whitespace() || matches!(char, '<' | '>' | '"' | '\'') {
            break;
        }
        end = end.saturating_add(char.len_utf8());
    }
    while end > index.saturating_add(scheme) {
        let char = line.get(..end)?.chars().next_back()?;
        if matches!(char, '.' | ',' | ';' | ':' | '!' | '?' | ')') {
            end = end.saturating_sub(char.len_utf8());
        } else {
            break;
        }
    }
    (end > index.saturating_add(scheme)).then_some(end)
}

fn footnote_def(line: &str) -> Option<usize> {
    let spaces = leading_spaces(line);
    if spaces > 3 {
        return None;
    }
    let rest = line.get(spaces..)?;
    if !rest.starts_with("[^") {
        return None;
    }
    let close = rest.find("]:")?;
    if close <= 2 {
        return None;
    }
    Some(spaces.saturating_add(close).saturating_add(2))
}

fn footnote_end(line: &str, index: usize, limit: usize) -> Option<usize> {
    let rest = line.get(index..limit)?;
    if !rest.starts_with("[^") {
        return None;
    }
    let close = rest.find(']')?;
    let body = rest.get(2..close)?;
    if body.is_empty() || body.contains([' ', '\n', ']']) {
        return None;
    }
    Some(index.saturating_add(close).saturating_add(1))
}

fn token(line: &str, index: usize, limit: usize, carry: Carry) -> Option<(usize, bool)> {
    let rest = line.get(index..limit)?;
    if in_code(carry) {
        return rest.starts_with('`').then_some((1, true));
    }
    if in_comment(carry) {
        return rest.starts_with("%%").then_some((2, true));
    }
    if top(carry) == Some(Open::BothStar)
        && rest.starts_with("***")
        && can_close(line, index, 3, limit)
    {
        return Some((3, true));
    }
    if top(carry) == Some(Open::BothUnder)
        && rest.starts_with("___")
        && can_close(line, index, 3, limit)
    {
        return Some((3, true));
    }
    if top(carry) == Some(Open::StrongStar)
        && rest.starts_with("**")
        && can_close(line, index, 2, limit)
    {
        return Some((2, true));
    }
    if top(carry) == Some(Open::StrongUnder)
        && rest.starts_with("__")
        && can_close(line, index, 2, limit)
    {
        return Some((2, true));
    }
    if top(carry) == Some(Open::Strike)
        && rest.starts_with("~~")
        && can_close(line, index, 2, limit)
    {
        return Some((2, true));
    }
    if top(carry) == Some(Open::Highlight)
        && rest.starts_with("==")
        && highlight_closes(line, index, limit)
    {
        return Some((2, true));
    }
    if top(carry) == Some(Open::EmStar)
        && rest.starts_with('*')
        && !rest.starts_with("**")
        && can_close(line, index, 1, limit)
    {
        return Some((1, true));
    }
    if top(carry) == Some(Open::EmUnder)
        && rest.starts_with('_')
        && !rest.starts_with("__")
        && can_close(line, index, 1, limit)
    {
        return Some((1, true));
    }
    if rest.starts_with("%%") {
        return Some((2, false));
    }
    if !edge_before(line, index) {
        return None;
    }
    if (rest.starts_with("***") || rest.starts_with("___")) && after_mark(line, index, 3, limit) {
        return Some((3, false));
    }
    if (rest.starts_with("**")
        || rest.starts_with("__")
        || rest.starts_with("~~")
        || rest.starts_with("=="))
        && after_mark(line, index, 2, limit)
    {
        return Some((2, false));
    }
    if rest.starts_with('`') && after_mark(line, index, 1, limit) {
        return Some((1, false));
    }
    if rest.starts_with('*') && after_mark(line, index, 1, limit) {
        return Some((1, false));
    }
    if rest.starts_with('_') && after_mark(line, index, 1, limit) {
        return Some((1, false));
    }
    None
}

fn opened(line: &str, index: usize, limit: usize) -> Open {
    let rest = line.get(index..limit).unwrap_or("");
    if rest.starts_with("***") {
        Open::BothStar
    } else if rest.starts_with("___") {
        Open::BothUnder
    } else if rest.starts_with("**") {
        Open::StrongStar
    } else if rest.starts_with("__") {
        Open::StrongUnder
    } else if rest.starts_with("~~") {
        Open::Strike
    } else if rest.starts_with("==") {
        Open::Highlight
    } else if rest.starts_with("%%") {
        Open::Comment
    } else if rest.starts_with('`') {
        Open::Code
    } else if rest.starts_with('*') {
        Open::EmStar
    } else {
        Open::EmUnder
    }
}

fn face(carry: Carry, base: Mark) -> Mark {
    if in_code(carry) {
        return code_mark();
    }
    if in_comment(carry) {
        return meta_mark();
    }
    let mut flags = base.flags;
    for index in 0..usize::from(carry.len) {
        match carry.opens.get(index).copied() {
            Some(Open::StrongStar | Open::StrongUnder) => flags |= STRONG,
            Some(Open::EmStar | Open::EmUnder) => flags |= EM,
            Some(Open::BothStar | Open::BothUnder) => flags |= STRONG | EM,
            Some(Open::Strike) => flags |= STRIKE,
            Some(Open::Highlight) => flags |= HIGH,
            Some(Open::Code | Open::Comment) | None => {}
        }
    }
    Mark {
        face: base.face,
        flags,
    }
}

fn in_literal(carry: Carry) -> bool {
    in_code(carry) || in_comment(carry)
}

fn in_comment(carry: Carry) -> bool {
    top(carry) == Some(Open::Comment)
}

fn in_code(carry: Carry) -> bool {
    top(carry) == Some(Open::Code)
}

fn top(carry: Carry) -> Option<Open> {
    let index = usize::from(carry.len).checked_sub(1)?;
    carry.opens.get(index).copied()
}

fn push_open(carry: &mut Carry, open: Open) {
    let index = usize::from(carry.len);
    if let Some(slot) = carry.opens.get_mut(index) {
        *slot = open;
        carry.len = carry.len.saturating_add(1);
    }
}

fn pop(carry: &mut Carry) {
    carry.len = carry.len.saturating_sub(1);
}

fn push_span(spans: &mut Vec<(Range<usize>, Mark)>, start: usize, end: usize, mark: Mark) {
    if start >= end {
        return;
    }
    if let Some((range, previous)) = spans.last_mut()
        && *previous == mark
        && range.end == start
    {
        range.end = end;
        return;
    }
    spans.push((start..end, mark));
}

fn lead(line: &str) -> Lead {
    if let Some(end) = footnote_def(line) {
        return Lead {
            skip: end,
            mark: Mark {
                face: Face::Link,
                flags: 0,
            },
            base: text_mark(),
            box_at: None,
        };
    }
    if let Some((end, level)) = heading_end(line) {
        return Lead {
            skip: end,
            mark: marker(),
            base: heading_mark(level),
            box_at: None,
        };
    }
    if let Some(end) = quote_end(line) {
        if let Some(list) = list_after(line, end) {
            return Lead {
                skip: list.end,
                mark: Mark {
                    face: Face::Quote,
                    flags: 0,
                },
                base: if list.done {
                    strike_mark()
                } else {
                    text_mark()
                },
                box_at: list.box_at.map(|at| (at, list.done)),
            };
        }
        return Lead {
            skip: end,
            mark: Mark {
                face: Face::Quote,
                flags: 0,
            },
            base: text_mark(),
            box_at: None,
        };
    }
    if let Some(list) = list_parts(line).or_else(|| bare_task(line)) {
        return Lead {
            skip: list.end,
            mark: marker(),
            base: if list.done {
                strike_mark()
            } else {
                text_mark()
            },
            box_at: list.box_at.map(|at| (at, list.done)),
        };
    }
    Lead {
        skip: 0,
        mark: marker(),
        base: text_mark(),
        box_at: None,
    }
}

fn heading_end(line: &str) -> Option<(usize, u8)> {
    let spaces = leading_spaces(line);
    if spaces > 3 {
        return None;
    }
    let rest = line.get(spaces..)?;
    let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = rest.as_bytes().get(hashes).copied();
    if after.is_none_or(|byte| byte == b' ' || byte == b'\t') {
        let gap = usize::from(after == Some(b' ') || after == Some(b'\t'));
        let level = u8::try_from(hashes).unwrap_or(1);
        return Some((spaces.saturating_add(hashes).saturating_add(gap), level));
    }
    None
}

fn quote_end(line: &str) -> Option<usize> {
    let spaces = leading_spaces(line);
    if spaces > 3 {
        return None;
    }
    let bytes = line.as_bytes();
    let mut index = spaces;
    let mut quotes = 0_usize;
    while bytes.get(index) == Some(&b'>') {
        quotes = quotes.saturating_add(1);
        index = index.saturating_add(1);
        if bytes.get(index) == Some(&b' ') {
            index = index.saturating_add(1);
        }
    }
    (quotes > 0).then_some(index)
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.matches('|').count() >= 2
}

fn is_table_rule(line: &str) -> bool {
    is_table_line(line)
        && line
            .chars()
            .all(|char| matches!(char, '|' | '-' | ':' | ' ' | '\t'))
}

fn char_len_at(text: &str, index: usize, limit: usize) -> usize {
    text.get(index..limit)
        .and_then(|rest| rest.chars().next())
        .map_or(0, char::len_utf8)
}

fn edge_after_limited(text: &str, index: usize, limit: usize) -> bool {
    text.get(index..limit)
        .and_then(|rest| rest.chars().next())
        .is_none_or(|char| !char.is_ascii_alphanumeric())
}

struct ListParts {
    end: usize,
    box_at: Option<usize>,
    done: bool,
}

fn list_parts(line: &str) -> Option<ListParts> {
    let spaces = leading_indent(line);
    if spaces > MAX_INDENT {
        return None;
    }
    let rest = line.get(spaces..)?;
    let marker = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        2
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        let after = rest.get(digits..)?;
        if after.starts_with(". ") || after.starts_with(") ") {
            digits.saturating_add(2)
        } else {
            return None;
        }
    };
    let mut end = spaces.saturating_add(marker);
    let body = line.get(end..).unwrap_or("");
    let (box_at, done) = if body.len() >= 3
        && body.as_bytes().first() == Some(&b'[')
        && body.as_bytes().get(2) == Some(&b']')
    {
        let mark = body.as_bytes().get(1).copied().unwrap_or(b' ');
        (Some(end), mark == b'x' || mark == b'X')
    } else {
        (None, false)
    };
    if box_at.is_some() {
        end = end.saturating_add(3);
        if line.as_bytes().get(end) == Some(&b' ') {
            end = end.saturating_add(1);
        }
    }
    Some(ListParts { end, box_at, done })
}

/// A checkbox with no list marker, such as `[ ] call`.
fn bare_task(line: &str) -> Option<ListParts> {
    let spaces = leading_indent(line);
    if spaces > 24 {
        return None;
    }
    let rest = line.get(spaces..)?;
    let mark = rest.as_bytes().get(1).copied()?;
    if rest.len() < 3
        || rest.as_bytes().first() != Some(&b'[')
        || rest.as_bytes().get(2) != Some(&b']')
        || !matches!(mark, b' ' | b'x' | b'X')
    {
        return None;
    }
    let mut end = spaces.saturating_add(3);
    if line.as_bytes().get(end) == Some(&b' ') {
        end = end.saturating_add(1);
    }
    Some(ListParts {
        end,
        box_at: Some(spaces),
        done: mark == b'x' || mark == b'X',
    })
}

/// `list_parts` or a bare checkbox, with indexes shifted by `from`.
fn list_after(line: &str, from: usize) -> Option<ListParts> {
    let rest = line.get(from..)?;
    let mut parts = list_parts(rest).or_else(|| bare_task(rest))?;
    parts.end = parts.end.saturating_add(from);
    if let Some(at) = parts.box_at.as_mut() {
        *at = at.saturating_add(from);
    }
    Some(parts)
}

fn task_line_parts(line: &str) -> Option<ListParts> {
    let from = quote_end(line).unwrap_or(0);
    let parts = list_after(line, from)?;
    parts.box_at?;
    Some(parts)
}

fn is_rule(text: &str) -> bool {
    let trimmed = text.trim();
    let mut chars = trimmed.chars().filter(|char| !char.is_whitespace());
    let Some(first) = chars.next() else {
        return false;
    };
    if first != '-' && first != '*' && first != '_' {
        return false;
    }
    let mut count = 1_usize;
    for char in chars {
        if char != first {
            return false;
        }
        count = count.saturating_add(1);
    }
    count >= 3
        && trimmed
            .chars()
            .all(|char| char == first || char.is_whitespace())
}

fn fence_open(text: &str) -> Option<bool> {
    if leading_spaces(text) > 3 {
        return None;
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with("```") {
        Some(false)
    } else if trimmed.starts_with("~~~") {
        Some(true)
    } else {
        None
    }
}

fn fence_closer(text: &str, tilde: bool) -> bool {
    if leading_spaces(text) > 3 {
        return false;
    }
    let trimmed = text.trim();
    let want = if tilde { '~' } else { '`' };
    trimmed.len() >= 3 && trimmed.chars().all(|char| char == want)
}

fn math_block(line: &str) -> Option<bool> {
    let trimmed = line.trim();
    if trimmed == "$$" {
        return Some(false);
    }
    if trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() > 4 {
        return Some(true);
    }
    if trimmed.starts_with("$$") {
        return Some(false);
    }
    None
}

fn indent_code_line(line: &str, carry: Carry) -> bool {
    let spaces = leading_spaces(line);
    if spaces < 4 || line.trim().is_empty() {
        return false;
    }
    if carry.indent_code {
        return true;
    }
    carry.blank && list_parts(line).is_none() && heading_end(line).is_none()
}

fn meta_mark() -> Mark {
    Mark {
        face: Face::Meta,
        flags: 0,
    }
}

fn math_mark() -> Mark {
    Mark {
        face: Face::Math,
        flags: 0,
    }
}

fn callout_mark(kind: u8) -> Mark {
    Mark {
        face: Face::Callout(kind),
        flags: 0,
    }
}

fn can_close(line: &str, index: usize, len: usize, limit: usize) -> bool {
    let before = index == 0
        || line
            .get(..index)
            .and_then(|prefix| prefix.chars().next_back())
            .is_some_and(|char| !char.is_whitespace());
    before && edge_after_limited(line, index.saturating_add(len), limit)
}

/// A highlight closes at `==` even when the selection left a space inside it.
/// Otherwise the mark stays open and paints every letter after it.
fn highlight_closes(line: &str, index: usize, limit: usize) -> bool {
    edge_after_limited(line, index.saturating_add(2), limit)
}

fn after_mark(line: &str, index: usize, len: usize, limit: usize) -> bool {
    line.get(index.saturating_add(len)..limit)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|char| !char.is_whitespace())
}

fn math_end(line: &str, index: usize, limit: usize) -> Option<usize> {
    let rest = line.get(index..limit)?;
    if !rest.starts_with('$') || rest.starts_with("$$") || !edge_before(line, index) {
        return None;
    }
    let next = rest.chars().nth(1)?;
    if next.is_whitespace() {
        return None;
    }
    let close = rest.get(1..)?.find('$')?;
    if close == 0 {
        return None;
    }
    let body = rest.get(1..close.saturating_add(1))?;
    if body.ends_with(|char: char| char.is_whitespace()) {
        return None;
    }
    Some(index.saturating_add(close).saturating_add(2))
}

fn callout_end(rest: &str) -> Option<usize> {
    if !rest.starts_with("[!") {
        return None;
    }
    let close = rest.find(']')?;
    (close > 2).then_some(close.saturating_add(1))
}

fn callout_kind(body: &str) -> u8 {
    let name = body
        .trim()
        .trim_end_matches(['+', '-'])
        .trim()
        .to_ascii_lowercase();
    match name.as_str() {
        "tip" | "hint" | "important" | "success" | "check" | "done" => 1,
        "warning" | "caution" | "attention" => 3,
        "danger" | "error" | "failure" | "fail" | "missing" | "bug" => 4,
        "question" | "help" | "faq" => 5,
        "quote" | "cite" => 6,
        _ => 0,
    }
}

fn leading_spaces(text: &str) -> usize {
    text.bytes().take_while(|byte| *byte == b' ').count()
}

fn leading_indent(text: &str) -> usize {
    text.chars()
        .take_while(|char| *char == ' ' || *char == '\t')
        .map(char::len_utf8)
        .sum()
}

fn edge_before(text: &str, index: usize) -> bool {
    text.get(..index)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|char| !char.is_ascii_alphanumeric())
}

/// One checkbox line in a note. The bytes that sync are still the note.
pub(super) struct TaskLine {
    pub line: usize,
    pub done: bool,
    pub text: String,
}

/// Checkbox lines, in the order they appear. A normal list item is not a task.
pub(super) fn task_lines(body: &str) -> Vec<TaskLine> {
    let mut lines = Vec::new();
    for (line, text) in body.split('\n').enumerate() {
        let Some(list) = task_line_parts(text) else {
            continue;
        };
        let Some(at) = list.box_at else {
            continue;
        };
        let after = at.saturating_add(3);
        let label = text.get(after..).unwrap_or("").trim();
        lines.push(TaskLine {
            line,
            done: list.done,
            text: label.to_owned(),
        });
    }
    lines
}

/// Removes one checkbox line. `None` when that line is not a task.
pub(super) fn remove_task_line(body: &str, line_index: usize) -> Option<String> {
    let (start, end) = line_span(body, line_index)?;
    let line = body.get(start..end)?;
    let parts = list_parts(line)?;
    parts.box_at?;
    let drop_end = if body.as_bytes().get(end) == Some(&b'\n') {
        end.saturating_add(1)
    } else {
        end
    };
    let mut out = String::with_capacity(body.len());
    out.push_str(body.get(..start).unwrap_or(""));
    out.push_str(body.get(drop_end..).unwrap_or(""));
    Some(out)
}

/// Replaces the words of one checkbox line. The box and its done mark stay.
/// `None` when that line is not a task, the label is empty, or it already matches.
pub(super) fn rename_task_line(body: &str, line_index: usize, label: &str) -> Option<String> {
    let label = label.trim().replace(['\n', '\r'], " ");
    if label.is_empty() {
        return None;
    }
    let (start, end) = line_span(body, line_index)?;
    let line = body.get(start..end)?;
    let parts = list_parts(line)?;
    parts.box_at?;
    let prefix = task_prefix(line, parts.end);
    let new_line = format!("{prefix}{label}");
    if new_line == line {
        return None;
    }
    let mut out = String::with_capacity(body.len());
    out.push_str(body.get(..start).unwrap_or(""));
    out.push_str(&new_line);
    out.push_str(body.get(end..).unwrap_or(""));
    Some(out)
}

fn task_prefix(line: &str, end: usize) -> String {
    let prefix = line.get(..end).unwrap_or("");
    if prefix.ends_with(' ') {
        prefix.to_owned()
    } else {
        format!("{prefix} ")
    }
}

fn line_span(body: &str, line_index: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    for index in 0..=line_index {
        let rest = body.get(start..)?;
        let end = rest
            .find('\n')
            .map_or(body.len(), |at| start.saturating_add(at));
        if index == line_index {
            return Some((start, end));
        }
        if end == body.len() {
            return None;
        }
        start = end.saturating_add(1);
    }
    None
}

/// Swaps a checkbox line with the previous or next checkbox line in the same note.
/// `None` when that line is not a task, or there is no neighbor that way.
pub(super) fn move_task_line(body: &str, line_index: usize, up: bool) -> Option<String> {
    let tasks = task_lines(body);
    let pos = tasks.iter().position(|task| task.line == line_index)?;
    let other = if up {
        pos.checked_sub(1)?
    } else {
        let next = pos.saturating_add(1);
        if next >= tasks.len() {
            return None;
        }
        next
    };
    let here = tasks.get(pos)?;
    let there = tasks.get(other)?;
    let left = here.line.min(there.line);
    let right = here.line.max(there.line);
    let (start_left, end_left) = line_span(body, left)?;
    let (start_right, end_right) = line_span(body, right)?;
    let line_left = body.get(start_left..end_left)?;
    let line_right = body.get(start_right..end_right)?;
    let mut out = String::with_capacity(body.len());
    out.push_str(body.get(..start_left)?);
    out.push_str(line_right);
    out.push_str(body.get(end_left..start_right)?);
    out.push_str(line_left);
    out.push_str(body.get(end_right..).unwrap_or(""));
    (out != body).then_some(out)
}

/// The label without its due day, time, and repeat, or a note citation.
pub(super) fn task_words(label: &str) -> String {
    crate::due::Due::split(&without_cites(label)).0.to_owned()
}

/// The task label with `[[...]]` citations removed and the gaps collapsed.
pub(super) fn without_cites(label: &str) -> String {
    let mut out = String::new();
    let mut from = 0;
    for (start, end) in cite_spans(label) {
        if let Some(head) = label.get(from..start) {
            out.push_str(head);
        }
        out.push(' ');
        from = end;
    }
    if let Some(tail) = label.get(from..) {
        out.push_str(tail);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Note links in a task label, as `(target, title)`.
pub(super) fn task_cites(label: &str) -> Vec<(String, String)> {
    cite_spans(label)
        .into_iter()
        .filter_map(|(start, end)| {
            let inner = label.get(start.saturating_add(2)..end.saturating_sub(2))?;
            let (target, title) = inner
                .split_once('|')
                .map_or((inner, inner), |(target, title)| (target, title));
            let target = target.trim();
            if target.is_empty() {
                return None;
            }
            let title = title.trim();
            let title = if title.is_empty() { target } else { title };
            Some((target.to_owned(), title.to_owned()))
        })
        .collect()
}

/// The raw `[[...]]` marks, so a rename can put them back.
pub(super) fn cite_marks(label: &str) -> Vec<String> {
    cite_spans(label)
        .into_iter()
        .filter_map(|(start, end)| label.get(start..end).map(str::to_owned))
        .collect()
}

fn cite_spans(label: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while from < label.len() {
        let Some(rel) = label.get(from..).and_then(|rest| rest.find("[[")) else {
            break;
        };
        let start = from.saturating_add(rel);
        let Some(close) = label
            .get(start.saturating_add(2)..)
            .and_then(|rest| rest.find("]]"))
        else {
            break;
        };
        let end = start
            .saturating_add(2)
            .saturating_add(close)
            .saturating_add(2);
        spans.push((start, end));
        from = end;
    }
    spans
}

/// Byte starts of `query` in `text`. ASCII case is ignored. Empty when there is no query.
pub(super) fn find_bytes(text: &str, query: &str) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.as_bytes();
    let hay = text.as_bytes();
    let mut hits = Vec::new();
    let mut index: usize = 0;
    while index.saturating_add(needle.len()) <= hay.len() {
        let end = index.saturating_add(needle.len());
        if text.is_char_boundary(index)
            && hay
                .get(index..end)
                .is_some_and(|slice| slice.eq_ignore_ascii_case(needle))
        {
            hits.push(index);
            index = end.max(index.saturating_add(1));
        } else {
            index = index.saturating_add(1);
        }
    }
    hits
}

/// Where a Ctrl-click should go. A plain click still only moves the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NoteTarget {
    pub name: String,
    pub version: Option<String>,
}

/// Where a Ctrl-click should go. A plain click still only moves the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum OpenLink {
    Url(String),
    Note(NoteTarget),
}

pub(super) fn opened_at(text: &str, byte: usize) -> Option<OpenLink> {
    if byte > text.len() {
        return None;
    }
    let start = text
        .get(..byte)
        .and_then(|prefix| prefix.rfind('\n'))
        .map_or(0, |at| at.saturating_add(1));
    let end = text
        .get(byte..)
        .and_then(|rest| rest.find('\n'))
        .map_or(text.len(), |at| byte.saturating_add(at));
    let line = text.get(start..end)?;
    opened_in_line(line, byte.saturating_sub(start))
}

fn opened_in_line(line: &str, at: usize) -> Option<OpenLink> {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes.get(index..index.saturating_add(2)) == Some(b"[[")
            && let Some((end, name, version)) = wiki_parts(line, index)
        {
            if (index.saturating_add(2)..end.saturating_sub(2)).contains(&at) {
                return Some(OpenLink::Note(NoteTarget { name, version }));
            }
            index = end;
            continue;
        }
        if let Some((end, url, label_start, label_end)) = md_url(line, index) {
            if (label_start..label_end).contains(&at) || (label_end..end).contains(&at) {
                return Some(OpenLink::Url(url));
            }
            index = end;
            continue;
        }
        if let Some(end) = bare_url_end(line, index)
            && (index..end).contains(&at)
        {
            let raw = line.get(index..end).unwrap_or("");
            let trimmed = raw.trim_end_matches(['.', ',', ';', ')', ']']);
            if !trimmed.is_empty() {
                return Some(OpenLink::Url(trimmed.to_owned()));
            }
        }
        index = index.saturating_add(1);
    }
    None
}

fn wiki_name(line: &str, start: usize) -> Option<(usize, String)> {
    let (end, name, _) = wiki_parts(line, start)?;
    Some((end, name))
}

/// A note link target. `version` is set only for `id@version` with two note ids.
fn wiki_parts(line: &str, start: usize) -> Option<(usize, String, Option<String>)> {
    let rest = line.get(start..)?;
    if !rest.starts_with("[[") {
        return None;
    }
    let close = rest.find("]]")?;
    let end = start.saturating_add(close).saturating_add(2);
    let inner = rest.get(2..close)?;
    if inner.is_empty() {
        return None;
    }
    let name = inner.split_once('|').map_or(inner, |(target, _)| target);
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let (name, version) = split_version(name);
    Some((end, name, version))
}

/// `id@version` names one snapshot. Anything else stays the whole target.
pub(super) fn split_version(target: &str) -> (String, Option<String>) {
    let Some((name, version)) = target.split_once('@') else {
        return (target.to_owned(), None);
    };
    if is_note_id(name) && is_note_id(version) {
        (name.to_owned(), Some(version.to_owned()))
    } else {
        (target.to_owned(), None)
    }
}

/// Wikilink targets in reading order. A fence and an image embed are not chapters.
/// The same name twice is kept once, at its first place.
pub(super) fn chapter_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut fence: Option<bool> = None;
    for line in split_lines(text) {
        if let Some(tilde) = fence {
            if fence_closer(line, tilde) {
                fence = None;
            }
            continue;
        }
        if let Some(kind) = fence_open(line) {
            fence = Some(kind);
            continue;
        }
        let mut index = 0;
        while index < line.len() {
            if !line.is_char_boundary(index) {
                index = index.saturating_add(1);
                continue;
            }
            if line
                .get(index..)
                .is_some_and(|rest| rest.starts_with("![["))
                && let Some((end, _)) = wiki_name(line, index.saturating_add(1))
            {
                index = end;
                continue;
            }
            if let Some((end, name)) = wiki_name(line, index) {
                if !names
                    .iter()
                    .any(|have: &String| have.eq_ignore_ascii_case(&name))
                {
                    names.push(name);
                }
                index = end;
                continue;
            }
            index = index.saturating_add(1);
        }
    }
    names
}

/// The same page, without a caret. Clicks do not move the open note.
pub(super) fn reading(text: &str, dark: bool) -> Element<'static, Message> {
    let cache = cache_lines(text);
    let mut page = column![].spacing(2).width(Length::Fill);
    for (index, line) in split_lines(text).into_iter().enumerate() {
        page = page.push(quiet_line(line, cache.get(index), index == 0, dark));
    }
    page.into()
}

fn quiet_line(
    line: &str,
    cached: Option<&LineCache>,
    at_start: bool,
    dark: bool,
) -> Element<'static, Message> {
    if line.is_empty() {
        return space().width(Length::Fill).height(line_px(line)).into();
    }
    if is_rule(line) {
        return container(rule::horizontal(1))
            .width(Length::Fill)
            .padding(Padding {
                top: 10.0,
                right: 0.0,
                bottom: 10.0,
                left: 0.0,
            })
            .into();
    }
    let carry = cached.map_or_else(Carry::default, |row| row.carry);
    let shown = line_pieces(cached, line, 0, 0..line.len(), carry, at_start, dark);
    let size = shown
        .iter()
        .fold(BODY, |size, piece| size.max(piece.look.size));
    let drawn = rich_of(&shown, true, false, false);
    let body = if quote_end(line).is_some() {
        quote_row(drawn, size, dark)
    } else {
        drawn
    };
    let Some((_, done)) = lead(line).box_at else {
        return body;
    };
    let mark = if done { "☑" } else { "☐" };
    row![text(mark).size(size), body]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .into()
}

fn md_url(line: &str, start: usize) -> Option<(usize, String, usize, usize)> {
    let rest = line.get(start..)?;
    let label_at = if rest.starts_with("![") {
        2
    } else if rest.starts_with('[') && !rest.starts_with("[^") && !rest.starts_with("[[") {
        1
    } else {
        return None;
    };
    let label_rel = rest.get(label_at..)?.find("](")?;
    let label_start = start.saturating_add(label_at);
    let label_end = label_start.saturating_add(label_rel);
    let url_at = label_end.saturating_add(2);
    let url_rel = line.get(url_at..)?.find(')')?;
    let end = url_at.saturating_add(url_rel).saturating_add(1);
    let url = line.get(url_at..url_at.saturating_add(url_rel))?.trim();
    if url.is_empty() {
        return None;
    }
    Some((end, url.to_owned(), label_start, label_end))
}

fn bare_url_end(line: &str, start: usize) -> Option<usize> {
    let rest = line.get(start..)?;
    let prefix = if rest.starts_with("https://") {
        8
    } else if rest.starts_with("http://") {
        7
    } else {
        return None;
    };
    if start > 0 {
        let prev = line.get(..start)?.chars().next_back()?;
        if prev.is_ascii_alphanumeric() {
            return None;
        }
    }
    let bytes = line.as_bytes();
    let mut end = start.saturating_add(prefix);
    while end < bytes.len() {
        let byte = bytes.get(end).copied()?;
        if byte.is_ascii_whitespace() || byte == b')' || byte == b']' {
            break;
        }
        end = end.saturating_add(1);
    }
    Some(end)
}

/// Writes `[x]` or `[ ]` on one line. `None` when that line is not a task, or it already matches.
pub(super) fn set_task_line(body: &str, line_index: usize, done: bool) -> Option<String> {
    let mut start = 0;
    for index in 0..=line_index {
        let end = body
            .get(start..)
            .and_then(|rest| rest.find('\n'))
            .map_or(body.len(), |at| start.saturating_add(at));
        if index == line_index {
            let line = body.get(start..end)?;
            let list = task_line_parts(line)?;
            let at = list.box_at?;
            if list.done == done {
                return None;
            }
            let mark = if done { "[x]" } else { "[ ]" };
            let mut out = String::with_capacity(body.len());
            out.push_str(body.get(..start.saturating_add(at))?);
            out.push_str(mark);
            out.push_str(body.get(start.saturating_add(at).saturating_add(3)..)?);
            return Some(out);
        }
        if end == body.len() {
            return None;
        }
        start = end.saturating_add(1);
    }
    None
}

/// A toolbar action. A word mark wraps the selection or the word under the caret.
/// A line mark toggles the lines the caret touches. An insert drops a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FormatKind {
    Bold,
    Italic,
    Strike,
    Highlight,
    Code,
    Heading,
    List,
    Task,
    Quote,
    Link,
    Wiki,
    Image,
    Table,
    Rule,
    Callout,
    CodeBlock,
}

pub(super) struct Change {
    pub anchor: text_editor::Position,
    pub end: text_editor::Position,
    pub insert: String,
    pub after: text_editor::Cursor,
}

pub(super) fn format_change(text: &str, cursor: text_editor::Cursor, kind: FormatKind) -> Change {
    match kind {
        FormatKind::Bold => wrap(text, &cursor, "**", "**"),
        FormatKind::Italic => wrap(text, &cursor, "*", "*"),
        FormatKind::Strike => wrap(text, &cursor, "~~", "~~"),
        FormatKind::Highlight => wrap(text, &cursor, "==", "=="),
        FormatKind::Code => wrap(text, &cursor, "`", "`"),
        FormatKind::Heading => map_lines(text, &cursor, cycle_heading),
        FormatKind::List => map_lines(text, &cursor, toggle_list),
        FormatKind::Task => map_lines(text, &cursor, toggle_task),
        FormatKind::Quote => map_lines(text, &cursor, toggle_quote),
        FormatKind::Link => link_insert(text, &cursor),
        FormatKind::Wiki => wiki_insert(text, &cursor),
        FormatKind::Image => image_insert(text, &cursor),
        FormatKind::Table => block_at_line(text, &cursor, "|  |  |\n| --- | --- |\n|  |  |\n", 2),
        FormatKind::Rule => rule_insert(text, &cursor),
        FormatKind::Callout => callout_insert(text, &cursor),
        FormatKind::CodeBlock => code_block(text, &cursor),
    }
}

/// Enter in a list, a task, or a quote starts the next line with the same marker. On an empty
/// item it ends the list instead, or steps out one level if the item is nested, so pressing
/// Enter twice finishes a list. A fence, math block, or frontmatter is left alone.
pub(super) fn continue_enter(text: &str, cursor: text_editor::Cursor) -> Option<Change> {
    let Some((start, line, parts)) = list_at_cursor(text, &cursor) else {
        return quote_enter(text, &cursor);
    };
    if line.get(parts.end..).unwrap_or("").trim().is_empty() {
        return Some(leave_item(text, start, line));
    }
    let column = cursor_column(line, cursor.position.column);
    let at = start.saturating_add(column.max(parts.end));
    let mut insert = String::from("\n");
    insert.push_str(&next_prefix(line, &parts));
    Some(rewrite(text, at, at, &insert, insert.len(), None))
}

/// An empty item: out one level if it is nested, otherwise the marker goes.
fn leave_item(text: &str, start: usize, line: &str) -> Change {
    let end = start.saturating_add(line.len());
    let indent = leading_indent(line);
    if indent > 0 {
        let step = if line.starts_with('\t') {
            1
        } else {
            indent.min(ITEM_INDENT)
        };
        let outer = line.get(step..).unwrap_or("");
        return rewrite(text, start, end, outer, outer.len(), None);
    }
    rewrite(text, start, end, "", 0, None)
}

/// The width of one level of nesting.
const ITEM_INDENT: usize = 2;

/// The marker of a quote line: any indent, one or more `>`, and the space after them.
fn quote_prefix(line: &str) -> Option<usize> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let mut end = indent;
    let mut marks = 0_usize;
    loop {
        match line.as_bytes().get(end) {
            Some(b'>') => {
                marks = marks.saturating_add(1);
                end = end.saturating_add(1);
            }
            Some(b' ')
                if marks > 0 && line.as_bytes().get(end.saturating_add(1)) == Some(&b'>') =>
            {
                end = end.saturating_add(1);
            }
            _ => break,
        }
    }
    if marks == 0 {
        return None;
    }
    if line.as_bytes().get(end) == Some(&b' ') {
        end = end.saturating_add(1);
    }
    Some(end)
}

fn quote_enter(text: &str, cursor: &text_editor::Cursor) -> Option<Change> {
    if cursor.selection.is_some() || skipped_block(text, cursor.position.line) {
        return None;
    }
    let start = line_start(text, cursor.position.line);
    let end = line_end(text, cursor.position.line);
    let line = text.get(start..end)?;
    let prefix = quote_prefix(line)?;
    let column = cursor_column(line, cursor.position.column);
    if column < prefix.min(line.len()) {
        return None;
    }
    if line.get(prefix..).unwrap_or("").trim().is_empty() {
        return Some(rewrite(text, start, end, "", 0, None));
    }
    let at = start.saturating_add(column);
    let mut insert = String::from("\n");
    insert.push_str(line.get(..prefix).unwrap_or("> "));
    Some(rewrite(text, at, at, &insert, insert.len(), None))
}

/// One line of a block that `shift_items` rewrote: where it began before and after, how long it
/// was, and how many bytes were added to its start (`grew`) or taken from it.
struct Row {
    old_start: usize,
    old_len: usize,
    new_start: usize,
    step: usize,
    grew: bool,
}

/// Tab and Shift+Tab: nest the list items the caret is in, or the ones a selection covers, one
/// level, or bring them out one. `None` when none of those lines is a list item, so the key does
/// nothing in a paragraph.
pub(super) fn shift_items(text: &str, cursor: &text_editor::Cursor, back: bool) -> Option<Change> {
    let (from, to) = range_bytes(text, cursor);
    let first = byte_pos(text, from).line;
    let mut last = byte_pos(text, to).line;
    if to > from && last > first && to == line_start(text, last) {
        // A selection that ends at the start of a line does not include that line.
        last = last.saturating_sub(1);
    }
    if (first..=last).any(|line_no| skipped_block(text, line_no)) {
        return None;
    }
    let block_start = line_start(text, first);
    let block_end = line_end(text, last);
    let block = text.get(block_start..block_end)?;
    let mut out = String::with_capacity(block.len().saturating_add(8));
    let mut rows: Vec<Row> = Vec::new();
    let mut old_start = 0_usize;
    for (index, line) in block.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let (shifted, step, grew) = shift_line(line, back);
        rows.push(Row {
            old_start,
            old_len: line.len(),
            new_start: out.len(),
            step,
            grew,
        });
        out.push_str(&shifted);
        old_start = old_start.saturating_add(line.len()).saturating_add(1);
    }
    if rows.iter().all(|row| row.step == 0) {
        return None;
    }
    let map = |byte: usize| -> usize {
        let local = byte.saturating_sub(block_start);
        for row in &rows {
            if local <= row.old_start.saturating_add(row.old_len) {
                let col = local.saturating_sub(row.old_start);
                let col = if row.grew {
                    col.saturating_add(row.step)
                } else {
                    col.saturating_sub(row.step)
                };
                return row.new_start.saturating_add(col);
            }
        }
        out.len()
    };
    let at = map(pos_byte(text, cursor.position));
    let select = cursor
        .selection
        .map(|anchor| (map(pos_byte(text, anchor)), at));
    Some(rewrite(text, block_start, block_end, &out, at, select))
}

/// One line moved a level. A line that is not a list item, or that cannot move further, comes
/// back as it was with a step of 0.
fn shift_line(line: &str, back: bool) -> (String, usize, bool) {
    if list_parts(line).is_none() {
        return (line.to_owned(), 0, false);
    }
    if back {
        let step = if line.starts_with('\t') {
            1
        } else {
            leading_spaces(line).min(ITEM_INDENT)
        };
        return (line.get(step..).unwrap_or(line).to_owned(), step, false);
    }
    if leading_indent(line).saturating_add(ITEM_INDENT) > MAX_INDENT {
        return (line.to_owned(), 0, false);
    }
    (
        format!("{}{line}", " ".repeat(ITEM_INDENT)),
        ITEM_INDENT,
        true,
    )
}

/// `list_parts` does not read an item indented further than this.
const MAX_INDENT: usize = 24;

/// Backspace on an empty list or task removes that marker.
pub(super) fn exit_empty_item(text: &str, cursor: text_editor::Cursor) -> Option<Change> {
    let (start, line, parts) = list_at_cursor(text, &cursor)?;
    let end = line_end(text, cursor.position.line);
    let body = line.get(parts.end..).unwrap_or("");
    let column = cursor_column(line, cursor.position.column);
    if !body.trim().is_empty() || column < parts.end {
        return None;
    }
    Some(rewrite(text, start, end, "", 0, None))
}

fn list_at_cursor<'a>(
    text: &'a str,
    cursor: &text_editor::Cursor,
) -> Option<(usize, &'a str, ListParts)> {
    if cursor.selection.is_some() {
        return None;
    }
    let line_no = cursor.position.line;
    if skipped_block(text, line_no) {
        return None;
    }
    let start = line_start(text, line_no);
    let end = line_end(text, line_no);
    let line = text.get(start..end).unwrap_or("");
    if is_rule(line) {
        return None;
    }
    let parts = list_parts(line)?;
    Some((start, line, parts))
}

fn cursor_column(line: &str, column: usize) -> usize {
    floor_boundary(line, column.min(line.len()))
}

fn next_prefix(line: &str, parts: &ListParts) -> String {
    let spaces = leading_spaces(line);
    let indent = line.get(..spaces).unwrap_or("");
    let rest = line.get(spaces..).unwrap_or("");
    let bullet = rest.chars().next().unwrap_or('-');
    if bullet.is_ascii_digit() {
        let number = following_number(rest);
        let delim = list_delim(rest);
        if parts.box_at.is_some() {
            return format!("{indent}{number}{delim} [ ] ");
        }
        return format!("{indent}{number}{delim} ");
    }
    if parts.box_at.is_some() {
        return format!("{indent}{bullet} [ ] ");
    }
    format!("{indent}{bullet} ")
}

fn list_delim(rest: &str) -> char {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if rest.as_bytes().get(digits) == Some(&b')') {
        ')'
    } else {
        '.'
    }
}

fn following_number(rest: &str) -> u32 {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    rest.get(..digits)
        .and_then(|text| text.parse::<u32>().ok())
        .unwrap_or(1)
        .saturating_add(1)
}

fn skipped_block(text: &str, line_no: usize) -> bool {
    let mut fence = false;
    let mut tilde = false;
    let mut math = false;
    let mut front = false;
    for (index, row) in text.split('\n').enumerate() {
        if index == line_no {
            return fence || math || front;
        }
        if front {
            if row.trim() == "---" {
                front = false;
            }
            continue;
        }
        if fence {
            if fence_closer(row, tilde) {
                fence = false;
            }
            continue;
        }
        if math {
            if row.trim() == "$$" {
                math = false;
            }
            continue;
        }
        if index == 0 && row.trim() == "---" {
            front = true;
            continue;
        }
        if let Some(is_tilde) = fence_open(row) {
            fence = true;
            tilde = is_tilde;
            continue;
        }
        if math_block(row) == Some(false) {
            math = true;
        }
    }
    false
}

fn wrap(text: &str, cursor: &text_editor::Cursor, left: &str, right: &str) -> Change {
    let (mut from, mut to) = range_bytes(text, cursor);
    if from == to {
        let (word_from, word_to) = word_bounds(text, from);
        if word_from != word_to {
            from = word_from;
            to = word_to;
        }
    } else {
        (from, to) = trim_edges(text, from, to);
    }
    (from, to) = expand_pairs(text, from, to, left);
    (from, to) = trim_edges(text, from, to);
    let inner = text.get(from..to).unwrap_or("");
    if let Some(stripped) = peel(inner, left, right) {
        return rewrite(text, from, to, stripped, stripped.len(), None);
    }
    if let Some(change) = unwrap_emphasis(text, from, to, left) {
        return change;
    }
    if surrounded(text, from, to, left, right) {
        let start = from.saturating_sub(left.len());
        let end = to.saturating_add(right.len());
        return rewrite(text, start, end, inner, inner.len(), None);
    }
    let plain = strip_delim(inner, left);
    let mut insert = String::with_capacity(left.len() + plain.len() + right.len());
    insert.push_str(left);
    insert.push_str(&plain);
    insert.push_str(right);
    let cursor_at = if plain.is_empty() {
        left.len()
    } else {
        insert.len()
    };
    rewrite(text, from, to, &insert, cursor_at, None)
}

/// Emphasis marks of `mark` that sit right outside `from..to` on both sides. Three of them are
/// bold and italic together.
fn emphasis_run(text: &str, from: usize, to: usize, mark: char) -> usize {
    let before = text.get(..from).map_or(0, |prefix| {
        prefix.chars().rev().take_while(|ch| *ch == mark).count()
    });
    let after = text.get(to..).map_or(0, |suffix| {
        suffix.chars().take_while(|ch| *ch == mark).count()
    });
    before.min(after)
}

/// Bold or italic that is already on, written with `*` or with `_`, taken off again.
///
/// The old check only knew an exact `**word**` or `*word*`. Any extra mark beside it, as in
/// `***both***`, or the underscore spelling, made it add a second layer instead of removing the
/// first. This counts the marks around the text and removes the ones that were asked for.
fn unwrap_emphasis(text: &str, from: usize, to: usize, left: &str) -> Option<Change> {
    let want = match left {
        "**" => 2,
        "*" => 1,
        _ => return None,
    };
    // Two marks are bold, one is italic, and three are both.
    let is_on = |run: usize| {
        if want == 2 {
            run >= 2
        } else {
            run == 1 || run >= 3
        }
    };
    let inner = text.get(from..to)?;
    for mark in ['*', '_'] {
        if is_on(emphasis_run(text, from, to, mark)) {
            let start = from.checked_sub(want)?;
            let end = to.checked_add(want)?;
            return Some(rewrite(text, start, end, inner, inner.len(), None));
        }
        // A word counts `_` as a letter, so the marks of `_em_` are inside the word the caret
        // is in. Count the ones at both ends of the text itself.
        if mark == '_' {
            let lead = inner.chars().take_while(|ch| *ch == '_').count();
            let trail = inner.chars().rev().take_while(|ch| *ch == '_').count();
            let run = lead.min(trail);
            if is_on(run) && inner.len() > want.saturating_mul(2) {
                let stripped = inner.get(want..inner.len().checked_sub(want)?)?;
                return Some(rewrite(text, from, to, stripped, stripped.len(), None));
            }
        }
    }
    None
}

fn trim_edges(text: &str, mut from: usize, mut to: usize) -> (usize, usize) {
    let Some(inner) = text.get(from..to) else {
        return (from, to);
    };
    let lead = inner
        .chars()
        .take_while(|char| char.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let trail = inner
        .chars()
        .rev()
        .take_while(|char| char.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    if lead.saturating_add(trail) >= inner.len() {
        return (from, to);
    }
    from = from.saturating_add(lead);
    to = to.saturating_sub(trail);
    (from, to)
}

/// A mark pair that overlaps the range is part of the selection. Wrapping then
/// sees the whole pair, instead of a closer stranded in the middle of the words.
fn expand_pairs(text: &str, mut from: usize, mut to: usize, left: &str) -> (usize, usize) {
    if from >= to || left.is_empty() {
        return (from, to);
    }
    let carries = line_carries(text);
    let start_line = byte_pos(text, from).line;
    let end_line = byte_pos(text, to.saturating_sub(1)).line;
    for line_no in start_line..=end_line {
        let ls = line_start(text, line_no);
        let le = line_end(text, line_no);
        let line = text.get(ls..le).unwrap_or("");
        let carry = carries.get(line_no).copied().unwrap_or_default();
        let (pairs, _) = scan_pairs(line, carry);
        for pair in pairs {
            if !pair.local_open || pair.open_len != left.len() || pair.close_len != left.len() {
                continue;
            }
            let abs_start = ls.saturating_add(pair.start);
            let abs_end = ls.saturating_add(pair.end);
            if abs_end <= from || abs_start >= to {
                continue;
            }
            if text.get(abs_start..abs_start.saturating_add(left.len())) != Some(left) {
                continue;
            }
            if abs_start < from {
                from = abs_start;
            }
            if abs_end > to {
                to = abs_end;
            }
        }
    }
    (from, to)
}

fn strip_delim(inner: &str, delim: &str) -> String {
    if delim.is_empty() || !inner.contains(delim) {
        return inner.to_owned();
    }
    let Some(mark) = delim.chars().next() else {
        return inner.to_owned();
    };
    if !delim.chars().all(|char| char == mark) {
        return inner.to_owned();
    }
    let mut out = String::with_capacity(inner.len());
    let mut index = 0;
    while index < inner.len() {
        if inner[index..].starts_with(delim) {
            let end = index.saturating_add(delim.len());
            let longer_before = index > 0
                && inner
                    .get(..index)
                    .and_then(|prefix| prefix.chars().next_back())
                    == Some(mark);
            let longer_after = inner.get(end..).is_some_and(|rest| rest.starts_with(mark));
            if !longer_before && !longer_after {
                index = end;
                continue;
            }
        }
        let Some(char) = inner.get(index..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        out.push(char);
        index = index.saturating_add(char.len_utf8());
    }
    out
}

fn link_insert(text: &str, cursor: &text_editor::Cursor) -> Change {
    let (from, to, inner) = label_range(text, cursor);
    let insert = format!("[{inner}](https://)");
    let url_off = 1 + inner.len() + 2;
    let url_end = url_off + "https://".len();
    if inner.is_empty() {
        rewrite(text, from, to, &insert, 1, None)
    } else {
        rewrite(text, from, to, &insert, url_end, Some((url_off, url_end)))
    }
}

fn wiki_insert(text: &str, cursor: &text_editor::Cursor) -> Change {
    let (from, to, inner) = label_range(text, cursor);
    let insert = format!("[[{inner}]]");
    let at = if inner.is_empty() { 2 } else { insert.len() };
    rewrite(text, from, to, &insert, at, None)
}

/// A note link that stores the note id and shows `title`.
pub(super) fn note_link_change(
    text: &str,
    cursor: &text_editor::Cursor,
    id: &str,
    title: &str,
) -> Change {
    let (from, to, _) = label_range(text, cursor);
    let label = title.replace(['|', '[', ']'], "");
    let insert = format!("[[{id}|{label}]]");
    rewrite(text, from, to, &insert, insert.len(), None)
}

/// The unclosed `[[` before the caret, and the text typed after it.
pub(super) fn open_wiki(text: &str, caret: usize) -> Option<(usize, String)> {
    let caret = caret.min(text.len());
    let before = text.get(..caret)?;
    let start = before.rfind("[[")?;
    let query = before.get(start.saturating_add(2)..)?;
    if query.contains('\n') || query.contains(']') || query.contains('|') {
        return None;
    }
    Some((start, query.to_owned()))
}

/// A note id is 32 bytes of hex.
pub(super) fn is_note_id(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

thread_local! {
    static KNOWN_LINKS: RefCell<Option<Arc<BTreeSet<String>>>> = const { RefCell::new(None) };
    static MISSPELLINGS: RefCell<Arc<BTreeSet<String>>> =
        RefCell::new(Arc::new(BTreeSet::new()));
}

/// Ids and lowercase titles a note link can resolve. `None` draws every link as known.
pub(super) fn set_known_links(known: Option<Arc<BTreeSet<String>>>) {
    KNOWN_LINKS.with(|slot| *slot.borrow_mut() = known);
}

/// Words to underline. The caret's thread only reads this set.
pub(crate) fn set_misspellings(words: Arc<BTreeSet<String>>) {
    MISSPELLINGS.with(|slot| *slot.borrow_mut() = words);
}

fn wiki_missing(line: &str, start: usize, end: usize) -> bool {
    let Some((_, target)) = wiki_name(line, start) else {
        return false;
    };
    if end < start {
        return false;
    }
    KNOWN_LINKS.with(|slot| {
        let known = slot.borrow();
        let Some(known) = known.as_ref() else {
            return false;
        };
        if is_note_id(&target) {
            !known.contains(&target)
        } else {
            !known.contains(&target.to_lowercase())
        }
    })
}

fn spell_look(text: &str, mut look: Look) -> Look {
    if look.underline || look.font == Font::MONOSPACE {
        return look;
    }
    let Some(key) = spell_key(text) else {
        return look;
    };
    let bad = MISSPELLINGS.with(|slot| slot.borrow().contains(&key));
    if !bad {
        return look;
    }
    look.underline = true;
    look.color = Some(Color::from_rgb8(0xB0, 0x4A, 0x42));
    look
}

/// One quiet line when a mark or a note link is still open. `None` when the page is closed.
pub(crate) fn broken_mark(text: &str) -> Option<String> {
    let mut carry = Carry::default();
    let lines = split_lines(text);
    for (index, line) in lines.iter().enumerate() {
        let blocked = carry.fence || carry.math || carry.frontmatter;
        if !blocked && line.trim().is_empty() && carry.len > 0 {
            return Some(mark_warning(top(carry)));
        }
        if !blocked && open_wiki_line(line) {
            return Some("A note link is still open.".to_owned());
        }
        let (_, next) = paint_in(line, carry, index == 0);
        carry = next;
    }
    if carry.len > 0 {
        return Some(mark_warning(top(carry)));
    }
    None
}

fn mark_warning(open: Option<Open>) -> String {
    let name = match open {
        Some(Open::StrongStar | Open::StrongUnder | Open::BothStar | Open::BothUnder) => "Bold",
        Some(Open::EmStar | Open::EmUnder) => "Italic",
        Some(Open::Strike) => "Strike",
        Some(Open::Highlight) => "Highlight",
        Some(Open::Code) => "Code",
        Some(Open::Comment) => "A comment",
        None => "A mark",
    };
    format!("{name} is still open.")
}

fn open_wiki_line(line: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find("[[") {
        let after = rest.get(at.saturating_add(2)..).unwrap_or("");
        let Some(close) = after.find("]]") else {
            return true;
        };
        rest = after.get(close.saturating_add(2)..).unwrap_or("");
    }
    false
}

/// Words, as typed, that are not in the dictionary. Links, code, and the tag line are skipped.
///
/// A word is kept as it was written, apart from the punctuation around it, because that is
/// the key the underline looks up. Names, acronyms, and code-like words are not judged. See
/// [`spell_key`] and [`collect_words`].
pub(crate) fn misspellings(text: &str) -> BTreeSet<String> {
    let dict = dictionary();
    let mut bad = BTreeSet::new();
    let mut carry = Carry::default();
    for (index, line) in split_lines(text).iter().enumerate() {
        let skip = carry.fence
            || carry.math
            || carry.frontmatter
            || carry.indent_code
            || line.trim_start().starts_with("%%ambient-tags:");
        let (spans, next) = paint_in(line, carry, index == 0);
        if !skip {
            let mut starts_sentence = true;
            let mut cursor = 0;
            for (range, mark) in &spans {
                if range.start > cursor {
                    collect_words(
                        line,
                        cursor..range.start,
                        dict,
                        &mut starts_sentence,
                        &mut bad,
                    );
                }
                if spell_face(mark.face) {
                    collect_words(line, range.clone(), dict, &mut starts_sentence, &mut bad);
                }
                cursor = cursor.max(range.end);
            }
            if cursor < line.len() {
                collect_words(
                    line,
                    cursor..line.len(),
                    dict,
                    &mut starts_sentence,
                    &mut bad,
                );
            }
        }
        carry = next;
    }
    bad
}

/// Check each word in `range`. `starts_sentence` is true at the start of a line and after a
/// word that ends a sentence. A capitalized word that is not at the start of a sentence and is
/// not in the dictionary is taken to be a name and is left alone.
fn collect_words(
    line: &str,
    range: Range<usize>,
    dict: &HashSet<&'static str>,
    starts_sentence: &mut bool,
    bad: &mut BTreeSet<String>,
) {
    let Some(slice) = line.get(range) else {
        return;
    };
    for word in slice.split_whitespace() {
        let at_start = *starts_sentence;
        if word.chars().any(char::is_alphanumeric) {
            *starts_sentence = ends_sentence(word);
        }
        let Some(key) = spell_key(word) else {
            continue;
        };
        if !at_start && key.starts_with(|ch: char| ch.is_ascii_uppercase()) {
            continue;
        }
        if !key_spelled(dict, &key) {
            bad.insert(key);
        }
    }
}

fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches([
        '"', '\'', ')', ']', '*', '_', '~', '=', '\u{201d}', '\u{2019}',
    ])
    .ends_with(['.', '!', '?', ':', '\u{2026}'])
}

fn spell_face(face: Face) -> bool {
    matches!(
        face,
        Face::Text
            | Face::Heading(_)
            | Face::Quote
            | Face::Check
            | Face::Checked
            | Face::Callout(_)
    )
}

/// The word to look up for one whitespace-delimited piece of text: the piece as typed, without
/// the punctuation around it. Curly apostrophes become `'` and en and em dashes become `-`.
///
/// `None` means the piece is not for the spellchecker, so it is never underlined. That covers
/// anything that is not plain English letters: a word with a digit, an accent, or a symbol in
/// it (a file name, an address, a path, an identifier), a `#tag` or `@name`, an acronym, and a word
/// with a capital inside it (`iPhone`, `JavaScript`). Only `'`, `-`, and `/` may join letters.
/// The underline and the proof both call this, so they always agree on the word.
fn spell_key(raw: &str) -> Option<String> {
    if raw.starts_with(['#', '@', '/', '~', '\\']) {
        return None;
    }
    let token: String = raw
        .chars()
        .map(|ch| match ch {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{2013}' | '\u{2014}' => '-',
            other => other,
        })
        .collect();
    if token
        .chars()
        .any(|ch| ch.is_alphabetic() && !ch.is_ascii_alphabetic())
    {
        return None;
    }
    let trimmed = token.trim_matches(|ch: char| !ch.is_ascii_alphabetic());
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphabetic() || matches!(ch, '\'' | '-' | '/'))
    {
        return None;
    }
    let letters = trimmed.chars().filter(char::is_ascii_alphabetic).count();
    if !(3..64).contains(&letters) {
        return None;
    }
    let capitals = trimmed.chars().filter(char::is_ascii_uppercase).count();
    let first_is_capital = trimmed.starts_with(|ch: char| ch.is_ascii_uppercase());
    if capitals > 1 || (capitals == 1 && !first_is_capital) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// Prefixes that are not words alone but are written before a hyphen: `pre-approved`,
/// `non-profit`, `anti-pattern`. Shorter ones (`re`, `un`, `co`) are never judged.
const PREFIXES: [&str; 22] = [
    "pre", "non", "pro", "sub", "mid", "mis", "out", "anti", "semi", "multi", "over", "under",
    "inter", "intra", "post", "meta", "mini", "mega", "ultra", "super", "hyper", "micro",
];

/// A word is spelled when every part of it is. Parts are split at `-` and `/`, so `well-known`
/// and `and/or` are two words each. A prefix from [`PREFIXES`] passes before a hyphen.
fn key_spelled(dict: &HashSet<&'static str>, key: &str) -> bool {
    let parts: Vec<&str> = key.split(['-', '/']).collect();
    let last = parts.len().saturating_sub(1);
    parts.iter().enumerate().all(|(at, part)| {
        part_spelled(dict, part)
            || (at < last && PREFIXES.contains(&part.to_ascii_lowercase().as_str()))
    })
}

fn part_spelled(dict: &HashSet<&'static str>, part: &str) -> bool {
    if part.chars().filter(char::is_ascii_alphabetic).count() < 3 {
        return true;
    }
    let lower = part.to_ascii_lowercase();
    if lower
        .split_once('\'')
        .is_some_and(|(head, tail)| clitic_spelled(dict, head, tail))
    {
        return true;
    }
    let bare: String = lower.chars().filter(char::is_ascii_alphabetic).collect();
    spelled(dict, &bare)
}

/// `it's`, `John's`, `don't`, `they're`, `we've`, `I'll`, `she'd`, `I'm`, and a few old ones.
/// A stem of one or two letters is not judged, because the dictionary holds words of three or more.
fn clitic_spelled(dict: &HashSet<&'static str>, head: &str, tail: &str) -> bool {
    match tail {
        "s" | "re" | "ve" | "ll" | "d" | "m" => head.len() < 3 || spelled(dict, head),
        // Only a verb that takes `n't`. A loose rule would accept `dosn't` because `dos` is a word.
        "t" => head.strip_suffix('n').is_some_and(|stem| {
            matches!(
                stem,
                "do" | "does"
                    | "did"
                    | "is"
                    | "are"
                    | "was"
                    | "were"
                    | "has"
                    | "have"
                    | "had"
                    | "would"
                    | "could"
                    | "should"
                    | "must"
                    | "might"
                    | "need"
                    | "dare"
                    | "ought"
                    | "ca"
                    | "wo"
                    | "sha"
                    | "ai"
            )
        }),
        _ => matches!(
            (head, tail),
            ("o", "clock") | ("ma", "am") | ("y", "all") | ("ne" | "e", "er")
        ),
    }
}

/// Words the person added to their own dictionary. The proof runs on its own thread, so this is
/// shared and not kept on the window.
static PERSONAL: std::sync::RwLock<BTreeSet<String>> = std::sync::RwLock::new(BTreeSet::new());

/// Replace the personal words. Each is lowercase letters.
pub(crate) fn set_personal_words(words: &[String]) {
    let mut held = PERSONAL
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *held = words.iter().cloned().collect();
}

fn is_personal(bare: &str) -> bool {
    PERSONAL
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(bare)
}

/// The word at a byte, as it would go in the dictionary: lowercase letters, three or more.
/// `None` when the caret is not in a word, or the word has a digit or a symbol in it.
pub(super) fn word_at(text: &str, byte: usize) -> Option<String> {
    let (start, end) = word_bounds(text, byte);
    let word = text.get(start..end)?.trim_matches('\'');
    if word.len() < 3
        || !word
            .bytes()
            .all(|ch| ch.is_ascii_alphabetic() || ch == b'\'')
    {
        return None;
    }
    Some(word.to_ascii_lowercase())
}

fn spelled(dict: &HashSet<&'static str>, bare: &str) -> bool {
    dict.contains(bare)
        || is_personal(bare)
        || contraction(bare)
        || bare
            .strip_suffix('s')
            .is_some_and(|stem| stem.len() >= 3 && (dict.contains(stem) || is_personal(stem)))
}

fn contraction(bare: &str) -> bool {
    matches!(
        bare,
        "dont"
            | "cant"
            | "wont"
            | "couldnt"
            | "wouldnt"
            | "shouldnt"
            | "isnt"
            | "arent"
            | "wasnt"
            | "werent"
            | "hasnt"
            | "havent"
            | "hadnt"
            | "doesnt"
            | "didnt"
            | "youre"
            | "theyre"
            | "ive"
            | "youve"
            | "weve"
            | "theyve"
            | "lets"
            | "thats"
            | "whats"
            | "theres"
            | "heres"
            | "whos"
            | "couldve"
            | "shouldve"
            | "wouldve"
            | "mightve"
            | "mustve"
            | "aint"
            | "mustnt"
    )
}

const WORDS: &str = include_str!("../dict/words.txt");
const WORDS_EXTRA: &str = include_str!("../dict/words-extra.txt");

/// Every word the proof accepts. The lines are borrowed from the compiled-in files, so loading
/// does no per-word allocation. `dict/README.md` says where the list came from. A test keeps
/// both files lowercase, sorted, and free of duplicates, so nothing is trimmed or lowercased here.
fn dictionary() -> &'static HashSet<&'static str> {
    static LOADED: OnceLock<HashSet<&'static str>> = OnceLock::new();
    LOADED.get_or_init(|| {
        WORDS
            .lines()
            .chain(WORDS_EXTRA.lines())
            .filter(|word| !word.is_empty())
            .collect()
    })
}

fn image_insert(text: &str, cursor: &text_editor::Cursor) -> Change {
    let (from, to, inner) = label_range(text, cursor);
    let insert = format!("![{inner}]()");
    let at = if inner.is_empty() {
        2
    } else {
        insert.len() - 1
    };
    rewrite(text, from, to, &insert, at, None)
}

fn rule_insert(text: &str, cursor: &text_editor::Cursor) -> Change {
    let at = pos_byte(text, cursor.position);
    let fresh = at == 0 || text.as_bytes().get(at.saturating_sub(1)) == Some(&b'\n');
    let insert = if fresh { "---\n" } else { "\n---\n" };
    rewrite(text, at, at, insert, insert.len(), None)
}

fn code_block(text: &str, cursor: &text_editor::Cursor) -> Change {
    let (from, to) = range_bytes(text, cursor);
    let inner = text.get(from..to).unwrap_or("");
    if inner.is_empty() {
        let (insert, at) = own_line(text, from, "```\n\n```\n", "```\n".len());
        return rewrite(text, from, to, &insert, at, None);
    }
    let body = format!("```\n{inner}\n```\n");
    let (insert, at) = own_line(text, from, &body, "```\n".len() + inner.len());
    rewrite(text, from, to, &insert, at, None)
}

fn callout_insert(text: &str, cursor: &text_editor::Cursor) -> Change {
    let (from, to) = range_bytes(text, cursor);
    let inner = text.get(from..to).unwrap_or("");
    let body = if inner.is_empty() {
        "> [!note] Title\n> \n".to_owned()
    } else {
        format!("> [!note] Title\n> {inner}\n")
    };
    let title_at = "> [!note] ".len();
    let (insert, lead) = own_line(text, from, &body, title_at);
    let title_end = lead + "Title".len();
    rewrite(text, from, to, &insert, title_end, Some((lead, title_end)))
}

fn block_at_line(
    text: &str,
    cursor: &text_editor::Cursor,
    body: &str,
    cursor_off: usize,
) -> Change {
    let at = line_end(text, cursor.position.line);
    let (insert, lead) = own_line(text, at, body, cursor_off);
    rewrite(text, at, at, &insert, lead, None)
}

fn map_lines(text: &str, cursor: &text_editor::Cursor, map: fn(&str) -> String) -> Change {
    let (start, end) = ordered(cursor);
    let lines = split_lines(text);
    let last_index = lines.len().saturating_sub(1);
    let first = start.line.min(last_index);
    let last = end.line.min(last_index);
    let from = line_start(text, first);
    let to = line_end(text, last);
    let mapped: Vec<String> = lines
        .iter()
        .take(last + 1)
        .skip(first)
        .map(|line| map(line))
        .collect();
    let caret_line = cursor.position.line.min(last).max(first);
    let old = lines.get(caret_line).copied().unwrap_or("");
    let new = mapped.get(caret_line - first).map_or("", String::as_str);
    let column = floor_boundary(
        new,
        cursor
            .position
            .column
            .saturating_add(new.len())
            .saturating_sub(old.len()),
    );
    let mut cursor_off: usize = 0;
    for (index, line) in mapped.iter().enumerate() {
        if first + index == caret_line {
            break;
        }
        cursor_off = cursor_off.saturating_add(line.len()).saturating_add(1);
    }
    cursor_off = cursor_off.saturating_add(column.min(new.len()));
    rewrite(text, from, to, &mapped.join("\n"), cursor_off, None)
}

fn cycle_heading(line: &str) -> String {
    let spaces = leading_spaces(line);
    let indent = line.get(..spaces).unwrap_or("");
    if let Some((end, level)) = heading_end(line) {
        let body = line.get(end..).unwrap_or("");
        if level >= 3 {
            return format!("{indent}{body}");
        }
        let marks = "#".repeat(usize::from(level.saturating_add(1)));
        return format!("{indent}{marks} {body}");
    }
    let rest = line.get(spaces..).unwrap_or("");
    format!("{indent}# {rest}")
}

fn toggle_list(line: &str) -> String {
    let spaces = leading_spaces(line);
    let indent = line.get(..spaces).unwrap_or("");
    if let Some(list) = list_parts(line) {
        let body = line.get(list.end..).unwrap_or("");
        return format!("{indent}{body}");
    }
    let rest = line.get(spaces..).unwrap_or("");
    format!("{indent}- {rest}")
}

fn toggle_task(line: &str) -> String {
    let spaces = leading_spaces(line);
    let indent = line.get(..spaces).unwrap_or("");
    if let Some(list) = list_parts(line) {
        if list.box_at.is_some() {
            let body = line.get(list.end..).unwrap_or("").trim_start();
            return format!("{indent}{body}");
        }
        let prefix = line.get(..list.end).unwrap_or("");
        let body = line.get(list.end..).unwrap_or("");
        return format!("{prefix}[ ] {body}");
    }
    let rest = line.get(spaces..).unwrap_or("");
    format!("{indent}- [ ] {rest}")
}

fn toggle_quote(line: &str) -> String {
    let spaces = leading_spaces(line);
    let indent = line.get(..spaces).unwrap_or("");
    let rest = line.get(spaces..).unwrap_or("");
    if spaces <= 3 && rest.starts_with('>') {
        let skip = if rest.as_bytes().get(1) == Some(&b' ') {
            2
        } else {
            1
        };
        return format!("{indent}{}", rest.get(skip..).unwrap_or(""));
    }
    format!("{indent}> {rest}")
}

fn label_range<'a>(text: &'a str, cursor: &text_editor::Cursor) -> (usize, usize, &'a str) {
    let (mut from, mut to) = range_bytes(text, cursor);
    if from == to {
        let (word_from, word_to) = word_bounds(text, from);
        if word_from != word_to {
            from = word_from;
            to = word_to;
        }
    }
    (from, to, text.get(from..to).unwrap_or(""))
}

fn own_line(text: &str, at: usize, body: &str, cursor_off: usize) -> (String, usize) {
    let mut insert = String::new();
    let mut shift: usize = 0;
    if at > 0 && text.as_bytes().get(at.saturating_sub(1)) != Some(&b'\n') {
        insert.push('\n');
        shift = 1;
    }
    insert.push_str(body);
    if !insert.ends_with('\n') {
        insert.push('\n');
    }
    (insert, shift.saturating_add(cursor_off))
}

fn rewrite(
    text: &str,
    from: usize,
    to: usize,
    insert: &str,
    cursor_off: usize,
    select: Option<(usize, usize)>,
) -> Change {
    let anchor = byte_pos(text, from);
    let end = byte_pos(text, to);
    let caret_off = cursor_off.min(insert.len());
    let selection = select.map(|(start, _)| byte_pos_new(text, from, to, insert, start));
    let after_off = select.map_or(caret_off, |(_, off)| off).min(insert.len());
    let after = byte_pos_new(text, from, to, insert, after_off);
    Change {
        anchor,
        end,
        insert: insert.to_owned(),
        after: text_editor::Cursor {
            position: after,
            selection,
        },
    }
}

fn byte_pos_new(
    text: &str,
    from: usize,
    to: usize,
    insert: &str,
    off: usize,
) -> text_editor::Position {
    let mut next = String::new();
    next.push_str(text.get(..from).unwrap_or(""));
    next.push_str(insert);
    next.push_str(text.get(to..).unwrap_or(""));
    byte_pos(&next, from.saturating_add(off.min(insert.len())))
}

fn ordered(cursor: &text_editor::Cursor) -> (text_editor::Position, text_editor::Position) {
    let anchor = cursor.selection.unwrap_or(cursor.position);
    if (anchor.line, anchor.column) <= (cursor.position.line, cursor.position.column) {
        (anchor, cursor.position)
    } else {
        (cursor.position, anchor)
    }
}

fn range_bytes(text: &str, cursor: &text_editor::Cursor) -> (usize, usize) {
    let (start, end) = ordered(cursor);
    let from = pos_byte(text, start);
    let to = pos_byte(text, end);
    (from.min(to), from.max(to))
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        vec![""]
    } else {
        text.split('\n').collect()
    }
}

/// The line the light keeps. One source line, so a wrapped line stays together and the next
/// line does not. A blank-line block was leaving everything after the first break bright.
fn spotlight_at(text: &str, caret: usize) -> usize {
    line_index(text, caret)
}

thread_local! {
    static SPOTLIGHT: RefCell<Option<(usize, usize)>> = const { RefCell::new(None) };
}

fn set_spotlight(range: Option<(usize, usize)>) {
    SPOTLIGHT.with(|slot| *slot.borrow_mut() = range);
}

fn line_dim(index: usize) -> bool {
    SPOTLIGHT.with(|slot| {
        slot.borrow()
            .is_some_and(|(start, end)| index < start || index > end)
    })
}

fn dim_pieces(mut pieces: Vec<Piece>, dark: bool) -> Vec<Piece> {
    let muted = if dark {
        Color::from_rgb8(0x6A, 0x64, 0x5E)
    } else {
        Color::from_rgb8(0xB7, 0xB1, 0xA6)
    };
    for piece in &mut pieces {
        piece.look.color = Some(Color { a: 0.72, ..muted });
        piece.look.background = None;
    }
    pieces
}

fn line_start(text: &str, line: usize) -> usize {
    let mut start = 0;
    for (index, part) in text.split('\n').enumerate() {
        if index == line {
            return start;
        }
        start = start.saturating_add(part.len()).saturating_add(1);
    }
    text.len()
}

fn line_end(text: &str, line: usize) -> usize {
    let start = line_start(text, line);
    let part = text.get(start..).unwrap_or("");
    start.saturating_add(part.find('\n').unwrap_or(part.len()))
}

fn pos_byte(text: &str, pos: text_editor::Position) -> usize {
    let start = line_start(text, pos.line);
    let line = text.get(start..).unwrap_or("");
    let line = line
        .get(..line.find('\n').unwrap_or(line.len()))
        .unwrap_or("");
    start.saturating_add(floor_boundary(line, pos.column.min(line.len())))
}

fn byte_pos(text: &str, byte: usize) -> text_editor::Position {
    let byte = byte.min(text.len());
    let mut seen: usize = 0;
    for (line, part) in text.split('\n').enumerate() {
        let end = seen.saturating_add(part.len());
        if byte <= end {
            return text_editor::Position {
                line,
                column: byte.saturating_sub(seen),
            };
        }
        seen = end.saturating_add(1);
    }
    text_editor::Position { line: 0, column: 0 }
}

fn floor_boundary(line: &str, column: usize) -> usize {
    let mut column = column.min(line.len());
    while column > 0 && !line.is_char_boundary(column) {
        column -= 1;
    }
    column
}

pub(super) fn word_bounds(text: &str, at: usize) -> (usize, usize) {
    let at = at.min(text.len());
    let mut start = at;
    while start > 0 {
        let Some(char) = text
            .get(..start)
            .and_then(|prefix| prefix.chars().next_back())
        else {
            break;
        };
        if !is_word(char) {
            break;
        }
        start = start.saturating_sub(char.len_utf8());
    }
    let mut end = at;
    while end < text.len() {
        let Some(char) = text.get(end..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        if !is_word(char) {
            break;
        }
        end = end.saturating_add(char.len_utf8());
    }
    (start, end)
}

pub(super) fn same_word(text: &str, left: usize, right: usize) -> bool {
    let (start, end) = word_bounds(text, left);
    if start == end {
        return false;
    }
    let (other_start, other_end) = word_bounds(text, right);
    start == other_start && end == other_end
}

pub(super) fn line_index(text: &str, byte: usize) -> usize {
    byte_pos(text, byte.min(text.len())).line
}

pub(super) fn line_bounds(text: &str, at: usize) -> (usize, usize) {
    let line = line_index(text, at);
    (line_start(text, line), line_end(text, line))
}

pub(super) fn same_line(text: &str, left: usize, right: usize) -> bool {
    line_index(text, left) == line_index(text, right)
}

fn is_word(char: char) -> bool {
    char.is_alphanumeric() || char == '_' || char == '\''
}

fn surrounded(text: &str, from: usize, to: usize, left: &str, right: &str) -> bool {
    if from < left.len()
        || !text
            .get(..from)
            .is_some_and(|prefix| prefix.ends_with(left))
    {
        return false;
    }
    if !text
        .get(to..)
        .is_some_and(|suffix| suffix.starts_with(right))
    {
        return false;
    }
    let Some(mark) = left.chars().next() else {
        return false;
    };
    if !left.chars().all(|item| item == mark) {
        return true;
    }
    let before = from.saturating_sub(left.len());
    if before > 0
        && text
            .get(..before)
            .and_then(|prefix| prefix.chars().next_back())
            == Some(mark)
    {
        return false;
    }
    let after = to.saturating_add(right.len());
    text.get(after..).and_then(|suffix| suffix.chars().next()) != Some(mark)
}

fn peel<'a>(inner: &'a str, left: &str, right: &str) -> Option<&'a str> {
    if inner.len() < left.len() + right.len() || !inner.starts_with(left) || !inner.ends_with(right)
    {
        return None;
    }
    let mark = left.chars().next()?;
    if left.chars().all(|char| char == mark) && inner[left.len()..].starts_with(mark) {
        return None;
    }
    let before_right = &inner[..inner.len() - right.len()];
    if right.chars().all(|char| char == mark) && before_right.ends_with(mark) {
        return None;
    }
    Some(&inner[left.len()..inner.len() - right.len()])
}

const BODY: f32 = 17.0;
const LEADING: f32 = 1.45;

fn line_px(line: &str) -> f32 {
    let size = heading_end(line).map_or(BODY, |(_, level)| heading_px(level));
    size * LEADING
}

fn heading_px(level: u8) -> f32 {
    // The note title is the largest heading. These step down from it.
    match level {
        1 => 28.0,
        2 => 24.0,
        3 => 21.0,
        4 => 19.0,
        5 => 17.0,
        _ => 15.0,
    }
}

/// A click on a hidden mark lands beside it, not on the mark.
///
/// The closer is the outside edge, so typing there does not keep the highlight
/// going. The opener is the inside edge.
pub(super) fn snap_caret(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    let start = byte.saturating_sub(2);
    let end = byte.saturating_add(2).min(text.len());
    let window = text.as_bytes().get(start..end).unwrap_or(b"");
    if !window
        .iter()
        .any(|byte| matches!(byte, b'=' | b'*' | b'_' | b'~' | b'`'))
    {
        return byte;
    }
    let carries = line_carries(text);
    let line_no = byte_pos(text, byte).line;
    let ls = line_start(text, line_no);
    let le = line_end(text, line_no);
    let line = text.get(ls..le).unwrap_or("");
    let local = byte.saturating_sub(ls).min(line.len());
    let carry = carries.get(line_no).copied().unwrap_or_default();
    let (pairs, _) = scan_pairs(line, carry);
    for pair in pairs {
        let open_end = pair.start.saturating_add(pair.open_len);
        let close_at = pair.end.saturating_sub(pair.close_len);
        if pair.local_open && pair.start <= local && local < open_end {
            return ls.saturating_add(open_end.min(line.len()));
        }
        if local >= close_at && local < pair.end {
            return ls.saturating_add(pair.end.min(line.len()));
        }
    }
    byte
}

fn line_carries(text: &str) -> Vec<Carry> {
    cache_lines(text).into_iter().map(|row| row.carry).collect()
}

#[derive(Clone, Debug, PartialEq)]
struct LineCache {
    carry: Carry,
    spans: Vec<(Range<usize>, Mark)>,
    unclosed: Vec<usize>,
    /// What the next line starts with. An edit restarts from the line before the change.
    outgoing: Carry,
}

fn cache_line(line: &str, incoming: Carry, first: bool) -> LineCache {
    let stored = if first {
        Carry {
            blank: true,
            ..incoming
        }
    } else {
        incoming
    };
    let (spans, next) = paint_in(line, stored, first);
    let (_, unclosed) = scan_pairs(line, stored);
    LineCache {
        carry: stored,
        spans,
        unclosed,
        outgoing: next,
    }
}

fn first_carry() -> Carry {
    Carry {
        blank: true,
        ..Carry::default()
    }
}

fn cache_lines(text: &str) -> Vec<LineCache> {
    let mut incoming = first_carry();
    let mut rows = Vec::new();
    for (index, line) in split_lines(text).into_iter().enumerate() {
        let row = cache_line(line, incoming, index == 0);
        incoming = row.outgoing;
        rows.push(row);
    }
    rows
}

/// The same rows as `cache_lines(new)`, built from the rows for `old`.
///
/// A line's row depends on the line and on the marks still open from the line before. So the
/// lines that are the same at the top keep their rows. The changed lines are parsed again. Below
/// them, rows are kept as soon as the marks open at the top of a line match what they were
/// before, which is almost always the very next line. Typing in a long note parses the line
/// that changed, not the note.
fn update_lines(old: &str, cache: Vec<LineCache>, new: &str) -> Vec<LineCache> {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    if cache.len() != old_lines.len() {
        return cache_lines(new);
    }
    let top = old_lines
        .iter()
        .zip(&new_lines)
        .take_while(|(left, right)| left == right)
        .count();
    let room = old_lines.len().min(new_lines.len()).saturating_sub(top);
    let bottom = old_lines
        .iter()
        .rev()
        .zip(new_lines.iter().rev())
        .take(room)
        .take_while(|(left, right)| left == right)
        .count();
    let mut rows = cache;
    let kept_below = rows.split_off(rows.len().saturating_sub(bottom));
    rows.truncate(top);
    let mut incoming = rows.last().map_or_else(first_carry, |row| row.outgoing);
    let changed_to = new_lines.len().saturating_sub(bottom);
    for index in top..changed_to {
        let Some(line) = new_lines.get(index) else {
            break;
        };
        let row = cache_line(line, incoming, index == 0);
        incoming = row.outgoing;
        rows.push(row);
    }
    let mut below = kept_below.into_iter();
    for index in changed_to..new_lines.len() {
        let Some(line) = new_lines.get(index) else {
            break;
        };
        let Some(row) = below.next() else {
            break;
        };
        let expected = if index == 0 {
            Carry {
                blank: true,
                ..incoming
            }
        } else {
            incoming
        };
        if row.carry == expected {
            // The marks open here are what they were. Nothing below can have changed.
            rows.push(row);
            rows.extend(below);
            return rows;
        }
        let row = cache_line(line, incoming, index == 0);
        incoming = row.outgoing;
        rows.push(row);
    }
    rows
}

fn window_bytes(
    text: &str,
    cursor: &text_editor::Cursor,
    cache: &[LineCache],
    touching: bool,
) -> (usize, usize) {
    let caret = pos_byte(text, cursor.position);
    let line_no = byte_pos(text, caret).line;
    let ls = line_start(text, line_no);
    let le = line_end(text, line_no);
    let line = text.get(ls..le).unwrap_or("");
    let incoming = cache.get(line_no).map(|row| row.carry).unwrap_or_default();
    let at = floor_boundary(line, caret.saturating_sub(ls).min(line.len()));
    let win = edit_window(line, at, incoming, touching);
    let start = floor_boundary(line, win.start.min(line.len()));
    let end = floor_boundary(line, win.end.max(start).min(line.len()));
    let info = lead(line);
    if info.box_at.is_some() {
        let skip = info.skip.min(line.len());
        let start = start.max(skip);
        let end = end.max(start);
        return (ls.saturating_add(start), ls.saturating_add(end));
    }
    (ls.saturating_add(start), ls.saturating_add(end))
}

/// The part of a line to show as typed, around the caret at `at`.
///
/// A mark that is still open shows from where it opens. A construct the caret is in shows whole,
/// so its marks can be read, changed, or taken out: the outermost pair of marks around the caret
/// (`**bold *and italic* too**`), or a whole link, image, or note link. With nothing around the
/// caret, it is the word it is in. `touching` is whether a caret just after a construct counts as
/// being in it. It does when the caret was moved there, so a Backspace acts on something that
/// can be seen. It does not right after typing, so a closing mark settles at once.
fn edit_window(line: &str, at: usize, carry: Carry, touching: bool) -> Range<usize> {
    let at = floor_boundary(line, at.min(line.len()));
    if line.is_empty() {
        return 0..0;
    }
    let scan = scan_all(line, carry);
    if let Some(start) = scan
        .unclosed
        .iter()
        .copied()
        .filter(|start| *start <= at)
        .min()
    {
        return start..line.len();
    }
    let around = |range: &Range<usize>| {
        range.start <= at && (at < range.end || (touching && at == range.end))
    };
    let pairs = scan
        .pairs
        .iter()
        .filter(|pair| pair.end > pair.start)
        .map(|pair| pair.start..pair.end);
    let outermost = pairs
        .chain(scan.atoms.iter().cloned())
        .filter(|range| around(range))
        .max_by_key(|range| range.end.saturating_sub(range.start));
    if let Some(range) = outermost {
        return range;
    }
    // Typing the last mark of a link settles it, as it does for a pair. Without this the last
    // word of a link with spaces in it stayed open until the caret moved on.
    if scan
        .atoms
        .iter()
        .any(|atom| atom.end == at && atom.end > atom.start)
    {
        return at..at;
    }
    caret_word(line, at, 0, line.len(), &scan.pairs)
}

#[derive(Clone, Copy)]
struct Pair {
    start: usize,
    end: usize,
    open_len: usize,
    close_len: usize,
    local_open: bool,
}

fn caret_word(line: &str, at: usize, lo: usize, hi: usize, pairs: &[Pair]) -> Range<usize> {
    let hi = hi.min(line.len());
    let lo = lo.min(hi);
    let at = at.clamp(lo, hi);
    if pairs
        .iter()
        .any(|pair| pair.end == at && pair.start >= lo && pair.end <= hi && pair.end > pair.start)
    {
        return at..at;
    }
    let mut index = lo;
    while index < hi {
        if let Some(pair) = pairs
            .iter()
            .copied()
            .filter(|pair| pair.start == index && pair.end <= hi && pair.end > pair.start)
            .min_by_key(|pair| pair.end.saturating_sub(pair.start))
        {
            if at == pair.end {
                return at..at;
            }
            if pair.start < at && at < pair.end {
                let inner_lo = pair.start.saturating_add(pair.open_len);
                let inner_hi = pair.end.saturating_sub(pair.close_len);
                if at >= inner_lo && at <= inner_hi {
                    return caret_word(line, at, inner_lo, inner_hi, pairs);
                }
                if pair.local_open {
                    return pair.start..pair.end;
                }
                let close_at = pair.end.saturating_sub(pair.close_len);
                return close_at..pair.end;
            }
            index = pair.end;
            continue;
        }
        let Some(ch) = line.get(index..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        if ch.is_whitespace() {
            index = index.saturating_add(ch.len_utf8());
            continue;
        }
        let start = index;
        while index < hi {
            if pairs
                .iter()
                .any(|pair| pair.start == index && pair.end <= hi && pair.end > pair.start)
            {
                break;
            }
            let Some(next) = line.get(index..).and_then(|rest| rest.chars().next()) else {
                break;
            };
            if next.is_whitespace() {
                break;
            }
            index = index.saturating_add(next.len_utf8());
        }
        if start < at && at <= index {
            return start..index;
        }
        if at == start {
            return at..at;
        }
    }
    at..at
}

fn scan_pairs(line: &str, carry: Carry) -> (Vec<Pair>, Vec<usize>) {
    let scan = scan_all(line, carry);
    (scan.pairs, scan.unclosed)
}

/// What one line holds that has marks around it: the pairs, the marks still open at the end, and
/// the atoms, which are links, images, note links, footnotes, math, and addresses.
struct Scan {
    pairs: Vec<Pair>,
    unclosed: Vec<usize>,
    atoms: Vec<Range<usize>>,
}

fn scan_all(line: &str, mut carry: Carry) -> Scan {
    let mut atoms = Vec::new();
    let mut pairs = Vec::new();
    let mut local = Vec::new();
    let mut index = 0;
    let limit = line.len();
    while index < limit {
        if line.as_bytes().get(index) == Some(&b'\\') {
            let escaped = char_len_at(line, index.saturating_add(1), limit);
            index = index.saturating_add(1).saturating_add(escaped);
            continue;
        }
        if !in_literal(carry)
            && let Some(end) = skip_atom(line, index, limit, carry)
        {
            atoms.push(index..end);
            index = end;
            continue;
        }
        if let Some((len, close)) = token(line, index, limit, carry) {
            let end = index.saturating_add(len);
            if close {
                if let Some((at, open_len)) = local.pop() {
                    pairs.push(Pair {
                        start: at,
                        end,
                        open_len,
                        close_len: len,
                        local_open: true,
                    });
                } else {
                    pairs.push(Pair {
                        start: 0,
                        end,
                        open_len: 0,
                        close_len: len,
                        local_open: false,
                    });
                }
                pop(&mut carry);
            } else {
                local.push((index, len));
                push_open(&mut carry, opened(line, index, limit));
            }
            index = end;
            continue;
        }
        let step = char_len_at(line, index, limit).max(1);
        index = index.saturating_add(step);
    }
    let unclosed = local.into_iter().map(|(at, _)| at).collect();
    Scan {
        pairs,
        unclosed,
        atoms,
    }
}

fn skip_atom(line: &str, index: usize, limit: usize, carry: Carry) -> Option<usize> {
    if in_literal(carry) {
        return None;
    }
    link_end(line, index, limit)
        .or_else(|| wiki_end(line, index, limit))
        .or_else(|| auto_end(line, index, limit))
        .or_else(|| bare_url(line, index, limit))
        .or_else(|| footnote_end(line, index, limit))
        .or_else(|| math_end(line, index, limit))
}

fn local_cursor(
    text: &str,
    cursor: &text_editor::Cursor,
    start: usize,
    end: usize,
) -> text_editor::Cursor {
    let position = local_pos(text, cursor.position, start, end);
    let selection = cursor
        .selection
        .map(|sel| local_pos(text, sel, start, end))
        .filter(|sel| sel.line != position.line || sel.column != position.column);
    text_editor::Cursor {
        position,
        selection,
    }
}

fn local_pos(
    text: &str,
    pos: text_editor::Position,
    start: usize,
    end: usize,
) -> text_editor::Position {
    let byte = pos_byte(text, pos).clamp(start, end);
    text_editor::Position {
        line: 0,
        column: byte.saturating_sub(start),
    }
}

fn abs_pos(text: &str, start: usize, pos: text_editor::Position) -> text_editor::Position {
    let byte = start.saturating_add(pos.column);
    byte_pos(text, byte.min(text.len()))
}

#[derive(Clone, Copy)]
struct Look {
    size: f32,
    font: Font,
    color: Option<Color>,
    strike: bool,
    underline: bool,
    background: Option<Color>,
}

fn look_of(mark: Mark, dark: bool) -> Look {
    let accent = accent(dark);
    let marker = if dark {
        Color::from_rgb8(0x6F, 0x69, 0x64)
    } else {
        Color::from_rgb8(0xA8, 0xA2, 0x9A)
    };
    let code = if dark {
        Color::from_rgb8(0xE0, 0xB3, 0x66)
    } else {
        Color::from_rgb8(0x8A, 0x5A, 0x12)
    };
    let done = if dark {
        Color::from_rgb8(0x7D, 0xC9, 0xA4)
    } else {
        Color::from_rgb8(0x1F, 0x7A, 0x4D)
    };
    let gold = if dark {
        Color::from_rgb8(0xE6, 0xC2, 0x5A)
    } else {
        Color::from_rgb8(0x8A, 0x62, 0x10)
    };
    let mut look = Look {
        size: BODY,
        font: sans(),
        color: None,
        strike: mark.flags & STRIKE != 0,
        underline: matches!(mark.face, Face::Link | Face::Wiki),
        background: (mark.flags & HIGH != 0).then_some(Color { a: 0.28, ..gold }),
    };
    if mark.face == Face::Code || mark.face == Face::Math {
        look.font = Font::MONOSPACE;
        look.color = Some(if mark.face == Face::Math {
            accent
        } else {
            code
        });
        return look;
    }
    look.color = match mark.face {
        Face::Text => None,
        Face::Marker | Face::Meta => Some(marker),
        Face::Link | Face::Image | Face::Check | Face::Quote => Some(accent),
        Face::Wiki => Some(if mark.flags & MISSING == 0 {
            accent
        } else {
            marker
        }),
        Face::Heading(level) => {
            look.size = heading_px(level);
            None
        }
        Face::Checked => Some(done),
        Face::Callout(kind) => Some(callout_color(kind, dark)),
        Face::Code | Face::Math => Some(code),
    };
    if mark.flags & STRIKE != 0 {
        look.strike = true;
    }
    look.font = if mark.flags & STRONG != 0 && mark.flags & EM != 0 {
        sans_bold_italic()
    } else if mark.flags & STRONG != 0 {
        sans_bold()
    } else if mark.flags & EM != 0 {
        sans_italic()
    } else {
        match mark.face {
            Face::Heading(level) if level <= 2 => serif(),
            Face::Heading(_) | Face::Check | Face::Checked | Face::Image => sans_bold(),
            _ => sans(),
        }
    };
    look
}

fn accent(dark: bool) -> Color {
    if dark {
        Color::from_rgb8(0x66, 0xA5, 0xE1)
    } else {
        Color::from_rgb8(0x16, 0x44, 0x6F)
    }
}

fn look_at(line: &str, at: usize, carry: Carry, at_start: bool, dark: bool) -> Look {
    let (spans, _) = paint_in(line, carry, at_start);
    let mut chosen = text_mark();
    for (range, mark) in spans {
        let covers = (range.start < at && at <= range.end) || (range.start <= at && at < range.end);
        if !covers {
            continue;
        }
        if mark.face != Face::Marker || chosen.face == Face::Text && chosen.flags == 0 {
            chosen = mark;
        }
    }
    let mut look = look_of(chosen, dark);
    if let Some((_, level)) = heading_end(line)
        && chosen.face != Face::Code
        && chosen.face != Face::Math
    {
        look.size = heading_px(level);
        if chosen.face == Face::Marker || (chosen.face == Face::Text && chosen.flags == 0) {
            look.font = if level <= 2 { serif() } else { sans_bold() };
        }
    }
    look
}

#[derive(Clone)]
struct Piece {
    text: String,
    /// Document byte where a click on this drawn text lands, when `literal`.
    origin: usize,
    /// The drawn letters are the source bytes. A bullet is not.
    literal: bool,
    look: Look,
    /// Document bytes this drawn text stands for. Selection uses this.
    covers: Range<usize>,
}

fn pieces_between(
    line: &str,
    base: usize,
    show: Range<usize>,
    carry: Carry,
    at_start: bool,
    dark: bool,
) -> Vec<Piece> {
    let (spans, _) = paint_in(line, carry, at_start);
    let (_, unclosed) = scan_pairs(line, carry);
    pieces_ready(line, base, show, &spans, &unclosed, dark)
}

fn pieces_ready(
    line: &str,
    base: usize,
    show: Range<usize>,
    spans: &[(Range<usize>, Mark)],
    unclosed: &[usize],
    dark: bool,
) -> Vec<Piece> {
    let lead = lead(line);
    let mut out = Vec::new();
    if show.start == 0
        && lead.skip > 0
        && lead.skip <= show.end
        && lead.box_at.is_none()
        && quote_end(line).is_none()
        && heading_end(line).is_none()
        && list_parts(line).is_some()
    {
        out.push(Piece {
            text: list_label(line),
            origin: base.saturating_add(lead.skip),
            literal: false,
            look: look_of(text_mark(), dark),
            covers: base..base.saturating_add(lead.skip),
        });
    }
    let plain = look_of(lead.base, dark);
    let limit = show.end.min(line.len());
    let mut cursor = show.start.min(limit);
    for pair in spans {
        let range = pair.0.clone();
        let mark = pair.1;
        if range.end <= cursor || range.start >= limit {
            continue;
        }
        let from = range.start.max(cursor);
        if from > cursor {
            push_slice(&mut out, line, cursor, from, base, plain);
            cursor = from;
        }
        if range.end <= lead.skip {
            cursor = range.end.min(limit);
            continue;
        }
        let draw_to = range.end.min(limit);
        let draw_from = cursor.max(lead.skip);
        if draw_from > cursor {
            cursor = draw_from;
        }
        if cursor >= draw_to {
            continue;
        }
        if mark.face == Face::Marker && hide_marker(line, cursor..draw_to, unclosed) {
            cursor = draw_to;
            continue;
        }
        push_slice(&mut out, line, cursor, draw_to, base, look_of(mark, dark));
        cursor = draw_to;
    }
    if cursor < limit {
        push_slice(&mut out, line, cursor, limit, base, plain);
    }
    out
}

fn hide_marker(line: &str, range: Range<usize>, unclosed: &[usize]) -> bool {
    let source = line.get(range.start..range.end).unwrap_or("");
    if source.chars().any(|ch| ch == '|') {
        return false;
    }
    !unclosed
        .iter()
        .any(|at| range.start <= *at && *at < range.end)
}

fn list_label(line: &str) -> String {
    let spaces = leading_indent(line);
    let rest = line.get(spaces..).unwrap_or("");
    let indent = line.get(..spaces).unwrap_or("");
    if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        return format!("{indent}• ");
    }
    line.get(..list_parts(line).map_or(0, |parts| parts.end))
        .unwrap_or("• ")
        .to_owned()
}

fn push_slice(out: &mut Vec<Piece>, line: &str, start: usize, end: usize, base: usize, look: Look) {
    if start >= end {
        return;
    }
    let Some(slice) = line.get(start..end) else {
        return;
    };
    push_words(out, slice, start, base, look);
}

fn push_words(out: &mut Vec<Piece>, text: &str, source_at: usize, base: usize, look: Look) {
    let mut index = 0;
    while index < text.len() {
        let Some(ch) = text.get(index..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        let mut end = index.saturating_add(ch.len_utf8());
        let space = ch.is_whitespace();
        while let Some(next) = text.get(end..).and_then(|rest| rest.chars().next()) {
            if next.is_whitespace() != space {
                break;
            }
            end = end.saturating_add(next.len_utf8());
        }
        let Some(slice) = text.get(index..end) else {
            break;
        };
        let origin = base.saturating_add(source_at).saturating_add(index);
        out.push(Piece {
            text: slice.to_owned(),
            origin,
            literal: true,
            look: spell_look(slice, look),
            covers: origin..origin.saturating_add(slice.len()),
        });
        index = end;
    }
}

fn place_of(pieces: &[Piece], shown: usize) -> usize {
    let mut seen = 0_usize;
    for (index, piece) in pieces.iter().enumerate() {
        let next = seen.saturating_add(piece.text.len());
        let last = index.saturating_add(1) == pieces.len();
        if shown < next || (shown == next && !last) {
            return piece_at(piece, shown.saturating_sub(seen));
        }
        if last {
            let local = shown.saturating_sub(seen).min(piece.text.len());
            return piece_at(piece, local);
        }
        seen = next;
    }
    0
}

fn piece_at(piece: &Piece, local: usize) -> usize {
    if !piece.literal {
        return piece.origin;
    }
    let local = floor_boundary(&piece.text, local.min(piece.text.len()));
    piece.origin.saturating_add(local)
}

fn paint_spans(pieces: &[Piece]) -> (Vec<iced::advanced::text::Span<'static, (), Font>>, f32) {
    let mut size = BODY;
    let mut spans = Vec::new();
    for piece in pieces {
        size = size.max(piece.look.size);
        let mut span = iced::widget::span(piece.text.clone())
            .size(piece.look.size)
            .font(piece.look.font);
        if let Some(color) = piece.look.color {
            span = span.color(color);
        }
        if piece.look.strike {
            span = span.strikethrough(true);
        }
        if piece.look.underline {
            span = span.underline(true);
        }
        if let Some(paint) = piece.look.background {
            span = span.background(Background::Color(paint));
        }
        spans.push(span);
    }
    (spans, size)
}

fn shade(pieces: Vec<Piece>, selected: Option<(usize, usize)>, paint: Color) -> Vec<Piece> {
    let Some((from, to)) = selected else {
        return pieces;
    };
    if from >= to {
        return pieces;
    }
    let mut out = Vec::new();
    for piece in pieces {
        let start = piece.covers.start;
        let end = piece.covers.end;
        if end <= from || start >= to || start >= end {
            out.push(piece);
            continue;
        }
        if !piece.literal {
            let mut piece = piece;
            piece.look.background = Some(paint);
            out.push(piece);
            continue;
        }
        let local_from = from.max(start).saturating_sub(start);
        let local_to = to.min(end).saturating_sub(start);
        let text = piece.text;
        let origin = piece.origin;
        let look = piece.look;
        if local_from > 0
            && let Some(head) = text.get(..local_from)
            && !head.is_empty()
        {
            out.push(Piece {
                covers: origin..origin.saturating_add(head.len()),
                origin,
                text: head.to_owned(),
                literal: true,
                look,
            });
        }
        if let Some(mid) = text.get(local_from..local_to)
            && !mid.is_empty()
        {
            let mut marked = look;
            marked.background = Some(paint);
            let at = origin.saturating_add(local_from);
            out.push(Piece {
                covers: at..at.saturating_add(mid.len()),
                origin: at,
                text: mid.to_owned(),
                literal: true,
                look: marked,
            });
        }
        if local_to < text.len()
            && let Some(tail) = text.get(local_to..)
            && !tail.is_empty()
        {
            let at = origin.saturating_add(local_to);
            out.push(Piece {
                covers: at..at.saturating_add(tail.len()),
                origin: at,
                text: tail.to_owned(),
                literal: true,
                look,
            });
        }
    }
    out
}

fn rich_of<'a>(pieces: &[Piece], fill: bool, listen: bool, dragging: bool) -> Element<'a, Message> {
    if pieces.is_empty() {
        return text("").size(BODY).into();
    }
    let (spans, size) = paint_spans(pieces);
    let mut rich = iced::widget::rich_text(spans)
        .size(size)
        .line_height(LEADING)
        .font(sans());
    if fill {
        rich = rich
            .width(Length::Fill)
            .wrapping(iced::advanced::text::Wrapping::Word);
    } else {
        rich = rich.wrapping(iced::advanced::text::Wrapping::None);
    }
    PageText {
        pieces: pieces.to_vec(),
        fill,
        listen,
        dragging,
        child: rich.into(),
    }
    .into()
}

/// Drawn text that takes a click at the letter under the pointer.
struct PageText<'a> {
    pieces: Vec<Piece>,
    fill: bool,
    listen: bool,
    dragging: bool,
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for PageText<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(Size::ZERO);
        };
        self.child.as_widget_mut().layout(child, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        if !self.listen {
            return;
        }
        let bounds = layout.bounds();
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                if cursor
                    .position()
                    .is_some_and(|point| bounds.contains(point)) =>
            {
                let point = cursor.position().unwrap_or_default();
                let byte = hit_byte(
                    &self.pieces,
                    self.fill,
                    bounds.size(),
                    point_in(bounds, point),
                );
                shell.publish(Message::SelectFrom {
                    byte,
                    x: point.x,
                    y: point.y,
                });
                shell.capture_event();
                return;
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                if self.dragging && landed(cursor).is_some_and(|point| in_band(bounds, point)) =>
            {
                let Some(point) = landed(cursor) else {
                    return;
                };
                let byte = hit_byte(
                    &self.pieces,
                    self.fill,
                    bounds.size(),
                    point_in(bounds, point),
                );
                shell.publish(Message::ExtendTo {
                    byte,
                    x: point.x,
                    y: point.y,
                });
                return;
            }
            _ => {}
        }
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first() else {
            return;
        };
        self.child
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        _tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
        _renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            iced::advanced::mouse::Interaction::Text
        } else {
            iced::advanced::mouse::Interaction::None
        }
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child
            .as_widget_mut()
            .operate(child, layout, renderer, operation);
    }
}

impl<'a> From<PageText<'a>> for Element<'a, Message> {
    fn from(text: PageText<'a>) -> Self {
        Element::new(text)
    }
}

fn hit_byte(pieces: &[Piece], fill: bool, bounds: Size, point: iced::Point) -> usize {
    let (spans, size) = paint_spans(pieces);
    let width = if fill {
        bounds.width.max(1.0)
    } else {
        bounds.width.max(1.0) + 32.0
    };
    let paragraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph::with_spans(
        iced::advanced::text::Text {
            content: spans.as_slice(),
            bounds: Size::new(width, bounds.height.max(1.0)),
            size: size.into(),
            line_height: iced::advanced::text::LineHeight::Relative(LEADING),
            font: sans(),
            align_x: iced::advanced::text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping: if fill {
                iced::advanced::text::Wrapping::Word
            } else {
                iced::advanced::text::Wrapping::None
            },
        },
    );
    let shown = paragraph.hit_test(point).map_or_else(
        || {
            pieces
                .iter()
                .map(|piece| piece.text.len())
                .fold(0_usize, usize::saturating_add)
        },
        iced::advanced::text::Hit::cursor,
    );
    place_of(pieces, shown)
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::fn_params_excessive_bools
)]
fn active_line<'a>(
    body: &'a NoteBody,
    line: &str,
    index: usize,
    base: usize,
    win: Range<usize>,
    show: Range<usize>,
    carry: Carry,
    at_start: bool,
    dark: bool,
    placeholder: bool,
    selected: Option<(usize, usize)>,
    paint: Color,
    dragging: bool,
    measure: f32,
) -> Element<'a, Message> {
    let cached = body.cache.get(index);
    let prefix_from = show.start.min(win.start);
    let prefix_to = win.start.min(show.end);
    let prefix = if prefix_from < prefix_to {
        shade(
            line_pieces(
                cached,
                line,
                base,
                prefix_from..prefix_to,
                carry,
                at_start,
                dark,
            ),
            selected,
            paint,
        )
    } else {
        Vec::new()
    };
    let suffix_from = win.end.max(show.start);
    let suffix_to = show.end.max(suffix_from).min(line.len());
    let suffix = if suffix_from < suffix_to {
        shade(
            line_pieces(
                cached,
                line,
                base,
                suffix_from..suffix_to,
                carry,
                at_start,
                dark,
            ),
            selected,
            paint,
        )
    } else {
        Vec::new()
    };
    let prefix = if line_dim(index) {
        dim_pieces(prefix, dark)
    } else {
        prefix
    };
    let suffix = if line_dim(index) {
        dim_pieces(suffix, dark)
    } else {
        suffix
    };
    let sample = line.get(win.start..win.end).unwrap_or("").to_owned();
    let probe = if win.start < win.end {
        win.end.saturating_sub(1)
    } else {
        win.start.min(line.len())
    };
    let mut look = look_at(line, probe, carry, at_start, dark);
    if win.start == win.end {
        // The caret is in the gap after a settled word. Keep the line size, but
        // do not pick up a marker, link, or strike color for an empty slot.
        look.color = None;
    }
    let natural = if sample.is_empty() {
        0.0
    } else {
        text_width(&sample, look.font, look.size)
    };
    let wrap_editor = natural > measure;
    let mut editor = text_editor::TextEditor::new(&body.live)
        .id(iced::widget::Id::new("note-body"))
        .padding(Padding::ZERO)
        .size(look.size)
        .line_height(LEADING)
        .font(look.font)
        .wrapping(if wrap_editor {
            iced::advanced::text::Wrapping::Word
        } else {
            iced::advanced::text::Wrapping::None
        })
        .on_action(Message::BodyEdited)
        .key_binding(binding)
        .style(move |theme, status| {
            let mut style = editor_style(theme, status);
            if let Some(color) = look.color {
                style.value = color;
            }
            style
        })
        .highlight_with::<Live>((), mark_format);
    if placeholder {
        editor = editor.placeholder("Write. **bold** starts when you type the marks.");
    }
    let edited: Element<'a, Message> = if placeholder {
        editor.into()
    } else {
        MarkedWord {
            sample: sample.clone(),
            font: look.font,
            size: look.size,
            span: (!wrap_editor)
                .then(|| {
                    covered(
                        selected,
                        base.saturating_add(win.start),
                        base.saturating_add(win.end),
                    )
                })
                .flatten(),
            paint,
            child: Fitted {
                sample: sample.clone(),
                font: look.font,
                size: look.size,
                limit: wrap_editor.then_some(measure),
                child: editor.into(),
            }
            .into(),
        }
        .into()
    };
    let gap = space().width(Length::Fill).height(look.size * LEADING);
    let inner = row![
        rich_of(&prefix, false, false, false),
        edited,
        rich_of(&suffix, false, false, false),
        gap
    ]
    .spacing(0)
    .align_y(iced::alignment::Vertical::Top)
    .width(Length::Fill);
    let hit = ActiveHit {
        prefix,
        suffix,
        base,
        window: win.clone(),
        line_len: line.len(),
        sample,
        font: look.font,
        size: look.size,
        dragging,
        child: inner.into(),
    }
    .into();
    let lead = lead(line);
    let quoted = quote_end(line).is_some() && (show.start > 0 || win.start >= lead.skip);
    let body_row = if quoted {
        quote_row(hit, look.size, dark)
    } else {
        hit
    };
    let show_box = lead.box_at.is_some() && show.start == 0;
    if show_box {
        let done = lead.box_at.is_some_and(|(_, done)| done);
        row![
            checkbox(done).on_toggle(move |done| Message::SetOpenTask { line: index, done }),
            body_row,
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .into()
    } else {
        body_row
    }
}

#[allow(clippy::too_many_arguments)]
fn settled_line(
    line: &str,
    index: usize,
    base: usize,
    carry: Carry,
    at_start: bool,
    dark: bool,
    selected: Option<(usize, usize)>,
    paint: Color,
    dragging: bool,
    cached: Option<&LineCache>,
    show: Range<usize>,
) -> Element<'static, Message> {
    if line.is_empty() {
        return press_at(
            space().width(Length::Fill).height(line_px(line)).into(),
            base,
            dragging,
        );
    }
    if is_rule(line) {
        return press_at(
            container(rule::horizontal(1))
                .width(Length::Fill)
                .padding(Padding {
                    top: 10.0,
                    right: 0.0,
                    bottom: 10.0,
                    left: 0.0,
                })
                .into(),
            base.saturating_add(line.len()),
            dragging,
        );
    }
    let shown = shade(
        line_pieces(cached, line, base, show.clone(), carry, at_start, dark),
        selected,
        paint,
    );
    let shown = if line_dim(index) {
        dim_pieces(shown, dark)
    } else {
        shown
    };
    let size = shown
        .iter()
        .fold(BODY, |size, piece| size.max(piece.look.size));
    let text = rich_of(&shown, true, true, dragging);
    let lead = lead(line);
    let quoted = quote_end(line).is_some();
    let body = if quoted {
        quote_row(text, size, dark)
    } else {
        text
    };
    if show.start == 0
        && let Some((_, done)) = lead.box_at
    {
        row![
            checkbox(done).on_toggle(move |done| Message::SetOpenTask { line: index, done }),
            body,
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .into()
    } else {
        body
    }
}

fn line_pieces(
    cached: Option<&LineCache>,
    line: &str,
    base: usize,
    show: Range<usize>,
    carry: Carry,
    at_start: bool,
    dark: bool,
) -> Vec<Piece> {
    match cached {
        Some(row) => pieces_ready(line, base, show, &row.spans, &row.unclosed, dark),
        None => pieces_between(line, base, show, carry, at_start, dark),
    }
}

const LINE_GAP: f32 = 2.0;

fn landed(cursor: iced::advanced::mouse::Cursor) -> Option<iced::Point> {
    cursor.land().position()
}

fn in_band(bounds: iced::Rectangle, point: iced::Point) -> bool {
    point.y >= bounds.y && point.y < bounds.y + bounds.height + LINE_GAP
}

fn point_in(bounds: iced::Rectangle, point: iced::Point) -> iced::Point {
    iced::Point::new(
        (point.x - bounds.x).clamp(0.0, (bounds.width - 0.01).max(0.0)),
        (point.y - bounds.y).clamp(0.0, (bounds.height - 0.01).max(0.0)),
    )
}

/// The part of `start..end` that the selection covers, as offsets into that slice.
fn covered(selected: Option<(usize, usize)>, start: usize, end: usize) -> Option<(usize, usize)> {
    let (from, to) = selected?;
    let lo = from.max(start);
    let hi = to.min(end);
    (lo < hi).then_some((lo.saturating_sub(start), hi.saturating_sub(start)))
}

fn press_at(content: Element<'_, Message>, byte: usize, dragging: bool) -> Element<'_, Message> {
    ByteBand {
        byte,
        dragging,
        child: content,
    }
    .into()
}

struct ByteBand<'a> {
    byte: usize,
    dragging: bool,
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for ByteBand<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(Size::ZERO);
        };
        self.child.as_widget_mut().layout(child, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                if cursor
                    .position()
                    .is_some_and(|point| bounds.contains(point)) =>
            {
                let point = cursor.position().unwrap_or_default();
                shell.publish(Message::SelectFrom {
                    byte: self.byte,
                    x: point.x,
                    y: point.y,
                });
                shell.capture_event();
                return;
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                if self.dragging && landed(cursor).is_some_and(|point| in_band(bounds, point)) =>
            {
                let Some(point) = landed(cursor) else {
                    return;
                };
                shell.publish(Message::ExtendTo {
                    byte: self.byte,
                    x: point.x,
                    y: point.y,
                });
                return;
            }
            _ => {}
        }
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first() else {
            return;
        };
        self.child
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let Some(child) = tree.children.first() else {
            return iced::advanced::mouse::Interaction::None;
        };
        self.child
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child
            .as_widget_mut()
            .operate(child, layout, renderer, operation);
    }
}

impl<'a> From<ByteBand<'a>> for Element<'a, Message> {
    fn from(band: ByteBand<'a>) -> Self {
        Element::new(band)
    }
}

struct MarkedWord<'a> {
    sample: String,
    font: Font,
    size: f32,
    span: Option<(usize, usize)>,
    paint: Color,
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for MarkedWord<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(Size::ZERO);
        };
        self.child.as_widget_mut().layout(child, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        if let Some((from, to)) = self.span {
            let bounds = layout.bounds();
            let x0 = text_width(self.sample.get(..from).unwrap_or(""), self.font, self.size);
            let x1 = text_width(self.sample.get(..to).unwrap_or(""), self.font, self.size);
            let width = (x1 - x0).max(0.0);
            if width > 0.0 {
                renderer.fill_quad(
                    iced::advanced::renderer::Quad {
                        bounds: iced::Rectangle {
                            x: bounds.x + x0,
                            y: bounds.y,
                            width,
                            height: bounds.height,
                        },
                        ..iced::advanced::renderer::Quad::default()
                    },
                    Background::Color(self.paint),
                );
            }
        }
        let Some(child) = tree.children.first() else {
            return;
        };
        self.child
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let Some(child) = tree.children.first() else {
            return iced::advanced::mouse::Interaction::None;
        };
        self.child
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child
            .as_widget_mut()
            .operate(child, layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let child = tree.children.first_mut()?;
        self.child
            .as_widget_mut()
            .overlay(child, layout, renderer, viewport, translation)
    }
}

impl<'a> From<MarkedWord<'a>> for Element<'a, Message> {
    fn from(word: MarkedWord<'a>) -> Self {
        Element::new(word)
    }
}

fn row_offset(rows: usize) -> f32 {
    f32::from(u16::try_from(rows).unwrap_or(u16::MAX))
}

fn line_face(line: &str) -> (Font, f32) {
    if let Some((_, level)) = heading_end(line) {
        let font = if level <= 2 { serif() } else { sans_bold() };
        return (font, heading_px(level));
    }
    (sans(), BODY)
}

/// Room left for the words after a quote bar or a checkbox.
fn content_measure(line: &str, measure: f32) -> f32 {
    let mut chrome = 0.0;
    if quote_end(line).is_some() {
        chrome += 11.0;
    }
    if lead(line).box_at.is_some() {
        chrome += 26.0;
    }
    (measure - chrome).max(48.0)
}

fn row_has_editor(range: &Range<usize>, win: &Range<usize>, next_start: Option<usize>) -> bool {
    if win.end > win.start {
        return range.start < win.end && win.start < range.end;
    }
    let at = win.start;
    if range.start <= at && at < range.end {
        return true;
    }
    match next_start {
        Some(start) => range.end <= at && at < start,
        None => range.end <= at && range.start <= at,
    }
}

fn block_rows(
    line: &str,
    ranges: &[Range<usize>],
    atomic: &Range<usize>,
    measure: f32,
    font: Font,
    size: f32,
) -> usize {
    let mut rows = ranges.len().max(1);
    if atomic.end > atomic.start
        && ranges
            .iter()
            .any(|range| range.start == atomic.start && range.end == atomic.end)
    {
        let sample = line.get(atomic.clone()).unwrap_or("");
        let sub = wrap_ranges(sample, measure, font, size, 0..0, font, size);
        if sub.len() > 1 {
            rows = rows.saturating_sub(1).saturating_add(sub.len());
        }
    }
    rows
}

fn rows_before(
    line: &str,
    local: usize,
    ranges: &[Range<usize>],
    atomic: &Range<usize>,
    measure: f32,
    font: Font,
    size: f32,
) -> usize {
    let mut count = 0usize;
    for (index, range) in ranges.iter().enumerate() {
        let next = ranges.get(index.saturating_add(1)).map(|next| next.start);
        let inside_atomic = atomic.end > atomic.start
            && range.start < atomic.end
            && atomic.start < range.end
            && local >= atomic.start
            && local <= atomic.end;
        if inside_atomic || row_has_editor(range, &(local..local), next) {
            if atomic.end > atomic.start
                && range.start == atomic.start
                && range.end == atomic.end
                && local >= atomic.start
            {
                let sample = line.get(atomic.clone()).unwrap_or("");
                let sub = wrap_ranges(sample, measure, font, size, 0..0, font, size);
                if sub.len() > 1 {
                    let inner = local.saturating_sub(atomic.start).min(sample.len());
                    count = count.saturating_add(subrow_index(&sub, inner));
                }
            }
            break;
        }
        count = count.saturating_add(1);
    }
    count
}

fn subrow_index(ranges: &[Range<usize>], local: usize) -> usize {
    for (index, range) in ranges.iter().enumerate() {
        let last = index.saturating_add(1) == ranges.len();
        if local < range.end || (last && local <= range.end) {
            return index;
        }
    }
    0
}

fn wrap_ranges(
    line: &str,
    measure: f32,
    font: Font,
    size: f32,
    atomic: Range<usize>,
    atomic_font: Font,
    atomic_size: f32,
) -> Vec<Range<usize>> {
    if line.is_empty() {
        return one_row(0..0);
    }
    let measure = measure.max(48.0);
    // A glyph is not wider than the em. Skip measurement when the line cannot reach the edge.
    let glyphs = u16::try_from(line.chars().count()).unwrap_or(u16::MAX);
    if f32::from(glyphs) * size.max(1.0) <= measure {
        return one_row(0..line.len());
    }
    let key = memo::key(|hasher| {
        use std::hash::Hash;
        line.hash(hasher);
        measure.to_bits().hash(hasher);
        atomic.start.hash(hasher);
        atomic.end.hash(hasher);
        memo::hash_font(hasher, font, size);
        memo::hash_font(hasher, atomic_font, atomic_size);
    });
    memo::wrap(key, || {
        wrap_by(line, measure, atomic.clone(), |slice, exact| {
            if exact {
                text_width(slice, atomic_font, atomic_size)
            } else {
                text_width(slice, font, size)
            }
        })
    })
}

#[allow(clippy::vec_init_then_push)]
fn one_row(range: Range<usize>) -> Vec<Range<usize>> {
    let mut rows = Vec::with_capacity(1);
    rows.push(range);
    rows
}

fn wrap_by(
    line: &str,
    measure: f32,
    atomic: Range<usize>,
    width_of: impl Fn(&str, bool) -> f32,
) -> Vec<Range<usize>> {
    let atomic = if atomic.start < atomic.end && atomic.end <= line.len() {
        atomic
    } else {
        0..0
    };
    let measure = measure.max(1.0);
    let mut rows = Vec::new();
    let mut start = 0usize;
    while start < line.len() && rows.len() < line.len().saturating_add(1) {
        if start > 0 {
            while start < line.len() && is_space_at(line, start) {
                start = step_char(line, start);
            }
        }
        if start >= line.len() {
            break;
        }
        let row_start = start;
        let mut cursor = start;
        let mut row_end = start;
        let mut pushed_split = false;
        while cursor < line.len() {
            if cursor > row_start && is_space_at(line, cursor) {
                cursor = skip_spaces(line, cursor, &atomic);
                continue;
            }
            let word_end = token_end(line, cursor, &atomic);
            if word_end <= cursor {
                break;
            }
            let exact = cursor == atomic.start && word_end == atomic.end && row_start == cursor;
            let width = width_of(line.get(row_start..word_end).unwrap_or(""), exact);
            if row_end > row_start && width > measure {
                break;
            }
            if row_end == row_start && width > measure {
                if exact {
                    rows.push(cursor..word_end);
                    start = word_end;
                } else {
                    let cut = split_to_fit(line, cursor, word_end, measure, &width_of);
                    rows.push(cursor..cut);
                    start = cut;
                }
                pushed_split = true;
                break;
            }
            row_end = word_end;
            cursor = word_end;
        }
        if pushed_split {
            continue;
        }
        if row_end <= row_start {
            let cut = step_char(line, row_start).min(line.len());
            if cut <= row_start {
                break;
            }
            rows.push(row_start..cut);
            start = cut;
            continue;
        }
        rows.push(row_start..row_end);
        start = row_end;
    }
    if rows.is_empty() {
        rows.push(0..line.len());
    }
    rows
}

fn split_to_fit(
    line: &str,
    start: usize,
    end: usize,
    measure: f32,
    width_of: &impl Fn(&str, bool) -> f32,
) -> usize {
    let mut fit = start;
    let mut cursor = start;
    while cursor < end {
        let next = step_char(line, cursor).min(end);
        if next <= cursor {
            break;
        }
        let width = width_of(line.get(start..next).unwrap_or(""), false);
        if width > measure && fit > start {
            return fit;
        }
        fit = next;
        cursor = next;
        if width > measure {
            break;
        }
    }
    if fit == start {
        step_char(line, start).min(end)
    } else {
        fit
    }
}

fn token_end(line: &str, index: usize, atomic: &Range<usize>) -> usize {
    if atomic.start < atomic.end && index >= atomic.start && index < atomic.end {
        return atomic.end;
    }
    let mut end = index;
    while end < line.len() && !is_space_at(line, end) {
        if atomic.start < atomic.end && end == atomic.start {
            break;
        }
        let next = step_char(line, end);
        if next <= end {
            break;
        }
        end = next;
    }
    end
}

fn skip_spaces(line: &str, mut index: usize, atomic: &Range<usize>) -> usize {
    while index < line.len() && is_space_at(line, index) {
        if atomic.start < atomic.end && index == atomic.start {
            break;
        }
        let next = step_char(line, index);
        if next <= index {
            break;
        }
        index = next;
    }
    index
}

fn is_space_at(line: &str, index: usize) -> bool {
    line.get(index..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(char::is_whitespace)
}

fn step_char(line: &str, index: usize) -> usize {
    index.saturating_add(
        line.get(index..)
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8),
    )
}

/// Widths and wraps are pure functions of their arguments, and shaping a paragraph to measure it
/// is the slowest thing the page does. The page asks again for every line on every update, and
/// for most lines the answer has not changed. These keep the answers, so a keystroke pays for
/// the line that changed and not for the whole page. An entry is looked up by a 64-bit hash of
/// everything it depends on. The tables are cleared when they grow past a bound.
mod memo {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::ops::Range;

    use iced::Font;

    const WIDTHS: usize = 20_000;
    const WRAPS: usize = 4_000;

    thread_local! {
        static WIDTH: RefCell<HashMap<u64, f32>> = RefCell::new(HashMap::new());
        static WRAP: RefCell<HashMap<u64, Vec<Range<usize>>>> = RefCell::new(HashMap::new());
    }

    pub(super) fn key(parts: impl FnOnce(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        parts(&mut hasher);
        hasher.finish()
    }

    pub(super) fn hash_font(hasher: &mut impl Hasher, font: Font, size: f32) {
        font.hash(hasher);
        size.to_bits().hash(hasher);
    }

    pub(super) fn width(key: u64, measure: impl FnOnce() -> f32) -> f32 {
        if let Some(width) = WIDTH.with(|table| table.borrow().get(&key).copied()) {
            return width;
        }
        let width = measure();
        WIDTH.with(|table| {
            let mut table = table.borrow_mut();
            if table.len() >= WIDTHS {
                table.clear();
            }
            table.insert(key, width);
        });
        width
    }

    pub(super) fn wrap(key: u64, rows: impl FnOnce() -> Vec<Range<usize>>) -> Vec<Range<usize>> {
        if let Some(rows) = WRAP.with(|table| table.borrow().get(&key).cloned()) {
            return rows;
        }
        let rows = rows();
        WRAP.with(|table| {
            let mut table = table.borrow_mut();
            if table.len() >= WRAPS {
                table.clear();
            }
            table.insert(key, rows.clone());
        });
        rows
    }
}

fn text_width(sample: &str, font: Font, size: f32) -> f32 {
    if sample.is_empty() {
        return 0.0;
    }
    let key = memo::key(|hasher| {
        use std::hash::Hash;
        sample.hash(hasher);
        memo::hash_font(hasher, font, size);
    });
    memo::width(key, || {
        let paragraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph::with_text(
            iced::advanced::text::Text {
                content: sample,
                bounds: Size::new(10_000.0, 10_000.0),
                size: size.into(),
                line_height: iced::advanced::text::LineHeight::Relative(LEADING),
                font,
                align_x: iced::advanced::text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::None,
            },
        );
        paragraph.min_width()
    })
}

fn selection_color(dark: bool) -> Color {
    let accent = accent(dark);
    let bg = if dark {
        Color::from_rgb8(0x1C, 0x19, 0x17)
    } else {
        Color::from_rgb8(0xED, 0xEA, 0xDE)
    };
    Color {
        r: accent.r + (bg.r - accent.r) * 0.35,
        g: accent.g + (bg.g - accent.g) * 0.35,
        b: accent.b + (bg.b - accent.b) * 0.35,
        a: 1.0,
    }
}

fn quote_row(inner: Element<'_, Message>, size: f32, dark: bool) -> Element<'_, Message> {
    let paint = accent(dark);
    row![
        container(space().width(3).height(size * LEADING)).style(move |_theme| container::Style {
            background: Some(Background::Color(paint)),
            ..container::Style::default()
        }),
        inner,
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

struct ActiveHit<'a> {
    prefix: Vec<Piece>,
    suffix: Vec<Piece>,
    base: usize,
    window: Range<usize>,
    line_len: usize,
    sample: String,
    font: Font,
    size: f32,
    dragging: bool,
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for ActiveHit<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(Size::ZERO);
        };
        self.child.as_widget_mut().layout(child, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let mouse = matches!(event, iced::Event::Mouse(_) | iced::Event::Touch(_));
        let bounds = layout.bounds();
        let inside = cursor
            .position()
            .is_some_and(|point| bounds.contains(point));
        // A press on the title never reaches this editor, so its caret stayed
        // on and the next key went to both lines. Hand that press through.
        let press_outside = !inside
            && matches!(
                event,
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                    | iced::Event::Touch(iced::touch::Event::FingerPressed { .. })
            );
        if (!mouse || press_outside)
            && let Some(child) = tree.children.first_mut()
        {
            self.child.as_widget_mut().update(
                child, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                if cursor
                    .position()
                    .is_some_and(|point| bounds.contains(point)) =>
            {
                let point = cursor.position().unwrap_or_default();
                let Some(byte) = self.byte_at(layout, point) else {
                    return;
                };
                shell.publish(Message::SelectFrom {
                    byte,
                    x: point.x,
                    y: point.y,
                });
                shell.capture_event();
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                if self.dragging && landed(cursor).is_some_and(|point| in_band(bounds, point)) =>
            {
                let Some(point) = landed(cursor) else {
                    return;
                };
                let clamped = iced::Point::new(
                    point.x.clamp(bounds.x, bounds.x + bounds.width.max(1.0)),
                    point
                        .y
                        .clamp(bounds.y, bounds.y + (bounds.height - 0.5).max(0.0)),
                );
                if let Some(byte) = self.byte_at(layout, clamped) {
                    shell.publish(Message::ExtendTo {
                        byte,
                        x: point.x,
                        y: point.y,
                    });
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first() else {
            return;
        };
        self.child
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let Some(child) = tree.children.first() else {
            return iced::advanced::mouse::Interaction::None;
        };
        self.child
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child
            .as_widget_mut()
            .operate(child, layout, renderer, operation);
    }
}

impl ActiveHit<'_> {
    fn byte_at(&self, layout: iced::advanced::Layout<'_>, absolute: iced::Point) -> Option<usize> {
        let index = region_at(layout, absolute)?;
        let child = layout.children().nth(index)?;
        let bounds = child.bounds();
        let point = iced::Point::new(absolute.x - bounds.x, absolute.y - bounds.y);
        Some(match index {
            0 => hit_byte(&self.prefix, false, bounds.size(), point),
            1 => {
                let local = hit_sample(&self.sample, self.font, self.size, bounds.size(), point);
                self.base
                    .saturating_add(self.window.start)
                    .saturating_add(local)
            }
            2 => hit_byte(&self.suffix, false, bounds.size(), point),
            _ => self.base.saturating_add(self.line_len),
        })
    }
}

impl<'a> From<ActiveHit<'a>> for Element<'a, Message> {
    fn from(hit: ActiveHit<'a>) -> Self {
        Element::new(hit)
    }
}

/// One place hears the mouse button lift, so a drag ends even on another line.
struct Release<'a> {
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for Release<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(Size::ZERO);
        };
        self.child.as_widget_mut().layout(child, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) =
            event
        {
            shell.publish(Message::SelectStop);
        }
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first() else {
            return;
        };
        self.child
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let Some(child) = tree.children.first() else {
            return iced::advanced::mouse::Interaction::None;
        };
        self.child
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child
            .as_widget_mut()
            .operate(child, layout, renderer, operation);
    }
}

impl<'a> From<Release<'a>> for Element<'a, Message> {
    fn from(release: Release<'a>) -> Self {
        Element::new(release)
    }
}

fn region_at(layout: iced::advanced::Layout<'_>, absolute: iced::Point) -> Option<usize> {
    layout
        .children()
        .enumerate()
        .find(|(_, child)| child.bounds().contains(absolute))
        .map(|(index, _)| index)
}

fn hit_sample(sample: &str, font: Font, size: f32, bounds: Size, point: iced::Point) -> usize {
    if sample.is_empty() {
        return 0;
    }
    let natural = text_width(sample, font, size);
    let wrapping = if natural > bounds.width.max(1.0) {
        iced::advanced::text::Wrapping::Word
    } else {
        iced::advanced::text::Wrapping::None
    };
    let paragraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph::with_text(
        iced::advanced::text::Text {
            content: sample,
            bounds: Size::new(bounds.width.max(1.0), bounds.height.max(1.0)),
            size: size.into(),
            line_height: iced::advanced::text::LineHeight::Relative(LEADING),
            font,
            align_x: iced::advanced::text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping,
        },
    );
    paragraph
        .hit_test(point)
        .map_or(sample.len(), iced::advanced::text::Hit::cursor)
        .min(sample.len())
}

struct Fitted<'a> {
    sample: String,
    font: Font,
    size: f32,
    /// When set, a sample wider than this wraps instead of stretching the page.
    limit: Option<f32>,
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for Fitted<'_> {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let paragraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph::with_text(
            iced::advanced::text::Text {
                content: self.sample.as_str(),
                bounds: Size::new(10_000.0, 10_000.0),
                size: self.size.into(),
                line_height: iced::advanced::text::LineHeight::Relative(LEADING),
                font: self.font,
                align_x: iced::advanced::text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::None,
            },
        );
        let measured = paragraph.min_width();
        let natural = if self.sample.is_empty() {
            2.0
        } else {
            measured + 2.0
        };
        let width = match self.limit {
            Some(limit) if natural > limit => limit.max(48.0),
            _ => natural,
        };
        let limits = limits.width(Length::Fixed(width)).height(Length::Shrink);
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(Size::ZERO);
        };
        self.child.as_widget_mut().layout(child, renderer, &limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first() else {
            return;
        };
        self.child
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let Some(child) = tree.children.first() else {
            return iced::advanced::mouse::Interaction::None;
        };
        self.child
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.child
            .as_widget_mut()
            .operate(child, layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let child = tree.children.first_mut()?;
        self.child
            .as_widget_mut()
            .overlay(child, layout, renderer, viewport, translation)
    }
}

impl<'a> From<Fitted<'a>> for Element<'a, Message> {
    fn from(fitted: Fitted<'a>) -> Self {
        Element::new(fitted)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Carry, EM, Face, HIGH, Mark, STRIKE, STRONG, check_mark, code_mark, em, heading_mark,
        marker, math_mark, paint, set_task_line, strike_mark, strong, task_lines,
    };

    #[test]
    fn bold_starts_at_the_opening_marks() {
        let line = "See **bold";
        let (spans, carry) = paint(line, Carry::default());
        assert!(hit(&spans, line, "**", marker()));
        assert!(hit(&spans, line, "bold", strong()));
        assert_eq!(carry.len, 1);
    }

    #[test]
    fn the_closer_ends_the_bold() {
        let line = "**bold** tail";
        let (spans, carry) = paint(line, Carry::default());
        assert!(hit(&spans, line, "bold", strong()));
        assert!(!hit(&spans, line, " tail", strong()));
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn bold_continues_until_the_closer_on_the_next_line() {
        let (first, carry) = paint("**still", Carry::default());
        assert!(hit(&first, "**still", "still", strong()));
        let (second, carry) = paint("bold** end", carry);
        assert!(hit(&second, "bold** end", "bold", strong()));
        assert!(!hit(&second, "bold** end", " end", strong()));
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn a_blank_line_ends_an_unclosed_mark() {
        let (_, carry) = paint("**open", Carry::default());
        let (_, carry) = paint("", carry);
        let (spans, carry) = paint("plain", carry);
        assert!(spans.is_empty());
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn italic_code_and_a_heading() {
        let line = "a *em* and `code`";
        let (spans, _) = paint(line, Carry::default());
        assert!(hit(&spans, line, "em", em()));
        assert!(hit(&spans, line, "code", code_mark()));
        let heading = "## Title **now**";
        let (spans, carry) = paint(heading, Carry::default());
        assert!(hit(&spans, heading, "## ", marker()));
        assert!(hit(&spans, heading, "Title ", heading_mark(2)));
        assert!(hit(
            &spans,
            heading,
            "now",
            Mark {
                face: Face::Heading(2),
                flags: STRONG,
            }
        ));
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn a_fence_stays_code_until_the_closer() {
        let (_, carry) = paint("```", Carry::default());
        assert!(carry.fence);
        let (inside, carry) = paint("**not bold**", carry);
        assert!(hit(&inside, "**not bold**", "**not bold**", code_mark()));
        let (_, carry) = paint("```", carry);
        assert!(!carry.fence);
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn a_done_task_is_struck_and_the_box_is_marked() {
        let line = "- [x] milk";
        let (spans, _) = paint(line, Carry::default());
        assert!(hit(&spans, line, "[x]", check_mark(true)));
        assert!(hit(&spans, line, "milk", strike_mark()));
        let open = "- [ ] call";
        let (spans, _) = paint(open, Carry::default());
        assert!(hit(&spans, open, "[ ]", check_mark(false)));
        assert!(spans.iter().all(|(_, mark)| mark.flags & STRIKE == 0));
    }

    #[test]
    fn tasks_are_checkbox_lines_and_a_toggle_writes_the_box() {
        let body = "keep\n- milk\n- [ ] call\n- [x] done\n";
        let lines = task_lines(body);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line, 2);
        assert!(!lines[0].done);
        assert_eq!(lines[0].text, "call");
        assert!(lines[1].done);
        let next = set_task_line(body, 2, true).unwrap();
        assert_eq!(next, "keep\n- milk\n- [x] call\n- [x] done\n");
        assert!(set_task_line(&next, 2, true).is_none());
        assert!(set_task_line(body, 1, true).is_none());
    }

    #[test]
    fn a_task_line_can_be_renamed_or_removed() {
        let body = "keep\n- [ ] one\n- [x] two\n";
        assert_eq!(
            super::remove_task_line(body, 1).as_deref(),
            Some("keep\n- [x] two\n")
        );
        assert!(super::remove_task_line(body, 0).is_none());
        assert_eq!(
            super::rename_task_line(body, 2, "bread").as_deref(),
            Some("keep\n- [ ] one\n- [x] bread\n")
        );
        assert!(super::rename_task_line(body, 2, "two").is_none());
        assert!(super::rename_task_line(body, 2, "  ").is_none());
        assert!(super::rename_task_line(body, 0, "nope").is_none());
    }

    #[test]
    fn a_task_moves_within_its_note() {
        let body = "keep\n- [ ] one\nmiddle\n- [ ] two\n";
        assert_eq!(
            super::move_task_line(body, 3, true).as_deref(),
            Some("keep\n- [ ] two\nmiddle\n- [ ] one\n")
        );
        assert!(super::move_task_line(body, 1, true).is_none());
        assert!(super::move_task_line(body, 3, false).is_none());
    }

    #[test]
    fn the_words_of_a_task_leave_out_when_it_is_due_and_what_it_cites() {
        assert_eq!(super::task_words("call 2026-11-20"), "call");
        assert_eq!(super::task_words("call tomorrow"), "call tomorrow");
        assert_eq!(super::task_words("call 2026-11-20 14:30"), "call");
        assert_eq!(
            super::task_words("water 2026-11-20 08:00 repeat:weekly"),
            "water"
        );
        assert_eq!(super::task_words("call [[abcd|Clinic]] 2026-11-20"), "call");
        assert_eq!(
            super::task_cites("call [[abcd|Clinic]]"),
            vec![("abcd".to_owned(), "Clinic".to_owned())]
        );
        assert_eq!(
            super::cite_marks("call [[abcd|Clinic]]"),
            vec!["[[abcd|Clinic]]".to_owned()]
        );
    }

    #[test]
    fn a_pause_names_a_broken_mark_and_a_misspelling() {
        assert_eq!(
            super::broken_mark("**still"),
            Some("Bold is still open.".to_owned())
        );
        assert!(super::broken_mark("**done**").is_none());
        assert_eq!(
            super::broken_mark("see [[Daily"),
            Some("A note link is still open.".to_owned())
        );
        assert!(super::broken_mark("see [[Daily|today]]").is_none());
        let words = super::misspellings("the clinic asdfghjk");
        assert!(words.contains("asdfghjk"));
        assert!(!words.contains("the"));
    }

    /// A small deterministic generator, so a failure can be reproduced from its seed.
    struct Dice(u64);

    impl Dice {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(bound.max(1)).unwrap_or(1)).unwrap_or(0)
        }
    }

    const PIECES: [&str; 26] = [
        "**", "*", "_", "__", "==", "~~", "`", "\n", "\n\n", "word ", "and ", " ", "# ", "- ",
        "- [ ] ", "> ", "```\n", "~~~\n", "[[", "]]", "$$\n", "---\n", "    ", "1. ", "|", "é ",
    ];

    fn random_text(dice: &mut Dice, pieces: usize) -> String {
        let mut text = String::new();
        for _ in 0..pieces {
            text.push_str(PIECES[dice.below(PIECES.len())]);
        }
        text
    }

    fn boundary(text: &str, mut at: usize) -> usize {
        at = at.min(text.len());
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    #[test]
    fn an_edit_re_parses_only_what_it_changed_and_gets_the_same_rows() {
        let mut dice = Dice(0x9E37_79B9_7F4A_7C15);
        for trial in 0..4000 {
            let size = 1 + dice.below(40);
            let old = random_text(&mut dice, size);
            let cache = super::cache_lines(&old);
            // One edit, as typing or a paste makes it: replace a stretch with something else.
            let from = boundary(&old, dice.below(old.len() + 1));
            let to = boundary(&old, from + dice.below(12));
            let mut new = String::new();
            new.push_str(&old[..from]);
            for _ in 0..dice.below(4) {
                new.push_str(PIECES[dice.below(PIECES.len())]);
            }
            new.push_str(&old[to..]);
            let updated = super::update_lines(&old, cache, &new);
            let fresh = super::cache_lines(&new);
            assert_eq!(
                updated, fresh,
                "trial {trial}: {old:?} became {new:?} and the rows differ"
            );
        }
    }

    #[test]
    fn a_chain_of_edits_never_drifts_from_a_fresh_parse() {
        let mut dice = Dice(0xD1B5_4A32_D192_ED03);
        for trial in 0..200 {
            let mut text = random_text(&mut dice, 30);
            let mut cache = super::cache_lines(&text);
            for step in 0..60 {
                let from = boundary(&text, dice.below(text.len() + 1));
                let to = boundary(&text, from + dice.below(4));
                let mut next = String::new();
                next.push_str(&text[..from]);
                if dice.below(3) > 0 {
                    next.push_str(PIECES[dice.below(PIECES.len())]);
                }
                next.push_str(&text[to..]);
                cache = super::update_lines(&text, cache, &next);
                text = next;
                assert_eq!(
                    cache,
                    super::cache_lines(&text),
                    "trial {trial} step {step}: {text:?}"
                );
            }
        }
    }

    fn apply(text: &str, from: usize, to: usize, kind: super::FormatKind) -> String {
        let cursor = super::text_editor::Cursor {
            position: super::byte_pos(text, to),
            selection: (from != to).then(|| super::byte_pos(text, from)),
        };
        let change = super::format_change(text, cursor, kind);
        let a = super::pos_byte(text, change.anchor);
        let b = super::pos_byte(text, change.end);
        format!("{}{}{}", &text[..a], change.insert, &text[b..])
    }

    #[test]
    fn bold_and_italic_toggle_one_layer_at_a_time() {
        use super::FormatKind::{Bold, Italic};
        // Bold and italic together: taking one off leaves the other.
        assert_eq!(apply("***both***", 5, 5, Bold), "*both*");
        assert_eq!(apply("***both***", 5, 5, Italic), "**both**");
        assert_eq!(apply("***both***", 3, 7, Bold), "*both*");
        assert_eq!(apply("***both***", 3, 7, Italic), "**both**");
        // Adding the second layer, and taking it off again, gets back to where it started.
        assert_eq!(apply("**bold**", 4, 4, Italic), "***bold***");
        assert_eq!(apply("*em*", 2, 2, Bold), "***em***");
        let both = apply("**bold**", 4, 4, Italic);
        assert_eq!(apply(&both, 5, 5, Italic), "**bold**");
        // The plain cases still work.
        assert_eq!(apply("hello world", 2, 2, Bold), "**hello** world");
        assert_eq!(apply("**hello** world", 4, 4, Bold), "hello world");
        assert_eq!(apply("*one* two", 1, 1, Italic), "one two");
        assert_eq!(apply("a **b c** d", 5, 5, Bold), "a b c d");
    }

    #[test]
    fn the_underscore_spelling_is_taken_off_too() {
        use super::FormatKind::{Bold, Italic};
        assert_eq!(apply("__bold__", 4, 4, Bold), "bold");
        assert_eq!(apply("_em_", 2, 2, Italic), "em");
        assert_eq!(apply("___both___", 5, 5, Bold), "_both_");
        // The other layer is added around it, not mistaken for it.
        assert_eq!(apply("_em_", 2, 2, Bold), "**_em_**");
        assert_eq!(apply("__bold__", 4, 4, Italic), "*__bold__*");
        // An underscore inside a word is not a mark.
        assert_eq!(apply("snake_case_name", 8, 8, Italic), "*snake_case_name*");
    }

    fn cursor_at(text: &str, byte: usize) -> super::text_editor::Cursor {
        super::text_editor::Cursor {
            position: super::byte_pos(text, byte),
            selection: None,
        }
    }

    fn selecting(text: &str, anchor: usize, caret: usize) -> super::text_editor::Cursor {
        super::text_editor::Cursor {
            position: super::byte_pos(text, caret),
            selection: Some(super::byte_pos(text, anchor)),
        }
    }

    /// The text after a change, and where the caret and the anchor are in it, as bytes.
    fn after(text: &str, change: &super::Change) -> (String, usize, Option<usize>) {
        let from = super::pos_byte(text, change.anchor);
        let to = super::pos_byte(text, change.end);
        let next = format!("{}{}{}", &text[..from], change.insert, &text[to..]);
        let caret = super::pos_byte(&next, change.after.position);
        let anchor = change.after.selection.map(|at| super::pos_byte(&next, at));
        (next, caret, anchor)
    }

    fn enter(text: &str, byte: usize) -> Option<(String, usize)> {
        let change = super::continue_enter(text, cursor_at(text, byte))?;
        let (next, caret, _) = after(text, &change);
        Some((next, caret))
    }

    #[test]
    fn enter_continues_a_list_and_a_second_enter_ends_it() {
        assert_eq!(enter("- one", 5), Some(("- one\n- ".to_owned(), 8)));
        assert_eq!(enter("1. one", 6), Some(("1. one\n2. ".to_owned(), 10)));
        assert_eq!(enter("9. nine", 7), Some(("9. nine\n10. ".to_owned(), 12)));
        assert_eq!(
            enter("- [x] done", 10),
            Some(("- [x] done\n- [ ] ".to_owned(), 17))
        );
        // On the empty item that Enter just made, the list ends.
        assert_eq!(enter("- one\n- ", 8), Some(("- one\n".to_owned(), 6)));
        assert_eq!(enter("- ", 2), Some((String::new(), 0)));
        assert_eq!(enter("- [ ] ", 6), Some((String::new(), 0)));
        assert_eq!(enter("1. one\n2. ", 10), Some(("1. one\n".to_owned(), 7)));
        // An empty item that is nested comes out one level first.
        assert_eq!(enter("- a\n  - ", 8), Some(("- a\n- ".to_owned(), 6)));
        // Text after the caret moves to the new item.
        assert_eq!(enter("- one two", 5), Some(("- one\n-  two".to_owned(), 8)));
        // A paragraph, and code, are left to the editor.
        assert_eq!(enter("plain", 5), None);
        assert_eq!(enter("```\n- x\n```", 9), None);
    }

    #[test]
    fn enter_continues_a_quote_and_a_second_enter_ends_it() {
        assert_eq!(enter("> quote", 7), Some(("> quote\n> ".to_owned(), 10)));
        assert_eq!(enter(">> deep", 7), Some((">> deep\n>> ".to_owned(), 11)));
        assert_eq!(enter("> a\n> ", 6), Some(("> a\n".to_owned(), 4)));
        assert_eq!(enter("> ", 2), Some((String::new(), 0)));
        // A caret in the marker itself is left alone.
        assert_eq!(enter("> quote", 1), None);
        // A greater-than that is not at the start of a line is not a quote.
        assert_eq!(enter("a > b", 5), None);
    }

    #[test]
    fn tab_nests_a_list_item_and_shift_tab_brings_it_out() {
        let text = "- one";
        let change = super::shift_items(text, &cursor_at(text, 3), false).expect("nests");
        assert_eq!(after(text, &change), ("  - one".to_owned(), 5, None));
        let nested = "  - one";
        let change = super::shift_items(nested, &cursor_at(nested, 5), true).expect("comes out");
        assert_eq!(after(nested, &change), ("- one".to_owned(), 3, None));
        // A tab character comes out as one.
        let tabbed = "\t- one";
        let change = super::shift_items(tabbed, &cursor_at(tabbed, 4), true).expect("tab");
        assert_eq!(after(tabbed, &change).0, "- one");
        // Nothing to bring out, nothing to nest in a paragraph, and nesting stops at a limit.
        assert!(super::shift_items("- one", &cursor_at("- one", 3), true).is_none());
        assert!(super::shift_items("plain", &cursor_at("plain", 2), false).is_none());
        let deep = format!("{}- x", " ".repeat(24));
        assert!(super::shift_items(&deep, &cursor_at(&deep, 28), false).is_none());
        // An ordered item and a task nest the same way.
        let task = "- [ ] box";
        let change = super::shift_items(task, &cursor_at(task, 9), false).expect("task");
        assert_eq!(after(task, &change).0, "  - [ ] box");
    }

    #[test]
    fn tab_nests_every_list_item_a_selection_covers() {
        let text = "- a\n- b\nplain\n- c";
        // From inside the first item to inside the last.
        let change = super::shift_items(text, &selecting(text, 2, 17), false).expect("block");
        let (next, caret, anchor) = after(text, &change);
        assert_eq!(next, "  - a\n  - b\nplain\n  - c");
        // The selection still covers the same words.
        assert_eq!(anchor.map(|at| &next[at..=at]), Some("a"));
        assert_eq!(&next[caret - 1..caret], "c");
        // Taking them out again gets back to the start.
        let change = super::shift_items(
            &next,
            &selecting(&next, anchor.expect("anchor"), caret),
            true,
        )
        .expect("back");
        assert_eq!(after(&next, &change).0, text);
        // A selection that ends at the start of a line leaves that line alone.
        let two = "- a\n- b";
        let change = super::shift_items(two, &selecting(two, 0, 4), false).expect("first only");
        assert_eq!(after(two, &change).0, "  - a\n- b");
    }

    #[test]
    fn a_word_in_the_personal_dictionary_is_not_underlined() {
        let _turn = crate::tests_support::one_at_a_time();
        let word = "zzqxfrobnicate";
        let line = format!("we {word} now");
        assert_eq!(flagged(&line), vec![word.to_owned()]);
        super::set_personal_words(&[word.to_owned()]);
        assert_eq!(flagged(&line), Vec::<String>::new());
        // Its endings are fine too.
        assert_eq!(
            flagged(&format!("{word}s and {word}'s")),
            Vec::<String>::new()
        );
        super::set_personal_words(&[]);
        assert_eq!(flagged(&line), vec![word.to_owned()]);
    }

    #[test]
    fn the_word_to_add_is_the_one_at_the_caret() {
        assert_eq!(
            super::word_at("a keypair here", 4),
            Some("keypair".to_owned())
        );
        // At the end of a word counts.
        assert_eq!(
            super::word_at("a keypair here", 9),
            Some("keypair".to_owned())
        );
        assert_eq!(super::word_at("Sam's", 2), Some("sam's".to_owned()));
        assert_eq!(super::word_at("a  b", 2), None);
        assert_eq!(super::word_at("ab", 1), None, "too short");
        assert_eq!(super::word_at("h264 codec", 2), None, "has a digit");
        assert_eq!(super::word_at("snake_case", 3), None, "has a symbol");
    }

    fn flagged(text: &str) -> Vec<String> {
        super::misspellings(text).into_iter().collect()
    }

    #[test]
    fn common_words_are_not_underlined() {
        // Words that the old 9,900-word list flagged, and ordinary prose around them.
        let text = "We cannot pause the encrypted sync. The sovereign tool was scoped, rewritten, \
            and quietly left unchanged. Deliberately narrower, it borrows the breadth of a cipher. \
            People went running, walked home, and happily made tomorrow's plans. The children \
            studied cities, boxes, and replayed the recording. Colour and color are both fine. \
            Please review the delegated sidebar, the audited primitives, and the redundancy of \
            the versioned mailbox. Email me online about the website, the app, and the repo.";
        assert_eq!(flagged(text), Vec::<String>::new());
    }

    #[test]
    fn a_compound_is_judged_by_its_parts() {
        let ok = "A well-known and/or state-of-the-art, long-lived, end-to-end tool \u{2014} \
            built by a self-taught maker \u{2013} is on-topic.";
        assert_eq!(flagged(ok), Vec::<String>::new());
        assert_eq!(
            flagged("a pre-approved, non-profit, anti-pattern plan"),
            Vec::<String>::new()
        );
        // A prefix is only a prefix before a hyphen.
        assert_eq!(flagged("the pre was"), vec!["pre".to_owned()]);
        assert_eq!(flagged("a well-knwon tool"), vec!["well-knwon".to_owned()]);
        assert_eq!(flagged("this and/orx that"), vec!["and/orx".to_owned()]);
    }

    #[test]
    fn contractions_and_possessives_are_words() {
        let text = "I don\u{2019}t think it's what they're saying. We've seen John's note, \
            and she'd say I'm sure it can't be. Won't you come? It's five o'clock, ma'am. \
            The dogs' bowls are empty, and y'all know that isn't all.";
        assert_eq!(flagged(text), Vec::<String>::new());
        assert_eq!(flagged("it dosn't work"), vec!["dosn't".to_owned()]);
    }

    #[test]
    fn things_that_are_not_words_are_left_alone() {
        let text = "See ROADMAP.md and notes_v2.txt, h264, 3rd, 1st, https://example.com/a, \
            /usr/bin/env, ~/notes, #tag, @someone, NASA, iPhone, JavaScript, caf\u{e9}, e.g. \
            and U.S. rules, and (maybe) word(s).";
        assert_eq!(flagged(text), Vec::<String>::new());
    }

    #[test]
    fn a_name_is_left_alone_until_it_starts_a_sentence() {
        assert_eq!(flagged("I met Zorblax yesterday."), Vec::<String>::new());
        assert_eq!(
            flagged("It ended. Zorblax left."),
            vec!["Zorblax".to_owned()]
        );
        assert_eq!(flagged("zorblax left."), vec!["zorblax".to_owned()]);
        // A real word that is capitalized is fine anywhere.
        assert_eq!(flagged("Then Tomorrow came."), Vec::<String>::new());
    }

    #[test]
    fn real_misspellings_are_still_caught() {
        let typos = [
            "recieve",
            "definately",
            "seperate",
            "occured",
            "untill",
            "wierd",
            "accomodate",
            "adress",
            "teh",
            "wich",
            "becuase",
            "alot",
            "beleive",
            "tommorow",
            "thier",
            "freind",
            "neccessary",
            "goverment",
            "enviroment",
            "embarass",
            "acheive",
            "existance",
            "grammer",
            "independant",
            "occurence",
            "recomend",
            "succesful",
            "tendancy",
            "truely",
        ];
        let text = typos.join(" ");
        let found = flagged(&text);
        let missed: Vec<&str> = typos
            .iter()
            .copied()
            .filter(|word| !found.iter().any(|hit| hit == word))
            .collect();
        assert!(missed.is_empty(), "not caught: {missed:?}");
        // At the start of a sentence a capital does not hide one.
        assert_eq!(flagged("Recieve it."), vec!["Recieve".to_owned()]);
    }

    #[test]
    fn the_underline_and_the_proof_use_the_same_word() {
        for raw in [
            "**word**",
            "(word),",
            "\"word\"",
            "word.",
            "Word",
            "well-known",
            "don\u{2019}t",
        ] {
            let key = super::spell_key(raw).expect("a word");
            assert!(key.chars().any(|ch| ch.is_ascii_alphabetic()));
            assert!(!key.starts_with(|ch: char| !ch.is_ascii_alphabetic()));
            assert!(!key.ends_with(|ch: char| !ch.is_ascii_alphabetic()));
        }
        assert_eq!(super::spell_key("don\u{2019}t"), Some("don't".to_owned()));
        assert_eq!(super::spell_key("Word,"), Some("Word".to_owned()));
        assert_eq!(super::spell_key("is"), None);
        assert_eq!(super::spell_key("a1b2c3"), None);
    }

    #[test]
    fn the_word_lists_are_clean() {
        for (name, list) in [
            ("words.txt", super::WORDS),
            ("words-extra.txt", super::WORDS_EXTRA),
        ] {
            let mut last = "";
            for line in list.lines() {
                assert!(line.len() >= 3, "{name}: {line:?} is too short");
                assert!(
                    line.bytes().all(|byte| byte.is_ascii_lowercase()),
                    "{name}: {line:?} is not lowercase letters"
                );
                assert!(last < line, "{name}: {line:?} is out of order or repeated");
                last = line;
            }
        }
        for word in super::WORDS_EXTRA.lines() {
            assert!(
                super::WORDS.lines().all(|main| main != word),
                "words-extra.txt repeats {word:?} from words.txt"
            );
        }
        assert!(super::dictionary().len() > 90_000);
    }

    #[test]
    fn a_version_link_keeps_the_note_id() {
        let id = "ab".repeat(32);
        let version = "cd".repeat(32);
        let line = format!("[[{id}@{version}|Today]]");
        assert_eq!(super::chapter_names(&line), vec![id.clone()]);
        let at = line.find("Today").expect("label");
        assert_eq!(
            super::opened_at(&line, at),
            Some(super::OpenLink::Note(super::NoteTarget {
                name: id,
                version: Some(version),
            }))
        );
    }

    #[test]
    fn find_ignores_ascii_case() {
        assert_eq!(
            super::find_bytes("Alpha beta", "beta"),
            vec!["Alpha ".len()]
        );
        assert_eq!(super::find_bytes("Alpha", "alpha"), vec![0]);
        assert!(super::find_bytes("Alpha", " ").is_empty());
    }

    #[test]
    fn a_ctrl_click_finds_a_link_or_a_note_name() {
        let text = "See [the clinic](https://example.com) today\n";
        let at = text.find("clinic").expect("label");
        assert_eq!(
            super::opened_at(text, at),
            Some(super::OpenLink::Url("https://example.com".to_owned()))
        );
        let wiki = "Open [[Daily|today]]\n";
        let at = wiki.find("today").expect("label");
        assert_eq!(
            super::opened_at(wiki, at),
            Some(super::OpenLink::Note(super::NoteTarget {
                name: "Daily".to_owned(),
                version: None,
            }))
        );
        let bare = "go https://example.com/a. now";
        let at = bare.find("https").expect("url");
        assert_eq!(
            super::opened_at(bare, at),
            Some(super::OpenLink::Url("https://example.com/a".to_owned()))
        );
    }

    #[test]
    fn usual_marks_read_as_they_are_typed() {
        let both = "***both***";
        let (spans, carry) = paint(both, Carry::default());
        assert!(hit(
            &spans,
            both,
            "both",
            Mark {
                face: Face::Text,
                flags: STRONG | EM
            }
        ));
        assert_eq!(carry.len, 0);

        let high = "a ==sun==";
        let (spans, _) = paint(high, Carry::default());
        assert!(hit(
            &spans,
            high,
            "sun",
            Mark {
                face: Face::Text,
                flags: HIGH
            }
        ));

        let wiki = "See [[Daily|today]]";
        let (spans, _) = paint(wiki, Carry::default());
        assert!(hit(
            &spans,
            wiki,
            "today",
            Mark {
                face: Face::Wiki,
                flags: 0
            }
        ));

        let url = "go https://example.com/a. now";
        let (spans, _) = paint(url, Carry::default());
        assert!(hit(
            &spans,
            url,
            "https://example.com/a",
            Mark {
                face: Face::Link,
                flags: 0
            }
        ));

        let image = "![cat](note-files/cat.png)";
        let (spans, _) = paint(image, Carry::default());
        assert!(hit(
            &spans,
            image,
            "cat",
            Mark {
                face: Face::Image,
                flags: 0
            }
        ));
    }

    #[test]
    fn blocks_tables_and_nested_marks() {
        let quote = ">> inner";
        let (spans, _) = paint(quote, Carry::default());
        assert!(hit(
            &spans,
            quote,
            ">> ",
            Mark {
                face: Face::Quote,
                flags: 0
            }
        ));

        let table = "| a | **b** |";
        let (spans, _) = paint(table, Carry::default());
        assert!(hit(&spans, table, "b", strong()));
        assert!(hit(&spans, table, "|", marker()));

        let rule = "| --- | :---: |";
        let (spans, _) = paint(rule, Carry::default());
        assert!(hit(&spans, rule, rule, marker()));

        let note = "> [!note] Hello";
        let (spans, _) = paint(note, Carry::default());
        assert!(hit(
            &spans,
            note,
            "[!note]",
            Mark {
                face: Face::Callout(0),
                flags: 0,
            }
        ));

        let foot = "see [^1]";
        let (spans, _) = paint(foot, Carry::default());
        assert!(hit(
            &spans,
            foot,
            "[^1]",
            Mark {
                face: Face::Link,
                flags: 0
            }
        ));

        let mixed = "~~**gone**~~";
        let (spans, _) = paint(mixed, Carry::default());
        assert!(hit(
            &spans,
            mixed,
            "gone",
            Mark {
                face: Face::Text,
                flags: STRONG | STRIKE
            }
        ));

        let deep = "    - nested";
        let (spans, _) = paint(deep, Carry::default());
        assert!(hit(&spans, deep, "    - ", marker()));
    }

    fn hit(spans: &[(std::ops::Range<usize>, Mark)], line: &str, text: &str, mark: Mark) -> bool {
        spans
            .iter()
            .any(|(range, kind)| *kind == mark && line.get(range.start..range.end) == Some(text))
    }

    fn caret(line: usize, column: usize) -> super::text_editor::Cursor {
        super::text_editor::Cursor {
            position: super::text_editor::Position { line, column },
            selection: None,
        }
    }

    fn applied(text: &str, cursor: super::text_editor::Cursor, kind: super::FormatKind) -> String {
        let change = super::format_change(text, cursor, kind);
        let from = super::pos_byte(text, change.anchor);
        let to = super::pos_byte(text, change.end);
        let mut out = String::new();
        out.push_str(text.get(..from).unwrap_or(""));
        out.push_str(&change.insert);
        out.push_str(text.get(to..).unwrap_or(""));
        out
    }

    #[test]
    fn word_marks_wrap_the_word_and_toggle_off() {
        let text = "see bold now";
        let once = applied(text, caret(0, 6), super::FormatKind::Bold);
        assert_eq!(once, "see **bold** now");
        let twice = applied(&once, caret(0, 8), super::FormatKind::Bold);
        assert_eq!(twice, "see bold now");
    }

    #[test]
    fn line_marks_toggle_and_inserts_drop_a_block() {
        assert_eq!(
            applied("milk", caret(0, 1), super::FormatKind::Task),
            "- [ ] milk"
        );
        assert_eq!(
            applied("- [ ] milk", caret(0, 4), super::FormatKind::Task),
            "milk"
        );
        assert_eq!(
            applied("title", caret(0, 0), super::FormatKind::Heading),
            "# title"
        );
        assert_eq!(
            applied("# title", caret(0, 3), super::FormatKind::Heading),
            "## title"
        );
        assert_eq!(
            applied("docs", caret(0, 2), super::FormatKind::Link),
            "[docs](https://)"
        );
        assert_eq!(
            applied("above", caret(0, 5), super::FormatKind::Rule),
            "above\n---\n"
        );
    }

    #[test]
    fn comments_math_tags_and_callouts_read_as_typed() {
        let comment = "keep %%secret%% here";
        let (spans, carry) = paint(comment, Carry::default());
        assert!(hit(
            &spans,
            comment,
            "secret",
            Mark {
                face: Face::Meta,
                flags: 0
            }
        ));
        assert_eq!(carry.len, 0);
        let math = "energy $e=mc^2$ today";
        let (spans, _) = paint(math, Carry::default());
        assert!(hit(
            &spans,
            math,
            "e=mc^2",
            Mark {
                face: Face::Math,
                flags: 0
            }
        ));
        let custom = "- [?] ask";
        let lines = task_lines(custom);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].done);
        assert_eq!(lines[0].text, "ask");
    }

    #[test]
    fn enter_continues_a_list_or_task_and_backspace_leaves_an_empty_one() {
        let (next, line, column) = entered("- milk", 0, 6).unwrap();
        assert_eq!(next, "- milk\n- ");
        assert_eq!((line, column), (1, 2));

        let (split, _, _) = entered("- milk", 0, 3).unwrap();
        assert_eq!(split, "- m\n- ilk");

        let (numbered, _, _) = entered("1. one", 0, 6).unwrap();
        assert_eq!(numbered, "1. one\n2. ");
        let (paren, _, _) = entered("3) three", 0, 8).unwrap();
        assert_eq!(paren, "3) three\n4) ");

        let (task, line, column) = entered("- [ ] call", 0, 10).unwrap();
        assert_eq!(task, "- [ ] call\n- [ ] ");
        assert_eq!((line, column), (1, 6));
        let (done, _, _) = entered("- [x] call", 0, 10).unwrap();
        assert_eq!(done, "- [x] call\n- [ ] ");
        let (star, _, _) = entered("* [ ] call", 0, 10).unwrap();
        assert_eq!(star, "* [ ] call\n* [ ] ");
        let (nested, _, _) = entered("  - child", 0, 9).unwrap();
        assert_eq!(nested, "  - child\n  - ");

        // Enter on an empty item ends the list. It no longer adds another empty item.
        let (blank, _, _) = entered("- ", 0, 2).unwrap();
        assert_eq!(blank, "");
        let (blank_task, _, _) = entered("- [ ] ", 0, 6).unwrap();
        assert_eq!(blank_task, "");

        let mid = "- a\n- b\n- c";
        let (kept, _, _) = entered(mid, 1, 3).unwrap();
        assert_eq!(kept, "- a\n- b\n- \n- c");

        assert!(entered("hello", 0, 5).is_none());
        assert!(entered("```\n- not", 1, 5).is_none());
        assert!(entered("---", 0, 3).is_none());

        let left = exited("- milk\n- ", 1, 2).unwrap();
        assert_eq!(left, "- milk\n");
        let left_task = exited("- [ ] ", 0, 6).unwrap();
        assert_eq!(left_task, "");
        assert!(exited("- milk", 0, 6).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn each_preview_reads_the_mark_that_was_typed() {
        let note = [
            "# One",
            "## Two",
            "### Three",
            "###### Six",
            "see **bold** and *em* and _under_ and ***both***",
            "~~gone~~ ==sun== `code`",
            "- bullet",
            "* star",
            "+ plus",
            "1. first",
            "2) second",
            "- [ ] open",
            "- [x] shut",
            "> said",
            "> [!warning] Careful",
            "| a | **b** |",
            "| --- | --- |",
            "---",
            "```",
            "**not bold**",
            "```",
            "$$",
            "e=mc",
            "$$",
            "See [[Daily|today]] and ![[cat.png]]",
            "go https://example.com/a. now",
            "[docs](https://docs.example)",
            "![cat](note-files/cat.png)",
            "keep %%secret%% here",
            "energy $e=mc^2$ today",
            "see #clinic/today.",
            "[^n]: the note",
            "see [^n]",
        ]
        .join("\n");
        let lines = paint_all(&note);
        expect(&lines, "# One", "# ", marker());
        expect(&lines, "# One", "One", heading_mark(1));
        expect(&lines, "## Two", "Two", heading_mark(2));
        expect(&lines, "### Three", "Three", heading_mark(3));
        expect(&lines, "###### Six", "Six", heading_mark(6));
        expect(
            &lines,
            "see **bold** and *em* and _under_ and ***both***",
            "bold",
            strong(),
        );
        expect(
            &lines,
            "see **bold** and *em* and _under_ and ***both***",
            "em",
            em(),
        );
        expect(
            &lines,
            "see **bold** and *em* and _under_ and ***both***",
            "under",
            em(),
        );
        expect(
            &lines,
            "see **bold** and *em* and _under_ and ***both***",
            "both",
            Mark {
                face: Face::Text,
                flags: STRONG | EM,
            },
        );
        expect(&lines, "~~gone~~ ==sun== `code`", "gone", strike_mark());
        expect(
            &lines,
            "~~gone~~ ==sun== `code`",
            "sun",
            Mark {
                face: Face::Text,
                flags: HIGH,
            },
        );
        expect(&lines, "~~gone~~ ==sun== `code`", "code", code_mark());
        expect(&lines, "- bullet", "- ", marker());
        expect(&lines, "* star", "* ", marker());
        expect(&lines, "+ plus", "+ ", marker());
        expect(&lines, "1. first", "1. ", marker());
        expect(&lines, "2) second", "2) ", marker());
        expect(&lines, "- [ ] open", "[ ]", check_mark(false));
        expect(&lines, "- [x] shut", "[x]", check_mark(true));
        expect(&lines, "- [x] shut", "shut", strike_mark());
        expect(
            &lines,
            "> said",
            "> ",
            Mark {
                face: Face::Quote,
                flags: 0,
            },
        );
        expect(
            &lines,
            "> [!warning] Careful",
            "[!warning]",
            Mark {
                face: Face::Callout(3),
                flags: 0,
            },
        );
        expect(&lines, "| a | **b** |", "b", strong());
        expect(&lines, "| --- | --- |", "| --- | --- |", marker());
        expect(&lines, "---", "---", marker());
        expect(&lines, "**not bold**", "**not bold**", code_mark());
        expect(&lines, "e=mc", "e=mc", math_mark());
        expect(
            &lines,
            "See [[Daily|today]] and ![[cat.png]]",
            "today",
            Mark {
                face: Face::Wiki,
                flags: 0,
            },
        );
        expect(
            &lines,
            "See [[Daily|today]] and ![[cat.png]]",
            "cat.png",
            Mark {
                face: Face::Image,
                flags: 0,
            },
        );
        expect(
            &lines,
            "go https://example.com/a. now",
            "https://example.com/a",
            Mark {
                face: Face::Link,
                flags: 0,
            },
        );
        expect(
            &lines,
            "[docs](https://docs.example)",
            "docs",
            Mark {
                face: Face::Link,
                flags: 0,
            },
        );
        expect(
            &lines,
            "![cat](note-files/cat.png)",
            "cat",
            Mark {
                face: Face::Image,
                flags: 0,
            },
        );
        expect(
            &lines,
            "keep %%secret%% here",
            "secret",
            Mark {
                face: Face::Meta,
                flags: 0,
            },
        );
        expect(
            &lines,
            "energy $e=mc^2$ today",
            "e=mc^2",
            Mark {
                face: Face::Math,
                flags: 0,
            },
        );
        expect(
            &lines,
            "[^n]: the note",
            "[^n]:",
            Mark {
                face: Face::Link,
                flags: 0,
            },
        );
        expect(
            &lines,
            "see [^n]",
            "[^n]",
            Mark {
                face: Face::Link,
                flags: 0,
            },
        );
    }

    fn entered(text: &str, line: usize, column: usize) -> Option<(String, usize, usize)> {
        let change = super::continue_enter(text, caret(line, column))?;
        let (out, pos) = apply_change(text, &change);
        Some((out, pos.line, pos.column))
    }

    fn exited(text: &str, line: usize, column: usize) -> Option<String> {
        let change = super::exit_empty_item(text, caret(line, column))?;
        Some(apply_change(text, &change).0)
    }

    fn apply_change(text: &str, change: &super::Change) -> (String, super::text_editor::Position) {
        let from = super::pos_byte(text, change.anchor);
        let to = super::pos_byte(text, change.end);
        let mut out = String::new();
        out.push_str(text.get(..from).unwrap_or(""));
        out.push_str(&change.insert);
        out.push_str(text.get(to..).unwrap_or(""));
        (out, change.after.position)
    }

    struct Painted {
        text: String,
        spans: Vec<(std::ops::Range<usize>, Mark)>,
    }

    fn paint_all(text: &str) -> Vec<Painted> {
        let mut carry = Carry::default();
        let mut lines = Vec::new();
        for (index, line) in text.split('\n').enumerate() {
            let (spans, next) = super::paint_in(line, carry, index == 0);
            lines.push(Painted {
                text: line.to_owned(),
                spans,
            });
            carry = next;
        }
        lines
    }

    fn expect(lines: &[Painted], line: &str, text: &str, mark: Mark) {
        let spans = lines
            .iter()
            .find(|row| row.text == line)
            .map_or(&[][..], |row| row.spans.as_slice());
        assert!(
            hit(spans, line, text, mark),
            "expected {text:?} as {mark:?} in {line:?}, spans were {spans:?}"
        );
    }

    #[test]
    fn a_settled_word_leaves_the_caret() {
        let none = Carry::default();
        let closed = "**bold**";
        // Typing the closing mark settles the word at once: the caret is outside.
        assert_eq!(super::edit_window(closed, closed.len(), none, false), 8..8);
        // The caret inside shows the whole run, so the marks can be read and changed.
        assert_eq!(super::edit_window(closed, 4, none, false), 0..8);
        assert_eq!(super::edit_window(closed, 6, none, false), 0..8);
        assert_eq!(super::edit_window(closed, 7, none, false), 0..8);
        assert_eq!(super::edit_window(closed, 0, none, false), 0..8);
        assert_eq!(super::edit_window("**bold", 6, none, false), 0..6);
        assert_eq!(super::edit_window("Hello", 5, none, false), 0..5);
        assert_eq!(super::edit_window("Hello ", 6, none, false), 6..6);
        let mixed = "**bold** next";
        assert_eq!(super::edit_window(mixed, 8, none, false), 8..8);
        assert_eq!(super::edit_window(mixed, 13, none, false), 9..13);
    }

    #[test]
    fn a_caret_moved_to_the_end_of_a_run_shows_it() {
        let none = Carry::default();
        // Arrowing or clicking to just after a closing mark touches the run, so a Backspace
        // there acts on marks that are on the screen.
        assert_eq!(super::edit_window("**bold**", 8, none, true), 0..8);
        let mixed = "**bold** next";
        assert_eq!(super::edit_window(mixed, 8, none, true), 0..8);
        // Past the space it is the next word again.
        assert_eq!(super::edit_window(mixed, 11, none, true), 9..13);
    }

    #[test]
    fn the_whole_outer_run_shows_when_the_caret_is_in_a_nested_one() {
        let none = Carry::default();
        let nested = "**bold *and italic* too**";
        // In the italic part, every mark of the bold run around it shows as well.
        assert_eq!(super::edit_window(nested, 10, none, false), 0..nested.len());
        // In the bold part outside the italic, the same.
        assert_eq!(super::edit_window(nested, 3, none, false), 0..nested.len());
        // Two runs side by side do not join.
        let apart = "**one** and **two**";
        assert_eq!(super::edit_window(apart, 3, none, false), 0..7);
        assert_eq!(super::edit_window(apart, 15, none, false), 12..19);
        assert_eq!(super::edit_window(apart, 9, none, false), 8..11);
    }

    #[test]
    fn a_link_shows_whole_when_the_caret_is_in_it() {
        let none = Carry::default();
        let text = "see [two words](https://example.com/a) now";
        let whole = 4..38;
        assert_eq!(&text[whole.clone()], "[two words](https://example.com/a)");
        // In the text, in the address, and on a bracket: the whole link.
        for at in [5, 9, 12, 20, 36, 4] {
            assert_eq!(super::edit_window(text, at, none, false), whole, "at {at}");
        }
        // Just after it, it settles when typing and shows when moved to.
        assert_eq!(super::edit_window(text, 38, none, false), 38..38);
        assert_eq!(super::edit_window(text, 38, none, true), whole);
        // A note link is the same.
        let wiki = "go to [[Daily|today]] soon";
        assert_eq!(super::edit_window(wiki, 12, none, false), 6..21);
    }

    #[test]
    fn a_run_inside_a_link_or_a_link_inside_a_run_shows_as_the_larger() {
        let none = Carry::default();
        let line = "**read [this](https://x.io) now**";
        assert_eq!(super::edit_window(line, 10, none, false), 0..line.len());
    }

    #[test]
    fn a_plain_word_stays_on_the_line_after_the_space() {
        let hello = "Hello ";
        assert_eq!(drawn(hello, 0..hello.len()), "Hello ");
        assert_eq!(drawn(hello, 0..6), "Hello ");
        let mixed = "hello **bold** there";
        assert_eq!(drawn(mixed, 0..mixed.len()), "hello bold there");
        assert_eq!(drawn("- buy milk", 0..10), "• buy milk");
        assert_eq!(drawn("# Title", 0..7), "Title");
    }

    fn drawn(line: &str, range: std::ops::Range<usize>) -> String {
        super::pieces_between(line, 0, range, Carry::default(), true, true)
            .iter()
            .map(|piece| piece.text.as_str())
            .collect()
    }

    #[test]
    fn a_click_lands_on_the_letter() {
        let piece = super::Piece {
            text: "Hello".to_owned(),
            origin: 10,
            literal: true,
            look: super::look_of(super::text_mark(), true),
            covers: 10..15,
        };
        assert_eq!(super::place_of(std::slice::from_ref(&piece), 0), 10);
        assert_eq!(super::place_of(std::slice::from_ref(&piece), 2), 12);
        assert_eq!(super::place_of(std::slice::from_ref(&piece), 5), 15);
    }

    #[test]
    fn a_task_keeps_its_box() {
        assert!(super::lead("- [ ] call").box_at.is_some());
        assert!(super::lead("> - [ ] call").box_at.is_some());
        assert!(super::lead("> [ ] call").box_at.is_some());
        assert!(super::lead("[ ] bare").box_at.is_some());
        assert!(
            super::lead("\t- [x] done")
                .box_at
                .is_some_and(|(_, done)| done)
        );
        assert!(super::lead("- milk").box_at.is_none());
        assert!(super::heading_px(1) > super::heading_px(2));
        assert!(super::heading_px(5) > super::heading_px(6));
        assert!(super::heading_px(1) < 36.0);
    }

    #[test]
    fn a_drag_selects_words_for_delete_and_bold() {
        let mut body = super::NoteBody::new();
        body.load("one two three");
        body.select_between(4, 13);
        assert_eq!(body.selection().as_deref(), Some("two three"));
        body.place_byte(0);
        assert!(body.selection().is_none());
        assert_eq!(body.caret_byte(), 0);
        body.select_between(4, 13);
        body.place_byte_soft(8);
        assert!(body.selection().is_none());
        assert_eq!(body.caret_byte(), 8);

        body.select_between(4, 13);
        assert_eq!(body.selection().as_deref(), Some("two three"));
        assert!(body.edit(&super::text_editor::Action::Edit(
            super::text_editor::Edit::Backspace
        )));
        assert_eq!(body.text(), "one ");

        body.load("one two three");
        body.select_between(4, 7);
        body.end_select();
        assert!(body.edit(&super::text_editor::Action::Edit(
            super::text_editor::Edit::Insert('X')
        )));
        assert_eq!(body.text(), "one X three");

        body.load("say **hello** there");
        body.select_between(6, 19);
        let change = super::format_change(&body.text(), body.cursor(), super::FormatKind::Bold);
        let (out, _) = apply_change("say **hello** there", &change);
        assert_eq!(out, "say **hello there**");

        let toggled = applied("say **hello** there", caret(0, 8), super::FormatKind::Bold);
        assert_eq!(toggled, "say hello there");
    }

    #[test]
    fn a_highlight_closes_even_when_a_space_got_inside() {
        let line = "==sun == more";
        let (spans, carry) = paint(line, Carry::default());
        assert_eq!(carry.len, 0);
        assert!(spans.iter().any(|(range, mark)| {
            mark.flags & super::HIGH != 0
                && line
                    .get(range.start..range.end)
                    .is_some_and(|text| text.contains("sun"))
        }));
        assert!(spans.iter().all(|(range, mark)| {
            let text = line.get(range.start..range.end).unwrap_or("");
            !text.contains("more") || mark.flags & super::HIGH == 0
        }));

        let across = paint("more==", paint("==sun", Carry::default()).1);
        assert_eq!(across.1.len, 0);
        assert!(hit(
            &across.0,
            "more==",
            "more",
            Mark {
                face: Face::Text,
                flags: super::HIGH,
            }
        ));

        let sloppy = super::text_editor::Cursor {
            position: super::text_editor::Position { line: 0, column: 4 },
            selection: Some(super::text_editor::Position { line: 0, column: 0 }),
        };
        assert_eq!(
            applied("sun more", sloppy, super::FormatKind::Highlight),
            "==sun== more"
        );
    }

    #[test]
    fn a_click_beside_a_highlight_does_not_land_on_the_mark() {
        let text = "see ==sun== now";
        assert_eq!(super::snap_caret(text, 6), 6);
        assert_eq!(super::snap_caret(text, 4), 6);
        assert_eq!(super::snap_caret(text, 9), 11);
        assert_eq!(super::snap_caret(text, 10), 11);
        assert_eq!(super::snap_caret(text, 12), 12);

        let marked = applied("see sun now", caret(0, 4), super::FormatKind::Highlight);
        assert_eq!(marked, "see ==sun== now");
        let off = applied(marked.as_str(), caret(0, 7), super::FormatKind::Highlight);
        assert_eq!(off, "see sun now");
    }

    #[test]
    fn a_highlight_covers_the_letters_not_the_whole_word() {
        let pieces = super::pieces_between("hello", 0, 0..5, Carry::default(), true, true);
        let paint = iced::Color::from_rgb8(0x66, 0xA5, 0xE1);
        let shaded = super::shade(pieces, Some((1, 4)), paint);
        let marked: Vec<_> = shaded
            .iter()
            .filter(|piece| piece.look.background.is_some())
            .map(|piece| piece.text.as_str())
            .collect();
        assert_eq!(marked, ["ell"]);
        assert_eq!(super::line_bounds("one\ntwo", 5), (4, 7));
        assert_eq!(super::covered(Some((3, 6)), 4, 7), Some((0, 2)));
        assert_eq!(super::covered(Some((0, 1)), 4, 7), None);

        let mut body = super::NoteBody::new();
        body.load("alpha beta\ngamma delta");
        body.place_byte(0);
        let window = body.live_at;
        let gamma = body.text().find("gamma").unwrap_or(0);
        body.place_byte_soft(gamma);
        assert_eq!(body.live_at, window);
        assert!(body.caret_byte() > 8);
        assert_eq!(body.open_line(), 0);
    }

    #[test]
    fn the_light_follows_the_caret_not_the_open_word() {
        let mut body = super::NoteBody::new();
        body.load("First paragraph.\n\nSecond paragraph.\nThird line.");
        body.place_byte(0);
        assert_eq!(body.open_line(), 0);
        let second = body.text().find("Second").unwrap_or(0);
        body.place_byte_soft(second);
        assert_eq!(
            body.open_line(),
            0,
            "the open word stayed on the first line"
        );
        let at = super::spotlight_at(&body.text(), body.caret_byte());
        assert_eq!(at, 2, "the light is the caret's line");
        let third = body.text().find("Third").unwrap_or(0);
        assert_eq!(super::spotlight_at(&body.text(), third), 3);
    }

    #[test]
    fn a_selection_keeps_the_word_window() {
        let mut body = super::NoteBody::new();
        let text = "hello **bold** there";
        body.load(text);
        body.select_between(0, text.len());
        body.end_select();
        let (start, end) = body.live_at.expect("window");
        assert!(
            end.saturating_sub(start) < text.len(),
            "selection opened the whole line {start}..{end}"
        );
    }

    #[test]
    fn a_long_page_moves_quickly() {
        let mut note = String::new();
        for index in 0..250 {
            let _ = std::fmt::Write::write_fmt(
                &mut note,
                format_args!(
                    "Line {index} keeps **a word** and ==mark== in view.\n- [ ] task {index} stays boxed\n"
                ),
            );
        }
        let started = std::time::Instant::now();
        let mut body = super::NoteBody::new();
        body.load(&note);
        let loaded = started.elapsed();
        let step = (note.len() / 40).max(1);
        let mut at = 0;
        while at < note.len() {
            body.place_byte(at);
            at = at.saturating_add(step);
        }
        body.select_between(10, note.len() / 2);
        assert!(body.edit(&super::text_editor::Action::Edit(
            super::text_editor::Edit::Delete
        )));
        let elapsed = started.elapsed();
        assert!(loaded.as_millis() < 800, "parsing the page took {loaded:?}");
        assert!(
            elapsed.as_millis() < 1500,
            "moving through the page took {elapsed:?}"
        );
        assert!(body.text().len() < note.len());
    }

    #[test]
    fn the_links_in_a_note_are_listed_in_order() {
        let names = super::chapter_names(
            "[[One]]\nsee [[Two|shown]] and [[one]]\n![[picture]]\n```\n[[Hidden]]\n```\n[[Three]]\n",
        );
        assert_eq!(names, ["One", "Two", "Three"]);
        let mut body = super::NoteBody::new();
        body.load("alpha\nbeta gamma");
        body.select_between(0, 5);
        assert_eq!(
            body.selection_origin(4_000.0),
            Some((0.0, super::line_px("alpha")))
        );
    }

    #[test]
    fn a_paragraph_wraps_before_the_measure() {
        let rows = super::wrap_by("one two three", 7.0, 0..0, |text, _exact| {
            f32::from(u16::try_from(text.chars().count()).unwrap_or(u16::MAX))
        });
        assert_eq!(rows, vec![0..7, 8..13]);
        let wide = super::wrap_ranges(
            "alpha",
            4_000.0,
            iced::Font::default(),
            17.0,
            0..0,
            iced::Font::default(),
            17.0,
        );
        assert_eq!(wide.len(), 1);
        assert_eq!(wide.first().cloned(), Some(0..5));
    }
}
