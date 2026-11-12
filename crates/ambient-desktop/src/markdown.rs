//! One editor for a note. Markdown is styled as it is typed.
//!
//! Opening `**` bolds the following text. The closing `**` ends it. An unclosed
//! mark continues onto the next line and stops at a blank line. The marks stay
//! in the text. That text is what is saved. This is not a second pane, and it
//! does not wait until you leave the paragraph.

use std::ops::Range;

use iced::advanced::text::highlighter::{self, Highlighter};
use iced::widget::text_editor::{self, Action};
use iced::{Color, Element, Font, Length, Padding, Theme};

use super::{Message, binding, editor_style, sans, serif};

pub(super) struct NoteBody {
    editor: text_editor::Content,
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
    Tag,
    Math,
    Meta,
    Callout(u8),
}

const STRONG: u8 = 1;
const EM: u8 = 2;
const STRIKE: u8 = 4;
const HIGH: u8 = 8;

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
        Self {
            editor: text_editor::Content::new(),
        }
    }

    pub(super) fn text(&self) -> String {
        self.editor.text()
    }

    /// Replaces the text. The same text keeps the cursor where it is.
    pub(super) fn load(&mut self, source: &str) {
        if self.editor.text() == source {
            return;
        }
        self.editor = text_editor::Content::with_text(source);
    }

    pub(super) fn edit(&mut self, action: &Action) -> bool {
        let previous = self.editor.text();
        self.editor.perform(action.clone());
        previous != self.editor.text()
    }

    pub(super) fn perform(&mut self, action: &Action) {
        self.editor.perform(action.clone());
    }

    pub(super) fn cursor(&self) -> text_editor::Cursor {
        self.editor.cursor()
    }

    pub(super) fn place(&mut self, cursor: text_editor::Cursor) {
        self.editor.move_to(cursor);
    }
}

pub(super) fn view(body: &NoteBody) -> Element<'_, Message> {
    text_editor::TextEditor::new(&body.editor)
        .id(iced::widget::Id::new("note-body"))
        .height(Length::Fill)
        .padding(Padding {
            top: 6.0,
            right: 0.0,
            bottom: 48.0,
            left: 0.0,
        })
        .size(17)
        .line_height(1.45)
        .font(sans())
        .placeholder("Write. **bold** starts when you type the marks.")
        .on_action(Message::BodyEdited)
        .key_binding(binding)
        .style(editor_style)
        .highlight_with::<Live>((), mark_format)
        .into()
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
    let heading = if dark {
        Color::from_rgb8(0xF6, 0xF1, 0xE4)
    } else {
        Color::from_rgb8(0x2A, 0x16, 0x12)
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
        Face::Text => None,
        Face::Marker | Face::Meta => Some(marker),
        Face::Tag | Face::Link | Face::Wiki | Face::Image | Face::Check | Face::Quote => {
            Some(accent)
        }
        Face::Heading(2) => Some(accent),
        Face::Heading(_) => Some(heading),
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
            if let Some(end) = tag_end(line, index, limit) {
                push_span(
                    &mut spans,
                    index,
                    end,
                    Mark {
                        face: Face::Tag,
                        flags: 0,
                    },
                );
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
            && tag_end(line, index, limit).is_none()
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
            flags: 0,
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
        && can_close(line, index, 2, limit)
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
    if let Some(list) = list_parts(line) {
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
    let spaces = leading_spaces(line);
    if spaces > 24 {
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

fn tag_end(line: &str, index: usize, limit: usize) -> Option<usize> {
    if !edge_before(line, index) {
        return None;
    }
    let rest = line.get(index..limit)?;
    if !rest.starts_with('#') || rest.starts_with("##") {
        return None;
    }
    let mut end = index.saturating_add(1);
    let mut saw = false;
    while end < limit {
        let char = line.get(end..limit)?.chars().next()?;
        if char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '/') {
            saw = true;
            end = end.saturating_add(char.len_utf8());
        } else {
            break;
        }
    }
    while end > index.saturating_add(1) && line.as_bytes().get(end.saturating_sub(1)) == Some(&b'/')
    {
        end = end.saturating_sub(1);
    }
    (saw && end > index.saturating_add(1)).then_some(end)
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
        let Some(list) = list_parts(text) else {
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
#[cfg(test)]
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
#[cfg(test)]
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

#[cfg(test)]
fn task_prefix(line: &str, end: usize) -> String {
    let prefix = line.get(..end).unwrap_or("");
    if prefix.ends_with(' ') {
        prefix.to_owned()
    } else {
        format!("{prefix} ")
    }
}

#[cfg(test)]
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
            let list = list_parts(line)?;
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

/// Enter in a list or task inserts the next marker, including on an empty item.
/// Backspace on an empty item leaves the list. A fence, math block, or
/// frontmatter is left alone.
pub(super) fn continue_enter(text: &str, cursor: text_editor::Cursor) -> Option<Change> {
    let (start, line, parts) = list_at_cursor(text, &cursor)?;
    let column = cursor_column(line, cursor.position.column);
    let at = start.saturating_add(column.max(parts.end));
    let mut insert = String::from("\n");
    insert.push_str(&next_prefix(line, &parts));
    Some(rewrite(text, at, at, &insert, insert.len(), None))
}

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
    }
    let inner = text.get(from..to).unwrap_or("");
    if let Some(stripped) = peel(inner, left, right) {
        return rewrite(text, from, to, stripped, stripped.len(), None);
    }
    if surrounded(text, from, to, left, right) {
        let start = from.saturating_sub(left.len());
        let end = to.saturating_add(right.len());
        return rewrite(text, start, end, inner, inner.len(), None);
    }
    let mut insert = String::with_capacity(left.len() + inner.len() + right.len());
    insert.push_str(left);
    insert.push_str(inner);
    insert.push_str(right);
    let cursor_at = if inner.is_empty() {
        left.len()
    } else {
        insert.len()
    };
    rewrite(text, from, to, &insert, cursor_at, None)
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

fn word_bounds(text: &str, at: usize) -> (usize, usize) {
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
        let tag = "see #clinic/today.";
        let (spans, _) = paint(tag, Carry::default());
        assert!(hit(
            &spans,
            tag,
            "#clinic/today",
            Mark {
                face: Face::Tag,
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

        let (blank, _, _) = entered("- ", 0, 2).unwrap();
        assert_eq!(blank, "- \n- ");
        let (blank_task, _, _) = entered("- [ ] ", 0, 6).unwrap();
        assert_eq!(blank_task, "- [ ] \n- [ ] ");

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
            "see #clinic/today.",
            "#clinic/today",
            Mark {
                face: Face::Tag,
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
}
