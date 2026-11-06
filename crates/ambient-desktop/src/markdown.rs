//! Live markdown for one note.
//!
//! The file on disk stays the raw markdown. A line is drawn rendered until it
//! is clicked, and then the marks come back in a small editor. Leaving the line
//! (Enter, or an arrow onto another line) renders it again. This is the usual
//! preview, not a second pane.

use std::ops::Range;

use iced::widget::text_editor::{self, Action, Edit, Motion};
use iced::widget::{
    checkbox, column, container, mouse_area, rich_text, row, rule, scrollable, span, text,
};
use iced::{Background, Color, Element, Font, Length, never};

use super::{Ink, Message, binding, editor_style, sans, serif};

pub(super) struct NoteBody {
    source: String,
    focus: Option<Range<usize>>,
    editor: text_editor::Content,
}

pub(super) struct Edited {
    pub changed: bool,
    pub refocus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Heading(u8),
    Paragraph,
    Item,
    Quote,
    Code,
    Rule,
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Block {
    kind: Kind,
    start: usize,
    end: usize,
}

struct Line<'a> {
    start: usize,
    content_end: usize,
    end: usize,
    text: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Code(String),
    Strong(String),
    Em(String),
    Strike(String),
    Link(String),
    Image(String),
}

impl NoteBody {
    pub(super) fn new() -> Self {
        Self {
            source: String::new(),
            focus: None,
            editor: text_editor::Content::new(),
        }
    }

    pub(super) fn text(&self) -> String {
        self.source.clone()
    }

    pub(super) fn load(&mut self, source: &str, editing: bool) {
        source.clone_into(&mut self.source);
        if editing {
            let end = source.strip_suffix('\n').map_or(source.len(), str::len);
            self.focus = Some(0..end);
            let raw = self.source.get(..end).unwrap_or("");
            self.editor = text_editor::Content::with_text(raw);
        } else {
            self.focus = None;
            self.editor = text_editor::Content::new();
        }
    }

    pub(super) fn edit(&mut self, action: &Action) -> Edited {
        if self.focus.is_none() {
            return Edited {
                changed: false,
                refocus: false,
            };
        }
        if matches!(action, Action::Edit(Edit::Backspace)) && caret_at_start(&self.editor) {
            return Edited {
                changed: self.merge(false),
                refocus: true,
            };
        }
        if matches!(action, Action::Edit(Edit::Delete)) && caret_at_end(&self.editor) {
            let changed = self.merge(true);
            return Edited {
                changed,
                refocus: changed,
            };
        }
        let up = matches!(action, Action::Move(Motion::Up));
        let down = matches!(action, Action::Move(Motion::Down));
        let before = self.editor.cursor().position;
        let last = self.editor.line_count().saturating_sub(1);
        let previous = self.editor.text();
        self.editor.perform(action.clone());
        self.splice();
        let changed = previous != self.editor.text();
        let split = matches!(action, Action::Edit(Edit::Enter | Edit::Paste(_)));
        if split && !caret_inside_fence(&self.editor) && self.focus_caret_line() {
            return Edited {
                changed: true,
                refocus: true,
            };
        }
        if up
            && before.line == 0
            && self.editor.cursor().position.line == 0
            && self.focus_sibling(false)
        {
            return Edited {
                changed: false,
                refocus: true,
            };
        }
        if down
            && before.line == last
            && self.editor.cursor().position.line == last
            && self.focus_sibling(true)
        {
            return Edited {
                changed: false,
                refocus: true,
            };
        }
        Edited {
            changed,
            refocus: false,
        }
    }

    pub(super) fn focus_index(&mut self, index: usize) -> bool {
        let blocks = parse(&self.source);
        let Some(block) = blocks.get(index) else {
            return false;
        };
        if block.kind == Kind::Gap {
            return false;
        }
        if self.focus.as_ref() == Some(&(block.start..block.end)) {
            return false;
        }
        self.set_focus(block.start, block.end, false)
    }

    pub(super) fn focus_below(&mut self) -> Edited {
        let grew = if self.source.is_empty() || self.source.ends_with('\n') {
            false
        } else {
            self.source.push('\n');
            true
        };
        let at = self.source.len();
        if self
            .focus
            .as_ref()
            .is_some_and(|range| range.start == at && range.end == at)
        {
            return Edited {
                changed: grew,
                refocus: false,
            };
        }
        self.focus = Some(at..at);
        self.editor = text_editor::Content::new();
        Edited {
            changed: grew,
            refocus: true,
        }
    }

    /// Flip a task box at `at`. The slice must be `[ ]`, `[x]`, or `[X]`.
    pub(super) fn toggle_task(&mut self, at: usize) -> bool {
        if self
            .focus
            .as_ref()
            .is_some_and(|range| at >= range.start && at < range.end)
        {
            return false;
        }
        let Some(window) = self.source.get(at..at.saturating_add(3)) else {
            return false;
        };
        let next = match window {
            "[ ]" => "[x]",
            "[x]" | "[X]" => "[ ]",
            _ => return false,
        };
        self.source.replace_range(at..at.saturating_add(3), next);
        true
    }

    fn splice(&mut self) {
        let Some(range) = self.focus.clone() else {
            return;
        };
        if range.end > self.source.len() || self.source.get(range.start..range.end).is_none() {
            self.focus = None;
            return;
        }
        let raw = self.editor.text();
        self.source.replace_range(range.start..range.end, &raw);
        self.focus = Some(range.start..range.start.saturating_add(raw.len()));
    }

    fn focus_caret_line(&mut self) -> bool {
        let Some(focus) = self.focus.clone() else {
            return false;
        };
        let (rel_start, rel_end) =
            line_span(&self.editor.text(), self.editor.cursor().position.line);
        let start = focus.start.saturating_add(rel_start);
        let end = focus.start.saturating_add(rel_end);
        if start == focus.start && end == focus.end {
            return false;
        }
        self.set_focus(start, end, false)
    }

    fn focus_sibling(&mut self, forward: bool) -> bool {
        let Some(focus) = self.focus.clone() else {
            return false;
        };
        let blocks = parse(&self.source);
        let found = if forward {
            blocks
                .iter()
                .find(|block| block.kind != Kind::Gap && block.start >= focus.end)
        } else {
            blocks
                .iter()
                .rev()
                .find(|block| block.kind != Kind::Gap && block.end <= focus.start)
        };
        let Some(block) = found else {
            return false;
        };
        self.set_focus(block.start, block.end, !forward)
    }

    fn merge(&mut self, forward: bool) -> bool {
        let Some(focus) = self.focus.clone() else {
            return false;
        };
        let blocks = parse(&self.source);
        let content: Vec<&Block> = blocks
            .iter()
            .filter(|block| block.kind != Kind::Gap)
            .collect();
        let pos = content
            .iter()
            .position(|block| block.start == focus.start && block.end == focus.end);
        let (left_start, left_end, right_start, right_end) = if forward {
            let Some(pos) = pos else {
                return false;
            };
            let Some(next) = content.get(pos.saturating_add(1)) else {
                return false;
            };
            (focus.start, focus.end, next.start, next.end)
        } else if let Some(pos) = pos {
            if pos == 0 {
                return false;
            }
            let Some(prev) = content.get(pos.saturating_sub(1)) else {
                return false;
            };
            (prev.start, prev.end, focus.start, focus.end)
        } else {
            let Some(prev) = content.iter().rev().find(|block| block.end <= focus.start) else {
                return false;
            };
            (prev.start, prev.end, focus.start, focus.end)
        };
        let left = self.source.get(left_start..left_end).unwrap_or("");
        let right = self.source.get(right_start..right_end).unwrap_or("");
        let mut joined = String::new();
        joined.push_str(left);
        let cursor_chars = joined.chars().count();
        joined.push_str(right);
        if self.source.get(left_start..right_end).is_none() {
            return false;
        }
        self.source.replace_range(left_start..right_end, &joined);
        self.focus = Some(left_start..left_start.saturating_add(joined.len()));
        self.editor = text_editor::Content::with_text(&joined);
        self.editor.perform(Action::Move(Motion::DocumentStart));
        for _ in 0..cursor_chars {
            self.editor.perform(Action::Move(Motion::Right));
        }
        true
    }

    fn set_focus(&mut self, start: usize, end: usize, cursor_at_end: bool) -> bool {
        if start > end || self.source.get(start..end).is_none() && start != end {
            return false;
        }
        let raw = self.source.get(start..end).unwrap_or("");
        self.focus = Some(start..end);
        self.editor = text_editor::Content::with_text(raw);
        if cursor_at_end {
            self.editor.perform(Action::Move(Motion::DocumentEnd));
        }
        true
    }
}

fn caret_at_start(editor: &text_editor::Content) -> bool {
    let cursor = editor.cursor();
    cursor.selection.is_none() && cursor.position.line == 0 && cursor.position.column == 0
}

fn caret_at_end(editor: &text_editor::Content) -> bool {
    let cursor = editor.cursor();
    if cursor.selection.is_some() {
        return false;
    }
    let last = editor.line_count().saturating_sub(1);
    if cursor.position.line != last {
        return false;
    }
    let (start, end) = line_span(&editor.text(), last);
    let line = editor.text().get(start..end).unwrap_or("").chars().count();
    cursor.position.column >= line
}

fn caret_inside_fence(editor: &text_editor::Content) -> bool {
    let line = editor.cursor().position.line;
    let mut open = false;
    for (index, raw) in editor.text().split('\n').enumerate() {
        if index == line {
            return open;
        }
        if fence_marker(raw) {
            open = !open;
        }
    }
    false
}

fn line_span(text: &str, line: usize) -> (usize, usize) {
    let mut start = 0;
    for (index, part) in text.split('\n').enumerate() {
        if index == line {
            return (start, start.saturating_add(part.len()));
        }
        start = start.saturating_add(part.len().saturating_add(1));
    }
    (text.len(), text.len())
}

pub(super) fn view(body: &NoteBody, ink: Ink) -> Element<'_, Message> {
    if body.source.is_empty() && body.focus.is_none() {
        return scrollable(
            mouse_area(
                text("Write in markdown. A line like ## Subtitle previews when you leave it.")
                    .size(16)
                    .color(ink.muted),
            )
            .on_press(Message::EditBelow),
        )
        .height(Length::Fill)
        .into();
    }
    let blocks = parse(&body.source);
    let mut lines = column![].spacing(0).width(Length::Fill);
    let mut editor_done = false;
    for (index, block) in blocks.iter().enumerate() {
        if block.kind == Kind::Gap {
            if block.end.saturating_sub(block.start) >= 2 {
                lines = lines.push(iced::widget::space().height(12));
            }
            continue;
        }
        let focused = body
            .focus
            .as_ref()
            .is_some_and(|range| range.start == block.start && range.end == block.end);
        if focused {
            lines = lines.push(source_editor(body));
            editor_done = true;
            continue;
        }
        let raw = body.source.get(block.start..block.end).unwrap_or("");
        lines = lines.push(preview(index, block, raw, ink));
    }
    if body.focus.is_some() && !editor_done {
        lines = lines.push(source_editor(body));
    }
    lines = lines.push(
        mouse_area(container(column![]).height(48).width(Length::Fill))
            .on_press(Message::EditBelow),
    );
    scrollable(lines).height(Length::Fill).into()
}

fn source_editor(body: &NoteBody) -> Element<'_, Message> {
    let raw = body.editor.text();
    let rows = u16::try_from(
        raw.lines()
            .count()
            .max(1)
            .saturating_add(usize::from(raw.ends_with('\n'))),
    )
    .unwrap_or(1);
    let height = f32::from(rows).mul_add(24.0, 28.0);
    let font = if raw.trim_start().starts_with("```") {
        Font::MONOSPACE
    } else {
        sans()
    };
    iced::widget::text_editor::TextEditor::new(&body.editor)
        .id(iced::widget::Id::new("note-body"))
        .height(Length::Fixed(height))
        .padding(10)
        .size(16)
        .font(font)
        .placeholder("Write in markdown")
        .on_action(Message::BodyEdited)
        .key_binding(binding)
        .style(editor_style)
        .into()
}

fn preview<'a>(index: usize, block: &Block, raw: &str, ink: Ink) -> Element<'a, Message> {
    let body = match block.kind {
        Kind::Heading(level) => heading(raw, level, ink),
        Kind::Paragraph => inline_text(raw, 16.0, ink.text, ink),
        Kind::Item => item(index, block.start, raw, ink),
        Kind::Quote => quote(raw, ink),
        Kind::Code => code(raw, ink),
        Kind::Rule => rule::horizontal(1).into(),
        Kind::Gap => column![].into(),
    };
    if matches!(block.kind, Kind::Item) {
        return body;
    }
    mouse_area(container(body).width(Length::Fill).padding([3, 2]))
        .on_press(Message::FocusBlock(index))
        .interaction(iced::mouse::Interaction::Text)
        .into()
}

fn heading<'a>(raw: &str, level: u8, ink: Ink) -> Element<'a, Message> {
    let text_body = heading_words(raw);
    let size = match level {
        1 => 32.0,
        2 => 26.0,
        3 => 22.0,
        _ => 18.0,
    };
    if text_body.is_empty() {
        text("Heading")
            .font(serif())
            .size(size)
            .color(ink.muted)
            .into()
    } else {
        text(text_body)
            .font(serif())
            .size(size)
            .color(ink.text)
            .into()
    }
}

fn heading_words(raw: &str) -> String {
    let trimmed = raw.trim_start();
    let hashes = trimmed.chars().take_while(|char| *char == '#').count();
    trimmed.get(hashes..).unwrap_or("").trim().to_owned()
}

fn quote<'a>(raw: &str, ink: Ink) -> Element<'a, Message> {
    let words = raw
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix('>').unwrap_or(trimmed);
            rest.strip_prefix(' ').unwrap_or(rest)
        })
        .collect::<Vec<_>>()
        .join("\n");
    row![
        text("│").color(ink.accent).size(16),
        inline_text(&words, 16.0, ink.text, ink)
    ]
    .spacing(8)
    .into()
}

fn code<'a>(raw: &str, ink: Ink) -> Element<'a, Message> {
    let inner = code_inner(raw);
    container(text(inner).font(Font::MONOSPACE).size(14).color(ink.text))
        .padding(10)
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(ink.field)),
            text_color: Some(ink.text),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: ink.hairline,
            },
            ..Default::default()
        })
        .into()
}

fn code_inner(raw: &str) -> String {
    let mut lines: Vec<&str> = raw.split('\n').collect();
    if lines.first().is_some_and(|line| fence_marker(line)) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|line| fence_closer(line)) {
        lines.pop();
    }
    lines.join("\n")
}

fn item<'a>(index: usize, block_start: usize, raw: &str, ink: Ink) -> Element<'a, Message> {
    let indent = raw.chars().take_while(|char| *char == ' ').count();
    let pad = f32::from(u8::try_from(indent.min(8)).unwrap_or(0)) * 10.0;
    let (marker, words, task) = split_item(raw);
    let words_el = if words.is_empty() {
        text("List").size(16).color(ink.muted).into()
    } else {
        inline_text(&words, 16.0, ink.text, ink)
    };
    let mark: Element<'a, Message> = if let Some((rel, checked)) = task {
        let at = block_start.saturating_add(rel);
        checkbox(checked)
            .size(16)
            .on_toggle(move |_| Message::ToggleTask(at))
            .into()
    } else {
        text(marker).size(16).color(ink.muted).into()
    };
    let words = mouse_area(container(words_el).width(Length::Fill))
        .on_press(Message::FocusBlock(index))
        .interaction(iced::mouse::Interaction::Text);
    row![mark, words]
        .spacing(8)
        .padding(iced::Padding {
            top: 2.0,
            right: 2.0,
            bottom: 2.0,
            left: pad,
        })
        .align_y(iced::Alignment::Center)
        .into()
}

fn split_item(raw: &str) -> (String, String, Option<(usize, bool)>) {
    let indent = raw.chars().take_while(|char| *char == ' ').count();
    let rest = raw.get(indent..).unwrap_or(raw);
    let marker_len = list_marker_len(rest).unwrap_or(0);
    let body = rest.get(marker_len..).unwrap_or("");
    let task = task_at(body).map(|(rel, checked)| {
        (
            indent.saturating_add(marker_len).saturating_add(rel),
            checked,
        )
    });
    let words = if let Some((rel, _)) = task_at(body) {
        body.get(rel.saturating_add(3)..)
            .unwrap_or("")
            .trim_start()
            .to_owned()
    } else {
        body.trim_start().to_owned()
    };
    let marker = if rest.starts_with(|char: char| char.is_ascii_digit()) {
        rest.get(..marker_len).unwrap_or("").trim().to_owned()
    } else {
        "–".to_owned()
    };
    (marker, words, task)
}

fn task_at(body: &str) -> Option<(usize, bool)> {
    let trimmed = body.trim_start();
    let lead = body.len().saturating_sub(trimmed.len());
    if let Some(rest) = trimmed.strip_prefix("[ ]")
        && (rest.is_empty() || rest.starts_with(' '))
    {
        return Some((lead, false));
    }
    if let Some(rest) = trimmed
        .strip_prefix("[x]")
        .or_else(|| trimmed.strip_prefix("[X]"))
        && (rest.is_empty() || rest.starts_with(' '))
    {
        return Some((lead, true));
    }
    None
}

fn inline_text<'a>(raw: &str, size: f32, color: Color, ink: Ink) -> Element<'a, Message> {
    if raw.is_empty() {
        return text(" ").size(size).into();
    }
    let spans = pieces(raw)
        .into_iter()
        .map(|piece| piece_span(piece, color, ink))
        .collect::<Vec<_>>();
    rich_text(spans)
        .size(size)
        .on_link_click(never)
        .width(Length::Fill)
        .into()
}

fn piece_span<'a>(
    piece: Piece,
    color: Color,
    ink: Ink,
) -> iced::widget::text::Span<'a, iced::Never, Font> {
    match piece {
        Piece::Text(value) => span(value).color(color).font(sans()),
        Piece::Code(value) => span(value)
            .font(Font::MONOSPACE)
            .color(ink.text)
            .background(Background::Color(ink.field))
            .padding([0, 3]),
        Piece::Strong(value) => span(value).color(color).font(sans_bold()),
        Piece::Em(value) => span(value).color(color).font(sans_italic()),
        Piece::Strike(value) => span(value)
            .color(ink.muted)
            .font(sans())
            .strikethrough(true),
        Piece::Link(value) => span(value).color(ink.accent).font(sans()).underline(true),
        Piece::Image(value) => span(value).color(ink.muted).font(sans()),
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

fn pieces(input: &str) -> Vec<Piece> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars.get(index) == Some(&'\\')
            && let Some(next) = chars.get(index.saturating_add(1))
        {
            buf.push(*next);
            index = index.saturating_add(2);
            continue;
        }
        if let Some((len, piece)) = take_piece(&chars, index) {
            if !buf.is_empty() {
                out.push(Piece::Text(std::mem::take(&mut buf)));
            }
            out.push(piece);
            index = index.saturating_add(len);
            continue;
        }
        if let Some(char) = chars.get(index) {
            buf.push(*char);
        }
        index = index.saturating_add(1);
    }
    if !buf.is_empty() {
        out.push(Piece::Text(buf));
    }
    out
}

fn take_piece(chars: &[char], index: usize) -> Option<(usize, Piece)> {
    if at(chars, index, "`") {
        let close = find_at(chars, index.saturating_add(1), "`")?;
        let text = chars_string(chars, index.saturating_add(1), close)?;
        return Some((
            close.saturating_add(1).saturating_sub(index),
            Piece::Code(text),
        ));
    }
    if at(chars, index, "![") {
        return take_link(chars, index, true);
    }
    if at(chars, index, "[") {
        return take_link(chars, index, false);
    }
    if at(chars, index, "~~") {
        let close = find_at(chars, index.saturating_add(2), "~~")?;
        let text = chars_string(chars, index.saturating_add(2), close)?;
        return Some((
            close.saturating_add(2).saturating_sub(index),
            Piece::Strike(text),
        ));
    }
    if at(chars, index, "**") || at(chars, index, "__") {
        let marker = if at(chars, index, "**") { "**" } else { "__" };
        let close = find_at(chars, index.saturating_add(2), marker)?;
        let text = chars_string(chars, index.saturating_add(2), close)?;
        return Some((
            close.saturating_add(2).saturating_sub(index),
            Piece::Strong(text),
        ));
    }
    if at(chars, index, "*") || (at(chars, index, "_") && emphasis_edge(chars, index)) {
        let marker = if at(chars, index, "*") { "*" } else { "_" };
        let close = find_at(chars, index.saturating_add(1), marker)?;
        if !at(chars, index, "*") && !emphasis_edge(chars, close) {
            return None;
        }
        let text = chars_string(chars, index.saturating_add(1), close)?;
        if text.is_empty() {
            return None;
        }
        return Some((
            close.saturating_add(1).saturating_sub(index),
            Piece::Em(text),
        ));
    }
    None
}

fn emphasis_edge(chars: &[char], index: usize) -> bool {
    let before = index.checked_sub(1).and_then(|at| chars.get(at)).copied();
    before.is_none_or(|char| !char.is_ascii_alphanumeric())
}

fn take_link(chars: &[char], index: usize, image: bool) -> Option<(usize, Piece)> {
    let label_at = if image {
        index.saturating_add(2)
    } else {
        index.saturating_add(1)
    };
    let end_label = find_at(chars, label_at, "](")?;
    let url_at = end_label.saturating_add(2);
    let end_url = find_at(chars, url_at, ")")?;
    let label = chars_string(chars, label_at, end_label)?;
    let shown = if label.is_empty() {
        if image {
            "Image".to_owned()
        } else {
            chars_string(chars, url_at, end_url)?
        }
    } else if image {
        format!("Image: {label}")
    } else {
        label
    };
    let piece = if image {
        Piece::Image(shown)
    } else {
        Piece::Link(shown)
    };
    Some((end_url.saturating_add(1).saturating_sub(index), piece))
}

fn chars_string(chars: &[char], start: usize, end: usize) -> Option<String> {
    Some(chars.get(start..end)?.iter().collect())
}

fn at(chars: &[char], index: usize, marker: &str) -> bool {
    let mark: Vec<char> = marker.chars().collect();
    chars.get(index..index.saturating_add(mark.len())) == Some(mark.as_slice())
}

fn find_at(chars: &[char], from: usize, marker: &str) -> Option<usize> {
    let mark: Vec<char> = marker.chars().collect();
    if mark.is_empty() {
        return None;
    }
    let mut index = from;
    while index.saturating_add(mark.len()) <= chars.len() {
        if chars.get(index..index.saturating_add(mark.len())) == Some(mark.as_slice()) {
            return Some(index);
        }
        index = index.saturating_add(1);
    }
    None
}

fn parse(source: &str) -> Vec<Block> {
    let lines = split_lines(source);
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(line) = lines.get(index) else {
            break;
        };
        if line.text.trim().is_empty() {
            push_gap(&mut blocks, line);
            index = index.saturating_add(1);
            continue;
        }
        if fence_marker(line.text) {
            // A bare ``` opens a fence. The closer is a later line of backticks only.
            let start = line.start;
            let mut last = index;
            index = index.saturating_add(1);
            while index < lines.len() {
                last = index;
                let closing = lines.get(index).is_some_and(|item| fence_closer(item.text));
                index = index.saturating_add(1);
                if closing {
                    break;
                }
            }
            let content_end = lines.get(last).map_or(start, |item| item.content_end);
            let line_end = lines.get(last).map_or(content_end, |item| item.end);
            blocks.push(Block {
                kind: Kind::Code,
                start,
                end: content_end,
            });
            if line_end > content_end {
                blocks.push(Block {
                    kind: Kind::Gap,
                    start: content_end,
                    end: line_end,
                });
            }
            continue;
        }
        if let Some(level) = heading_level(line.text) {
            push_line(&mut blocks, Kind::Heading(level), line);
            index = index.saturating_add(1);
            continue;
        }
        if is_rule(line.text) {
            push_line(&mut blocks, Kind::Rule, line);
            index = index.saturating_add(1);
            continue;
        }
        if list_marker_len(line.text.trim_start()).is_some() && leading_spaces(line.text) <= 8 {
            push_line(&mut blocks, Kind::Item, line);
            index = index.saturating_add(1);
            continue;
        }
        if is_quote(line.text) {
            push_line(&mut blocks, Kind::Quote, line);
            index = index.saturating_add(1);
            continue;
        }
        push_line(&mut blocks, Kind::Paragraph, line);
        index = index.saturating_add(1);
    }
    merge_gaps(blocks)
}

fn push_line(blocks: &mut Vec<Block>, kind: Kind, line: &Line<'_>) {
    blocks.push(Block {
        kind,
        start: line.start,
        end: line.content_end,
    });
    if line.end > line.content_end {
        blocks.push(Block {
            kind: Kind::Gap,
            start: line.content_end,
            end: line.end,
        });
    }
}

fn push_gap(blocks: &mut Vec<Block>, line: &Line<'_>) {
    blocks.push(Block {
        kind: Kind::Gap,
        start: line.start,
        end: line.end,
    });
}

fn merge_gaps(blocks: Vec<Block>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for block in blocks {
        if block.kind == Kind::Gap
            && let Some(last) = out.last_mut()
            && last.kind == Kind::Gap
        {
            last.end = block.end;
            continue;
        }
        out.push(block);
    }
    out
}

fn split_lines(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, char) in source.char_indices() {
        if char == '\n' {
            let text = source.get(start..index).unwrap_or("");
            lines.push(Line {
                start,
                content_end: index,
                end: index.saturating_add(1),
                text,
            });
            start = index.saturating_add(1);
        }
    }
    if start < source.len() {
        let text = source.get(start..).unwrap_or("");
        lines.push(Line {
            start,
            content_end: source.len(),
            end: source.len(),
            text,
        });
    }
    lines
}

fn heading_level(text: &str) -> Option<u8> {
    if leading_spaces(text) > 3 {
        return None;
    }
    let trimmed = text.trim_start();
    let hashes = trimmed.chars().take_while(|char| *char == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = trimmed.get(hashes..)?;
    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
        return u8::try_from(hashes).ok();
    }
    None
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

fn is_quote(text: &str) -> bool {
    leading_spaces(text) <= 3 && text.trim_start().starts_with('>')
}

fn leading_spaces(text: &str) -> usize {
    text.chars().take_while(|char| *char == ' ').count()
}

fn list_marker_len(rest: &str) -> Option<usize> {
    if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        return Some(2);
    }
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if (1..=9).contains(&digits) {
        let after = rest.get(digits..)?;
        if after.starts_with(". ") || after.starts_with(") ") {
            return Some(digits.saturating_add(2));
        }
    }
    None
}

fn fence_marker(text: &str) -> bool {
    leading_spaces(text) <= 3 && text.trim_start().starts_with("```")
}

fn fence_closer(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|char| char == '`')
}

#[cfg(test)]
mod tests {
    use super::{Kind, Piece, parse, pieces};

    #[test]
    fn subtitle_is_a_heading_without_losing_the_marks() -> Result<(), String> {
        let source = "## Subtitle\n";
        let blocks = parse(source);
        let heading = blocks
            .iter()
            .find(|block| block.kind == Kind::Heading(2))
            .ok_or("heading")?;
        let slice = source.get(heading.start..heading.end).ok_or("range")?;
        if slice != "## Subtitle" {
            return Err(slice.to_owned());
        }
        covered(source)?;
        Ok(())
    }

    #[test]
    fn lines_cover_the_source() -> Result<(), String> {
        covered(
            "## Subtitle\n\nHello **there**\n- [ ] milk\n- [x] eggs\n> quote\n---\n```\ncode\n```\n",
        )?;
        Ok(())
    }

    #[test]
    fn inline_marks() {
        assert_eq!(
            pieces("**bold** and *em* and `x` and [a](https://example.com)"),
            vec![
                Piece::Strong("bold".to_owned()),
                Piece::Text(" and ".to_owned()),
                Piece::Em("em".to_owned()),
                Piece::Text(" and ".to_owned()),
                Piece::Code("x".to_owned()),
                Piece::Text(" and ".to_owned()),
                Piece::Link("a".to_owned()),
            ]
        );
    }

    fn covered(source: &str) -> Result<(), String> {
        let blocks = parse(source);
        let mut seen = vec![false; source.len()];
        for block in &blocks {
            if block.end > source.len() || block.start > block.end {
                return Err("out of range".to_owned());
            }
            for index in block.start..block.end {
                let slot = seen.get_mut(index).ok_or("index")?;
                if *slot {
                    return Err(format!("overlap at {index}"));
                }
                *slot = true;
            }
        }
        if seen.iter().any(|slot| !*slot) {
            return Err("uncovered".to_owned());
        }
        Ok(())
    }
}
