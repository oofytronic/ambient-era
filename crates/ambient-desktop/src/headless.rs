//! Real frames, without a window.
//!
//! The other tests in this crate build the views and look at the state. They cannot tell a menu
//! that is drawn from one that is not, or a button that is on screen from one off its edge. These
//! lay out, draw, and click the whole window on the software renderer, at a size they choose.
//!
//! Text is measured with the fonts the program ships. Nothing is rasterized, so this checks
//! where things are and what a press does, not how they look. Look at the window for that.

use std::borrow::Cow;

use iced::advanced::clipboard;
use iced::advanced::layout::{Limits, Node};
use iced::advanced::renderer;
use iced::advanced::widget::operation::Scrollable;
use iced::advanced::widget::{Operation, Tree};
use iced::{Color, Event, Font, Pixels, Point, Rectangle, Size, Vector, mouse};
use iced_runtime::user_interface::{Cache, UserInterface};

use super::app_tests::{folder, open_app, pinned, summary, wide};
use super::{App, Message, Screen, TaskView};

/// A software renderer that has the program's fonts.
pub(super) struct Headless {
    renderer: iced::Renderer,
}

impl Headless {
    pub(super) fn new() -> Self {
        let renderer =
            iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(16.0)));
        for bytes in [
            super::SANS_BYTES,
            super::SANS_BOLD_BYTES,
            super::SANS_ITALIC_BYTES,
            super::SERIF_BYTES,
        ] {
            iced::advanced::graphics::text::font_system()
                .write()
                .expect("the font system")
                .load_font(Cow::Borrowed(bytes));
        }
        Self { renderer }
    }

    /// Lay out and draw one frame at `size`.
    pub(super) fn draw(&mut self, app: &App, size: Size) {
        let theme = super::theme(app);
        let mut ui = UserInterface::build(app.view(), size, Cache::new(), &mut self.renderer);
        ui.draw(
            &mut self.renderer,
            &theme,
            &renderer::Style {
                text_color: Color::BLACK,
            },
            mouse::Cursor::Unavailable,
        );
    }

    /// Press and release the left button at `at`, and give back the messages that made.
    pub(super) fn click(&mut self, app: &App, size: Size, at: Point) -> Vec<Message> {
        let theme = super::theme(app);
        let mut ui = UserInterface::build(app.view(), size, Cache::new(), &mut self.renderer);
        let mut messages = Vec::new();
        let cursor = mouse::Cursor::Available(at);
        // Draw first, as the window does, so what floats has been laid out.
        ui.draw(
            &mut self.renderer,
            &theme,
            &renderer::Style {
                text_color: Color::BLACK,
            },
            cursor,
        );
        let events = [
            Event::Mouse(mouse::Event::CursorMoved { position: at }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ];
        let _ = ui.update(
            &events,
            cursor,
            &mut self.renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        messages
    }

    /// Layout of the whole window, and the content areas of its scroll areas, which are allowed
    /// to be bigger than what shows.
    pub(super) fn layout(&mut self, app: &App, size: Size) -> (Node, Vec<Rectangle>) {
        let mut element = app.view();
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &self.renderer,
            &Limits::new(Size::ZERO, size),
        );
        let mut scrollers = Scrollers(Vec::new());
        element.as_widget_mut().operate(
            &mut tree,
            iced::advanced::Layout::new(&node),
            &self.renderer,
            &mut scrollers,
        );
        (node, scrollers.0)
    }
}

/// Collects where the scroll areas of a window put their content.
struct Scrollers(Vec<Rectangle>);

impl Operation for Scrollers {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: Rectangle,
        content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        self.0.push(content_bounds);
    }
}

/// Boxes in the layout that end to the right of the window, as `x..right` and how deep they are.
/// A scroll area's content is allowed to, so this reports the first box on each branch that does,
/// and the caller decides which of them matter.
fn past_the_right_edge(
    node: &Node,
    at: Point,
    window: f32,
    scrollers: &[Rectangle],
    depth: usize,
    out: &mut Vec<String>,
) {
    let bounds = node.bounds();
    let (x, y) = (at.x + bounds.x, at.y + bounds.y);
    let inside_a_scroll_area = scrollers.iter().any(|area| {
        x >= area.x - 1.0
            && x + bounds.width <= area.x + area.width + 1.0
            && y >= area.y - 1.0
            && y + bounds.height <= area.y + area.height + 1.0
    });
    if x + bounds.width > window + 1.0 && bounds.width > 0.0 && !inside_a_scroll_area {
        out.push(format!(
            "depth {depth}: {:.0}..{:.0} of {window:.0}, {:.0} high, {} children, first child {:?}",
            x,
            x + bounds.width,
            bounds.height,
            node.children().len(),
            node.children()
                .first()
                .map(|child| (child.bounds().width.round(), child.children().len()))
        ));
        return;
    }
    for child in node.children() {
        past_the_right_edge(child, Point::new(x, y), window, scrollers, depth + 1, out);
    }
}

const WIDTHS: [f32; 9] = [
    1400.0, 1100.0, 1019.0, 900.0, 760.0, 719.0, 600.0, 420.0, 340.0,
];

fn app_in(name: &str) -> (App, std::path::PathBuf) {
    let dir = folder(name);
    let mut app = open_app(&dir);
    pinned(&mut app, 10 * 60);
    app.catalog = vec![summary("a", &["Novel"]), summary("b", &[])];
    let _ = app.update(Message::TitleChanged("A note".to_owned()));
    (app, dir)
}

#[test]
fn every_screen_lays_out_and_draws_at_every_width() {
    let _turn = super::tests_support::one_at_a_time();
    let (mut app, dir) = app_in("headless-draw");
    let mut screen = Headless::new();
    for width in WIDTHS {
        let size = Size::new(width, 760.0);
        let _ = app.update(Message::Resized(size));
        for show in [
            Message::ShowNotes,
            Message::ShowTasks,
            Message::ShowChat,
            Message::ShowSettings,
            Message::ShowInvite,
        ] {
            let _ = app.update(show);
            screen.draw(&app, size);
        }
        // The cards that take the keyboard, over whichever page is behind them.
        for open in [
            Message::PaletteOpen,
            Message::CaptureOpen,
            Message::TemplatesOpen,
            Message::RenameGroupOpen("novel".to_owned()),
        ] {
            let _ = app.update(Message::ShowNotes);
            let _ = app.update(open);
            assert!(app.overlay_open());
            screen.draw(&app, size);
            let _ = app.update(Message::CloseOverlays);
        }
        // The Tasks tab with a late task, a priority, a task with subtasks, and the tools out.
        let row = |line: usize, depth: usize, parent: Option<usize>, text: &str| super::TaskRow {
            note_id: "tasks".to_owned(),
            note_title: "Tasks".to_owned(),
            line,
            done: false,
            due: super::due::Due::of(text),
            priority: super::markdown::task_priority(text),
            depth,
            parent,
            kids: (0, usize::from(line == 0) * 2),
            text: text.to_owned(),
        };
        app.tasks = vec![
            row(
                0,
                0,
                None,
                "plan a trip with a rather long name to wrap !1 2026-01-01",
            ),
            row(1, 1, Some(0), "book the hotel"),
            row(2, 1, Some(0), "pack !2"),
            row(3, 0, None, "water plants !3 2099-01-01 repeat:weekly"),
        ];
        for view in [super::TaskView::Today, super::TaskView::Open] {
            let _ = app.update(Message::ShowTaskView(view));
            let _ = app.update(Message::ShowTasks);
            screen.draw(&app, size);
            let _ = app.update(Message::RowHover("tasks:0".to_owned(), true));
            let _ = app.update(Message::SnoozeToggle {
                id: "tasks".to_owned(),
                line: 0,
            });
            let _ = app.update(Message::SubtaskOpen {
                id: "tasks".to_owned(),
                line: 0,
            });
            screen.draw(&app, size);
            let _ = app.update(Message::SubtaskCancel);
            let _ = app.update(Message::SnoozeToggle {
                id: "tasks".to_owned(),
                line: 0,
            });
            let _ = app.update(Message::RowHover("tasks:0".to_owned(), false));
        }
        // The graph, with a group in view and zoomed in, and the panel with its list of facts.
        let _ = app.update(Message::ShowNotes);
        let _ = app.update(Message::ToggleGraph);
        screen.draw(&app, size);
        let _ = app.update(Message::GraphCamera(super::GraphCam {
            zoom: 2.5,
            pan: (40.0, -30.0),
        }));
        screen.draw(&app, size);
        let _ = app.update(Message::ToggleGraph);
        let _ = app.update(Message::ToggleNoteSettings);
        let _ = app.update(Message::ToggleProps);
        screen.draw(&app, size);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_menu_under_a_button_is_drawn_and_a_press_elsewhere_closes_it() {
    let _turn = super::tests_support::one_at_a_time();
    let (mut app, dir) = app_in("headless-anchored");
    let mut screen = Headless::new();
    let size = Size::new(1100.0, 760.0);
    let _ = app.update(Message::Resized(size));
    let nowhere = Point::new(400.0, 300.0);

    // The sync card hangs under the globe. It is an overlay, so a press that is not on it is the
    // overlay's to take and send `ToggleSyncPop`. Before overlays were passed up through the
    // wheel wrapper, nothing took the press, and nothing was drawn.
    let _ = app.update(Message::ToggleSyncPop);
    assert!(app.sync_open);
    screen.draw(&app, size);
    let sent = screen.click(&app, size, nowhere);
    assert!(
        sent.iter()
            .any(|message| matches!(message, Message::ToggleSyncPop)),
        "the sync card was not there to be dismissed: {sent:?}"
    );
    let _ = app.update(Message::ToggleSyncPop);

    // The same for the Cite menu of the add row and for the calendar.
    let _ = app.update(Message::ShowTasks);
    let _ = app.update(Message::ToggleTaskCite);
    screen.draw(&app, size);
    let sent = screen.click(&app, size, nowhere);
    assert!(
        sent.iter()
            .any(|message| matches!(message, Message::ToggleTaskCite)),
        "{sent:?}"
    );
    let _ = app.update(Message::ToggleTaskCite);
    let _ = app.update(Message::OpenCalendar(super::CalFor::New));
    screen.draw(&app, size);
    let sent = screen.click(&app, size, nowhere);
    assert!(
        sent.iter()
            .any(|message| matches!(message, Message::CloseCalendar)),
        "{sent:?}"
    );

    // With nothing open, the same press does none of those.
    let _ = app.update(Message::CloseCalendar);
    let sent = screen.click(&app, size, nowhere);
    assert!(
        !sent.iter().any(|message| matches!(
            message,
            Message::ToggleSyncPop | Message::ToggleTaskCite | Message::CloseCalendar
        )),
        "{sent:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_card_over_the_page_closes_on_a_press_beside_it_and_not_on_it() {
    let _turn = super::tests_support::one_at_a_time();
    let (mut app, dir) = app_in("headless-modal");
    let mut screen = Headless::new();
    let size = Size::new(420.0, 760.0);
    let _ = app.update(Message::Resized(size));
    let _ = app.update(Message::ShowTasks);
    let _ = app.update(Message::ShowTaskView(TaskView::Open));
    let _ = app.update(Message::OpenCalendar(super::CalFor::New));
    assert!(app.modal().is_some());
    screen.draw(&app, size);
    // The corner is the dimmed page, and a press on it closes the card.
    let sent = screen.click(&app, size, Point::new(4.0, 4.0));
    assert!(
        sent.iter()
            .any(|message| matches!(message, Message::CloseCalendar)),
        "the dimmed page did not close the card: {sent:?}"
    );
    // The middle is the card, and a press there is the card's own.
    let sent = screen.click(&app, size, Point::new(210.0, 380.0));
    assert!(
        !sent
            .iter()
            .any(|message| matches!(message, Message::CloseCalendar)),
        "a press on the card closed it: {sent:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_right_panel_closes_from_the_dimmed_page_when_it_is_a_sheet() {
    let _turn = super::tests_support::one_at_a_time();
    let (mut app, dir) = app_in("headless-sheet");
    let mut screen = Headless::new();
    let size = Size::new(900.0, 760.0);
    let _ = app.update(Message::Resized(size));
    let _ = app.update(Message::ShowNotes);
    let _ = app.update(Message::ToggleContext);
    assert!(app.context_sheet().is_some());
    screen.draw(&app, size);
    let sent = screen.click(&app, size, Point::new(300.0, 300.0));
    assert!(
        sent.iter()
            .any(|message| matches!(message, Message::ToggleContext)),
        "{sent:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_important_runs_past_the_right_edge_of_the_window() {
    let _turn = super::tests_support::one_at_a_time();
    let (mut app, dir) = app_in("headless-overflow");
    let mut screen = Headless::new();
    let mut found: Vec<String> = Vec::new();
    for width in WIDTHS {
        let size = Size::new(width, 760.0);
        let _ = app.update(Message::Resized(size));
        wide_or_not(&mut app, width);
        for (name, show) in [
            ("notes", Message::ShowNotes),
            ("tasks", Message::ShowTasks),
            ("chat", Message::ShowChat),
            ("settings", Message::ShowSettings),
        ] {
            let _ = app.update(show);
            let (root, scrollers) = screen.layout(&app, size);
            let mut edge = Vec::new();
            past_the_right_edge(&root, Point::ORIGIN, width, &scrollers, 0, &mut edge);
            for line in edge {
                found.push(format!("{name} at {width:.0}: {line}"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        found.is_empty(),
        "past the right edge:\n{}",
        found.join("\n")
    );
}

fn wide_or_not(app: &mut App, width: f32) {
    if width >= 720.0 {
        wide(app);
        let _ = app.update(Message::Resized(Size::new(width, 760.0)));
    }
}

#[test]
fn the_screens_are_the_ones_the_sweep_names() {
    // The sweep above shows these by name. If a screen is added, add it there too.
    let _ = (Screen::Notes, Screen::Tasks, Screen::Chat);
}

#[test]
fn the_edge_check_sees_a_box_that_runs_off_and_lets_a_scroll_area_have_its_content() {
    let wide_child = Node::with_children(
        Size::new(100.0, 10.0),
        vec![Node::new(Size::new(300.0, 10.0))],
    );
    let mut found = Vec::new();
    past_the_right_edge(&wide_child, Point::ORIGIN, 100.0, &[], 0, &mut found);
    assert_eq!(found.len(), 1, "{found:?}");
    // The same box inside what a scroll area says is its content is allowed.
    let mut found = Vec::new();
    let content = Rectangle::new(Point::ORIGIN, Size::new(300.0, 10.0));
    past_the_right_edge(&wide_child, Point::ORIGIN, 100.0, &[content], 0, &mut found);
    assert!(found.is_empty(), "{found:?}");
}
