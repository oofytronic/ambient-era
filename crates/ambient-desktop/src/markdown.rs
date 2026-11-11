//! One editor for a note. Markdown is styled as it is typed.
//!
//! Opening `**` bolds the following text. The closing `**` ends it. An unclosed
//! mark continues onto the next line and stops at a blank line. The marks stay
//! in the text. That text is what is saved. This is not a second pane, and it
//! does not wait until you leave the paragraph.

use std::ops::Range;

use iced::advanced::text::highlighter::{self, Highlighter};
use iced::widget::text_editor::{self, Action};
use iced::{Color, Element, Font, Length, Theme};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Carry {
    fence: bool,
    opens: [Open; OPEN_CAP],
    len: u8,
}

impl Default for Carry {
    fn default() -> Self {
        Self {
            fence: false,
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
        let (spans, outgoing) = paint(line, incoming);
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

    pub(super) fn selection(&self) -> Option<String> {
        self.editor.selection()
    }

    pub(super) fn perform(&mut self, action: &Action) {
        self.editor.perform(action.clone());
    }
}

pub(super) fn view(body: &NoteBody) -> Element<'_, Message> {
    text_editor::TextEditor::new(&body.editor)
        .id(iced::widget::Id::new("note-body"))
        .height(Length::Fill)
        .padding([6, 0])
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
    if mark.face == Face::Code {
        return highlighter::Format {
            color: Some(code),
            font: Some(Font::MONOSPACE),
        };
    }
    let mut color = match mark.face {
        Face::Text => None,
        Face::Marker => Some(marker),
        Face::Link | Face::Wiki | Face::Image | Face::Check | Face::Quote | Face::Heading(2) => {
            Some(accent)
        }
        Face::Heading(_) => Some(heading),
        Face::Checked => Some(done),
        Face::Code => Some(code),
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

fn paint(line: &str, carry: Carry) -> (Vec<(Range<usize>, Mark)>, Carry) {
    if carry.fence {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), code_mark());
        if fence_closer(line) {
            return (spans, Carry::default());
        }
        return (spans, carry);
    }
    if fence_marker(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), code_mark());
        return (
            spans,
            Carry {
                fence: true,
                ..Carry::default()
            },
        );
    }
    if line.trim().is_empty() || is_rule(line) {
        let mut spans = Vec::new();
        if is_rule(line) {
            push_span(&mut spans, 0, line.len(), marker());
        }
        return (spans, Carry::default());
    }
    if is_table_rule(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), marker());
        return (spans, carry);
    }
    if is_table_line(line) {
        return paint_table(line, carry);
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
        && rest.starts_with("[!")
        && let Some(rel) = rest.find(']')
    {
        let end = from.saturating_add(rel).saturating_add(1);
        push_span(&mut spans, from, end, marker());
        from = end;
    }
    let (inner, carry) = paint_inline(line, from, line.len(), carry, lead.base);
    spans.extend(inner);
    (spans, carry)
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
        if !in_code(carry) {
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
            && (in_code(carry) || link_end(line, index, limit).is_none())
            && wiki_end(line, index, limit).is_none()
            && auto_end(line, index, limit).is_none()
            && bare_url(line, index, limit).is_none()
            && footnote_end(line, index, limit).is_none()
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
    let Some(inner) = line.get(start.saturating_add(2)..end.saturating_sub(2)) else {
        return;
    };
    let label = inner.split_once('|').map_or(inner, |(_, label)| label);
    let label_at = if let Some((target, _)) = inner.split_once('|') {
        start
            .saturating_add(2)
            .saturating_add(target.len())
            .saturating_add(1)
    } else {
        start.saturating_add(2)
    };
    push_span(spans, start, label_at, marker());
    push_span(
        spans,
        label_at,
        label_at.saturating_add(label.len()),
        Mark {
            face: Face::Wiki,
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
    if !rest.starts_with("[[") {
        return None;
    }
    let close = rest.find("]]")?;
    if close <= 2 {
        return None;
    }
    Some(index.saturating_add(close).saturating_add(2))
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
    if top(carry) == Some(Open::BothStar) && rest.starts_with("***") {
        return Some((3, true));
    }
    if top(carry) == Some(Open::BothUnder) && rest.starts_with("___") {
        return Some((3, true));
    }
    if top(carry) == Some(Open::StrongStar) && rest.starts_with("**") {
        return Some((2, true));
    }
    if top(carry) == Some(Open::StrongUnder) && rest.starts_with("__") {
        return Some((2, true));
    }
    if top(carry) == Some(Open::Strike) && rest.starts_with("~~") {
        return Some((2, true));
    }
    if top(carry) == Some(Open::Highlight) && rest.starts_with("==") {
        return Some((2, true));
    }
    if top(carry) == Some(Open::EmStar) && rest.starts_with('*') && !rest.starts_with("**") {
        return Some((1, true));
    }
    if top(carry) == Some(Open::EmUnder)
        && rest.starts_with('_')
        && !rest.starts_with("__")
        && edge_after_limited(line, index.saturating_add(1), limit)
    {
        return Some((1, true));
    }
    if rest.starts_with("***") || rest.starts_with("___") {
        return Some((3, false));
    }
    if rest.starts_with("**")
        || rest.starts_with("__")
        || rest.starts_with("~~")
        || rest.starts_with("==")
    {
        return Some((2, false));
    }
    if rest.starts_with('`') {
        return Some((1, false));
    }
    if rest.starts_with('*')
        && rest
            .chars()
            .nth(1)
            .is_some_and(|char| !char.is_whitespace())
    {
        return Some((1, false));
    }
    if rest.starts_with('_')
        && edge_before(line, index)
        && rest
            .chars()
            .nth(1)
            .is_some_and(|char| !char.is_whitespace())
    {
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
    let mut flags = base.flags;
    for index in 0..usize::from(carry.len) {
        match carry.opens.get(index).copied() {
            Some(Open::StrongStar | Open::StrongUnder) => flags |= STRONG,
            Some(Open::EmStar | Open::EmUnder) => flags |= EM,
            Some(Open::BothStar | Open::BothUnder) => flags |= STRONG | EM,
            Some(Open::Strike) => flags |= STRIKE,
            Some(Open::Highlight) => flags |= HIGH,
            Some(Open::Code) | None => {}
        }
    }
    if flags == base.flags {
        return base;
    }
    Mark {
        face: if matches!(base.face, Face::Heading(_)) {
            Face::Text
        } else {
            base.face
        },
        flags,
    }
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
    let (box_at, done) = if body.starts_with("[ ]") {
        (Some(end), false)
    } else if body.starts_with("[x]") || body.starts_with("[X]") {
        (Some(end), true)
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

fn fence_marker(text: &str) -> bool {
    leading_spaces(text) <= 3 && text.trim_start().starts_with("```")
}

fn fence_closer(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|char| char == '`')
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

#[cfg(test)]
mod tests {
    use super::{
        Carry, EM, Face, HIGH, Mark, STRIKE, STRONG, check_mark, code_mark, em, heading_mark,
        marker, paint, set_task_line, strike_mark, strong, task_lines,
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
        assert!(hit(&spans, heading, "now", strong()));
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
        assert!(hit(&spans, note, "[!note]", marker()));

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
}
