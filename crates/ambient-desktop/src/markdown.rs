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

const OPEN_CAP: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    StrongStar,
    StrongUnder,
    EmStar,
    EmUnder,
    Strike,
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
enum Mark {
    Text,
    Marker,
    Strong,
    Em,
    Strike,
    Code,
    Link,
    Heading,
    Quote,
}

struct Lead {
    skip: usize,
    mark: Mark,
    base: Mark,
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
    let text = if dark {
        Color::from_rgb8(0xED, 0xEA, 0xDE)
    } else {
        Color::from_rgb8(0x1C, 0x19, 0x17)
    };
    let muted = if dark {
        Color::from_rgb8(0xA8, 0xA2, 0x9E)
    } else {
        Color::from_rgb8(0x6B, 0x65, 0x60)
    };
    let accent = if dark {
        Color::from_rgb8(0x66, 0xA5, 0xE1)
    } else {
        Color::from_rgb8(0x16, 0x44, 0x6F)
    };
    let (color, font) = match mark {
        Mark::Text => {
            return highlighter::Format {
                color: None,
                font: None,
            };
        }
        Mark::Marker | Mark::Strike => (muted, sans()),
        Mark::Strong => (text, sans_bold()),
        Mark::Em => (text, sans_italic()),
        Mark::Code => (text, Font::MONOSPACE),
        Mark::Link | Mark::Quote => (accent, sans()),
        Mark::Heading => (text, serif()),
    };
    highlighter::Format {
        color: Some(color),
        font: Some(font),
    }
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

fn paint(line: &str, mut carry: Carry) -> (Vec<(Range<usize>, Mark)>, Carry) {
    if carry.fence {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), Mark::Code);
        if fence_closer(line) {
            return (spans, Carry::default());
        }
        return (spans, carry);
    }
    if fence_marker(line) {
        let mut spans = Vec::new();
        push_span(&mut spans, 0, line.len(), Mark::Code);
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
            push_span(&mut spans, 0, line.len(), Mark::Marker);
        }
        return (spans, Carry::default());
    }

    let lead = lead(line);
    let mut spans = Vec::new();
    if lead.skip > 0 {
        push_span(&mut spans, 0, lead.skip, lead.mark);
    }
    let mut index = lead.skip;
    while index < line.len() {
        if line.as_bytes().get(index) == Some(&b'\\') {
            push_span(&mut spans, index, index.saturating_add(1), Mark::Marker);
            let escaped = char_len(line, index.saturating_add(1));
            if escaped > 0 {
                let end = index.saturating_add(1).saturating_add(escaped);
                let face = face(carry, lead.base);
                if face != Mark::Text {
                    push_span(&mut spans, index.saturating_add(1), end, face);
                }
                index = end;
            } else {
                index = index.saturating_add(1);
            }
            continue;
        }
        if top(carry) != Some(Open::Code)
            && let Some(end) = link_end(line, index)
        {
            paint_link(&mut spans, line, index, end);
            index = end;
            continue;
        }
        if let Some((len, close)) = token(line, index, carry) {
            let end = index.saturating_add(len);
            if close {
                pop(&mut carry);
            }
            push_span(&mut spans, index, end, Mark::Marker);
            if !close {
                push_open(&mut carry, opened(line, index));
            }
            index = end;
            continue;
        }
        let start = index;
        let face = face(carry, lead.base);
        index = index.saturating_add(char_len(line, index).max(1));
        while index < line.len()
            && line.as_bytes().get(index) != Some(&b'\\')
            && (top(carry) == Some(Open::Code) || link_end(line, index).is_none())
            && token(line, index, carry).is_none()
        {
            index = index.saturating_add(char_len(line, index).max(1));
        }
        if face != Mark::Text {
            push_span(&mut spans, start, index, face);
        }
    }
    (spans, carry)
}

fn paint_link(spans: &mut Vec<(Range<usize>, Mark)>, line: &str, start: usize, end: usize) {
    let Some(rest) = line.get(start..end) else {
        return;
    };
    let bang = usize::from(rest.starts_with("!["));
    let label_at = start.saturating_add(1).saturating_add(bang);
    let Some(label_rel) = line.get(label_at..end).and_then(|slice| slice.find("](")) else {
        push_span(spans, start, end, Mark::Link);
        return;
    };
    let label_end = label_at.saturating_add(label_rel);
    let url_at = label_end.saturating_add(2);
    push_span(spans, start, label_at, Mark::Marker);
    push_span(spans, label_at, label_end, Mark::Link);
    push_span(spans, label_end, url_at, Mark::Marker);
    push_span(spans, url_at, end.saturating_sub(1), Mark::Marker);
    push_span(spans, end.saturating_sub(1), end, Mark::Marker);
}

fn link_end(line: &str, index: usize) -> Option<usize> {
    let rest = line.get(index..)?;
    let label_at = if rest.starts_with("![") {
        2
    } else if rest.starts_with('[') {
        1
    } else {
        return None;
    };
    let label_rel = rest.get(label_at..)?.find("](")?;
    let url_at = index
        .saturating_add(label_at)
        .saturating_add(label_rel)
        .saturating_add(2);
    let url_rel = line.get(url_at..)?.find(')')?;
    Some(url_at.saturating_add(url_rel).saturating_add(1))
}

fn token(line: &str, index: usize, carry: Carry) -> Option<(usize, bool)> {
    let rest = line.get(index..)?;
    if top(carry) == Some(Open::Code) {
        return rest.starts_with('`').then_some((1, true));
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
    if top(carry) == Some(Open::EmStar) && rest.starts_with('*') {
        return Some((1, true));
    }
    if top(carry) == Some(Open::EmUnder)
        && rest.starts_with('_')
        && edge_after(line, index.saturating_add(1))
    {
        return Some((1, true));
    }
    if rest.starts_with("**") || rest.starts_with("__") || rest.starts_with("~~") {
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

fn opened(line: &str, index: usize) -> Open {
    let rest = line.get(index..).unwrap_or("");
    if rest.starts_with("**") {
        Open::StrongStar
    } else if rest.starts_with("__") {
        Open::StrongUnder
    } else if rest.starts_with("~~") {
        Open::Strike
    } else if rest.starts_with('`') {
        Open::Code
    } else if rest.starts_with('*') {
        Open::EmStar
    } else {
        Open::EmUnder
    }
}

fn face(carry: Carry, base: Mark) -> Mark {
    match top(carry) {
        Some(Open::StrongStar | Open::StrongUnder) => Mark::Strong,
        Some(Open::EmStar | Open::EmUnder) => Mark::Em,
        Some(Open::Strike) => Mark::Strike,
        Some(Open::Code) => Mark::Code,
        None => base,
    }
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
    if let Some(end) = heading_end(line) {
        return Lead {
            skip: end,
            mark: Mark::Marker,
            base: Mark::Heading,
        };
    }
    if let Some(end) = quote_end(line) {
        return Lead {
            skip: end,
            mark: Mark::Quote,
            base: Mark::Text,
        };
    }
    if let Some(end) = list_end(line) {
        return Lead {
            skip: end,
            mark: Mark::Marker,
            base: Mark::Text,
        };
    }
    Lead {
        skip: 0,
        mark: Mark::Marker,
        base: Mark::Text,
    }
}

fn heading_end(line: &str) -> Option<usize> {
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
        return Some(spaces.saturating_add(hashes).saturating_add(gap));
    }
    None
}

fn quote_end(line: &str) -> Option<usize> {
    let spaces = leading_spaces(line);
    if spaces > 3 {
        return None;
    }
    let rest = line.get(spaces..)?;
    if !rest.starts_with('>') {
        return None;
    }
    let gap = usize::from(rest.as_bytes().get(1) == Some(&b' '));
    Some(spaces.saturating_add(1).saturating_add(gap))
}

fn list_end(line: &str) -> Option<usize> {
    let spaces = leading_spaces(line);
    if spaces > 8 {
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
    if let Some(body) = line.get(end..) {
        if body.starts_with("[ ] ") || body.starts_with("[x] ") || body.starts_with("[X] ") {
            end = end.saturating_add(4);
        } else if body == "[ ]" || body == "[x]" || body == "[X]" {
            end = end.saturating_add(3);
        }
    }
    Some(end)
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

fn char_len(text: &str, index: usize) -> usize {
    text.get(index..)
        .and_then(|rest| rest.chars().next())
        .map_or(0, char::len_utf8)
}

fn edge_before(text: &str, index: usize) -> bool {
    text.get(..index)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|char| !char.is_ascii_alphanumeric())
}

fn edge_after(text: &str, index: usize) -> bool {
    text.get(index..)
        .and_then(|rest| rest.chars().next())
        .is_none_or(|char| !char.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{Carry, Mark, paint};

    #[test]
    fn bold_starts_at_the_opening_marks() {
        let line = "See **bold";
        let (spans, carry) = paint(line, Carry::default());
        assert!(hit(&spans, line, "**", Mark::Marker));
        assert!(hit(&spans, line, "bold", Mark::Strong));
        assert_eq!(carry.len, 1);
    }

    #[test]
    fn the_closer_ends_the_bold() {
        let line = "**bold** tail";
        let (spans, carry) = paint(line, Carry::default());
        assert!(hit(&spans, line, "bold", Mark::Strong));
        assert!(!hit(&spans, line, " tail", Mark::Strong));
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn bold_continues_until_the_closer_on_the_next_line() {
        let (first, carry) = paint("**still", Carry::default());
        assert!(hit(&first, "**still", "still", Mark::Strong));
        let (second, carry) = paint("bold** end", carry);
        assert!(hit(&second, "bold** end", "bold", Mark::Strong));
        assert!(!hit(&second, "bold** end", " end", Mark::Strong));
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
        assert!(hit(&spans, line, "em", Mark::Em));
        assert!(hit(&spans, line, "code", Mark::Code));
        let heading = "## Title **now**";
        let (spans, carry) = paint(heading, Carry::default());
        assert!(hit(&spans, heading, "## ", Mark::Marker));
        assert!(hit(&spans, heading, "Title ", Mark::Heading));
        assert!(hit(&spans, heading, "now", Mark::Strong));
        assert_eq!(carry.len, 0);
    }

    #[test]
    fn a_fence_stays_code_until_the_closer() {
        let (_, carry) = paint("```", Carry::default());
        assert!(carry.fence);
        let (inside, carry) = paint("**not bold**", carry);
        assert!(hit(&inside, "**not bold**", "**not bold**", Mark::Code));
        let (_, carry) = paint("```", carry);
        assert!(!carry.fence);
        assert_eq!(carry.len, 0);
    }

    fn hit(spans: &[(std::ops::Range<usize>, Mark)], line: &str, text: &str, mark: Mark) -> bool {
        spans
            .iter()
            .any(|(range, kind)| *kind == mark && line.get(range.start..range.end) == Some(text))
    }
}
