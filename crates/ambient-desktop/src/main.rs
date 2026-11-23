//! Linux notes window.
//!
//! Iced draws the controls in a system window. There is no webview. The folder
//! is the same one the `ambient` CLI uses. Pass a path, or it uses `./notebook`.
//! Sync, pairing, and chat call `ambient-cli`. This file does not seal events.
//! The relay it can start is cleartext on `127.0.0.1` only.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ambient_cli::{Draft, NoteSummary, Notebook};
use iced::clipboard;
use iced::keyboard::Key;
use iced::widget::canvas;
use iced::widget::image as iced_image;
use iced::widget::{
    button, checkbox, column, container, mouse_area, row, scrollable, space, stack, text,
    text_editor, text_input, tooltip,
};
use iced::{Color, Element, Fill, Font, Size, Task, Theme};
use image::ImageEncoder;
use qrcode::QrCode;

mod clock;
mod dates;
mod due;
mod files;
mod lane;
mod markdown;
mod profile;
mod zone;

use markdown::FormatKind;

const SANS_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Regular.ttf");
const SANS_BOLD_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Bold.ttf");
const SANS_ITALIC_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Italic.ttf");
const SERIF_BYTES: &[u8] = include_bytes!("../fonts/CrimsonPro-Regular.ttf");

fn sans() -> Font {
    Font::with_name("DM Sans 9pt")
}

fn serif() -> Font {
    Font::with_name("Crimson Pro")
}

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window_size((1280.0, 820.0))
        .theme(theme)
        .default_font(sans())
        .font(SANS_BYTES)
        .font(SANS_BOLD_BYTES)
        .font(SANS_ITALIC_BYTES)
        .font(SERIF_BYTES)
        .run()
}

fn theme(app: &App) -> Theme {
    let ink = Ink::from_dark(app.is_dark());
    Theme::custom(
        if app.is_dark() {
            "parchment-dark"
        } else {
            "parchment-light"
        },
        iced::theme::Palette {
            background: ink.bg,
            text: ink.text,
            primary: ink.accent,
            success: Color::from_rgb8(46, 140, 96),
            warning: Color::from_rgb8(176, 122, 48),
            danger: Color::from_rgb8(184, 78, 62),
        },
    )
}

#[allow(clippy::struct_excessive_bools)]
struct App {
    dir: PathBuf,
    notebook: Option<Notebook>,
    /// Generation of `notebook`. An older snapshot is never installed over a newer one.
    book_gen: u64,
    search_hits: Option<Vec<NoteSummary>>,
    open_error: String,
    catalog: Vec<NoteSummary>,
    notes: Vec<NoteSummary>,
    tasks: Vec<TaskRow>,
    query: String,
    tag: Option<String>,
    selected: Option<String>,
    title_text: String,
    body: markdown::NoteBody,
    conflicts: Vec<(String, String, String)>,
    note_writer: String,
    message: String,
    screen: Screen,
    chats: Vec<ambient_cli::ConversationSummary>,
    chat_id: Option<String>,
    devices: Vec<ambient_cli::DeviceLine>,
    chat: Vec<ChatBubble>,
    draft_editor: text_editor::Content,
    my_card: String,
    their_card: String,
    /// Link pasted to add someone to the open conversation.
    add_draft: String,
    /// The add field is open.
    adding: bool,
    /// A name for the person being added. This computer only.
    add_call: String,
    relay_ready: bool,
    relay_ours: bool,
    sync_open: bool,
    sync_detail: String,
    pending: bool,
    net_mode: ambient_cli::NetMode,
    pin_draft: String,
    backup_draft: String,
    advanced: bool,
    net_up: bool,
    unsaved: Vec<(Option<String>, String, String)>,
    disk_title: String,
    disk_body: String,
    recovery_phrase: String,
    restore_phrase: String,
    contact_link: String,
    /// This folder's contact signing key, hex.
    contact_key: String,
    show_qr: bool,
    qr_bytes: Option<Vec<u8>>,
    unlock_input: String,
    sheet: Sheet,
    appearance: profile::Appearance,
    system_dark: bool,
    loaded: bool,
    saving: bool,
    save_again: bool,
    save_token: u64,
    your_name: String,
    /// One line on the card. This computer offers it with the name.
    card_line: String,
    device_draft: String,
    name_draft: String,
    line_draft: String,
    call_draft: String,
    wipe_draft: String,
    aliases: BTreeMap<String, String>,
    /// A name for a person's signing key. This computer only. Not the conversation's name.
    people: BTreeMap<String, String>,
    /// The one line a person's card offered. Keyed by the same signing key. This computer only.
    card_lines: BTreeMap<String, String>,
    /// Words this person added to the spellchecker. Kept on this computer, in `dictionary`.
    my_words: BTreeSet<String>,
    /// Where backups go, as typed. Empty until the person chooses, and then the default is used.
    backup_to: String,
    /// When the last backup finished, in unix milliseconds, or zero.
    backup_at: u64,
    /// A copy is being made. It holds both locks, so a second one waits for this one.
    backup_busy: bool,
    syncing: bool,
    edited_at: Option<Instant>,
    /// Bumps when the pause wait starts over, so a late wake cannot save another note.
    pause_token: u64,
    pause_armed: bool,
    /// Bumps when the search wait starts over. Only the latest query is run.
    search_token: u64,
    search_armed: bool,
    scroll_token: u64,
    scroll_armed: bool,
    writes: u64,
    size: Size,
    detail: bool,
    pending_files: Vec<PathBuf>,
    task_draft: String,
    confirm_delete: bool,
    send_open: bool,
    send_choice: Option<String>,
    boot_at: Instant,
    breeze_frame: u32,
    /// Byte where the current drag started. Empty when the mouse is up.
    drag_from: Option<usize>,
    /// Previous press: when, byte, and how many clicks landed on that line.
    last_press: Option<(Instant, usize, u8)>,
    /// Steps for the note that is open. Not written into the log.
    history: NoteHistory,
    /// Set after a new folder is created, until the welcome is finished.
    welcome: Option<WelcomeStep>,
    /// False only until the first welcome is finished. Older folders stay true.
    seen: bool,
    task_query: String,
    /// Note id and checkbox line being renamed in the task list.
    task_rename: Option<(String, usize)>,
    task_rename_text: String,
    /// Last line and clock label for each conversation. Display only.
    chat_last: BTreeMap<String, (String, String)>,
    /// How many messages each conversation had last time the list was read.
    chat_count: BTreeMap<String, usize>,
    /// How many this computer has already looked at. A local file, not the log.
    seen_count: BTreeMap<String, usize>,
    chat_query: String,
    hide_done: bool,
    confirm_clear: bool,
    /// The Tasks note is shown over the Tasks tab.
    tasks_note_open: bool,
    /// The due moment of the task being edited.
    edit_due: DueDraft,
    /// The due moment picked for the task being added.
    new_due: DueDraft,
    /// Whose due date the calendar is choosing, when it is open.
    cal: Option<CalFor>,
    /// The month the calendar is showing.
    cal_month: (i32, u32),
    /// What the Tasks tab is showing.
    task_view: TaskView,
    /// What the clock said when it was last read: the day, the minute, and the zone. Everything
    /// on a screen is worked out from this one reading, so it all agrees about now.
    clock: clock::Clock,
    /// The zone file that reading used, kept to tell when the system's zone changes.
    zone: zone::Zone,
    /// The zone's name, when the system gave one.
    zone_name: Option<String>,
    /// Times read `3:42 PM` when true and `15:42` when false.
    hour12: bool,
    /// Weeks begin on Monday in the calendar when true, on Sunday when false.
    week_monday: bool,
    /// The zone the person lives in, by name. Empty until the first named zone is seen.
    home_zone: String,
    /// Set while this computer is somewhere other than home.
    trip: Option<clock::Trip>,
    /// The zone a trip notice was put away for, so it does not come back until the zone changes.
    trip_dismissed: Option<String>,
    /// A reading to keep. Only tests set it, so they do not depend on the day they run.
    pinned_clock: Option<clock::Clock>,
    find_open: bool,
    find_query: String,
    /// Index of the current find hit. `usize::MAX` means the next jump starts at the first.
    find_at: usize,
    command_down: bool,
    shift_down: bool,
    /// True only while the button that started a selection is still down.
    pointer_down: bool,
    /// Where that press landed. A small move is still a click.
    press_x: f32,
    press_y: f32,
    /// The pointer moved far enough to be a drag, not a click.
    dragged: bool,
    /// This press already chose a word, a line, or a shift range.
    kept_click: bool,
    note_search: bool,
    task_search: bool,
    chat_search: bool,
    /// Writing with nothing else in view: the panels are away and the title is small.
    focus: bool,
    /// The right panel shows this note's settings, not its links.
    note_panel: bool,
    /// Groups the owner has folded in the list. Only for this run.
    folded: BTreeSet<String>,
    /// The order notes were put in by hand, per group key. A note not listed comes after, in
    /// alphabetical order. On this computer only.
    orders: BTreeMap<String, Vec<String>>,
    /// A note being dragged to a new place in its group.
    list_drag: Option<ListDrag>,
    /// A selection waiting for a chat to be chosen: its words and the lines it covers.
    send_part: Option<(String, usize, usize)>,
    /// The note whose row the pointer is over, so its delete button shows.
    hover_row: Option<String>,
    /// The formatting bar, while writing in focus. Hidden until pulled in.
    slide_bar: Slide,
    /// The list and the group panel, with their edge handles, in and out of focus.
    slide_panels: Slide,
    /// The conversation whose name is being typed. Otherwise the name is plain text.
    alias_editing: Option<String>,
    invite_pick: Option<InvitePick>,
    /// Undo for notes that are not on screen. Still memory, still not the log.
    histories: BTreeMap<String, NoteHistory>,
    history_order: Vec<String>,
    history_key: String,
    /// A group's notes drawn as one reading, not the editor.
    reading: bool,
    /// The group being read, as written in its tag.
    read_title: String,
    /// A missing chapter to create once the open note finishes saving.
    pending_chapter: Option<String>,
    /// The note list on the left. Memory only.
    show_list: bool,
    /// The chapter panel on the right. Memory only.
    show_context: bool,
    /// The chapter panel was closed by hand, so a group does not open it again.
    context_shut: bool,
    /// Tags on the open note. Not words in the writing.
    tags: Vec<String>,
    tag_draft: String,
    /// The notes the open note links to, as id and title.
    links_to: Vec<(String, String)>,
    /// Every link between two notes, from id to id.
    graph_edges: Vec<(String, String)>,
    /// The graph is shown in place of the note.
    graph_open: bool,
    /// The graph's dots and lines, laid out. Built when it is opened and when the list changes.
    graph: Option<Arc<GraphData>>,
    cached_chapters: Vec<Chapter>,
    cached_backlinks: Vec<(String, String)>,
    /// Ids and lowercase titles a note link can open. Empty until the list is read.
    known_links: Option<Arc<BTreeSet<String>>>,
    /// Body last sent to the proof job. Unchanged text is not sent again.
    proof_body: String,
    /// One quiet line. Empty when the marks are closed.
    proof_warning: String,
    /// Misspelled words from the last proof. The caret thread only reads this.
    misspellings: Arc<BTreeSet<String>>,
    /// Matching line for each note id from the last search.
    search_lines: BTreeMap<String, String>,
    /// Conversations whose lines matched the note search.
    search_chats: Vec<lane::ChatHit>,
    /// An older snapshot on screen. It is not the editor and it is not saved.
    older: Option<OlderNote>,
    /// Note id of a version open that has not arrived yet.
    version_wait: Option<String>,
    /// Note cited by the next chat message.
    chat_cite: Option<(String, String)>,
    cite_open: bool,
    /// Note cited by the next task.
    task_cite: Option<(String, String)>,
    task_cite_open: bool,
    /// Background folder work. The caret's thread only posts a job.
    lane: std::sync::mpsc::Sender<lane::Job>,
}

/// Which side of an invite this computer is on. One path, not both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InvitePick {
    Theirs,
    Mine,
}

/// One wait. A new token drops the previous wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Wake {
    Pause(u64),
    Search(u64),
    /// The last scroll was a moment ago, so the scrollbar can go away.
    Scroll(u64),
}

#[derive(Clone, Debug)]
enum Message {
    QueryChanged(String),
    PickTag(Option<String>),
    Select(String),
    NewNote,
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    /// Press on a letter. A click puts the caret there. A drag starts from there.
    SelectFrom {
        byte: usize,
        x: f32,
        y: f32,
    },
    /// Drag across letters, once the pointer has actually moved.
    ExtendTo {
        byte: usize,
        x: f32,
        y: f32,
    },
    /// The mouse button lifted.
    SelectStop,
    SetOpenTask {
        line: usize,
        done: bool,
    },
    CopyNote,
    CutNote,
    Save,
    ShowNotes,
    ShowTasks,
    ShowChat,
    ToggleList,
    ToggleContext,
    TagDraft(String),
    /// A tag offered under the tag field.
    PickSuggestion(String),
    AddTag,
    RemoveTag(String),
    CompleteLink(String),
    CompleteVersion {
        id: String,
        version: String,
    },
    CloseVersion,
    OpenCited(String),
    ToggleChatCite,
    PickChatCite(String),
    ClearChatCite,
    ToggleTaskCite,
    PickTaskCite(String),
    ClearTaskCite,
    Lane(lane::Done),
    AutoSync,
    ToggleSyncPop,
    ToggleRelay,
    ToggleAdvanced,
    PinDraft(String),
    BackupDraft(String),
    UseThisComputer,
    SavePinned,
    OpenPaired,
    OpenPerson,
    OpenOneMore,
    AddDraft(String),
    AddCall(String),
    ToggleAdd,
    AddToChat,
    PersonDraft(String, String),
    #[allow(dead_code)]
    CopyCard,
    #[allow(dead_code)]
    MyCardChanged(String),
    TheirCardChanged(String),
    StartConversation,
    DraftEdited(text_editor::Action),
    SendChat,
    ShowPhrase,
    CopyPhrase,
    RestorePhraseChanged(String),
    StartRestore,
    CopyLink,
    ToggleQr,
    SendNoteToChat,
    UnlockChanged(String),
    SubmitUnlock,
    ShowInvite,
    ShowSettings,
    CloseSheet,
    SelectChat(String),
    SetTask {
        id: String,
        line: usize,
        done: bool,
    },
    Revoke(String),
    ToggleTheme,
    Resized(Size),
    FileDropped(PathBuf),
    PickFile,
    FilePicked(Result<PathBuf, String>),
    DropPending(usize),
    OpenPath(PathBuf),
    Format(FormatKind),
    /// Tab or Shift+Tab in the writing: nest the list items under the caret, or bring them out.
    ShiftItems {
        back: bool,
    },
    PasteNote,
    Pasted(Option<String>),
    ClearSelect,
    ToggleRead,
    ChapterMade(Result<MadeChapter, String>),
    TaskDraft(String),
    AddTask,
    AskDelete,
    CancelDelete,
    DeleteNote,
    Deleted(Result<Vec<NoteSummary>, String>),
    ChooseSend(String),
    ConfirmSend,
    CancelSend,
    Breeze,
    Woke(Wake),
    /// The first wheel tick of a burst. Starts the wake that hides the scrollbar again.
    Scrolled,
    Opened(Result<(), String>),
    Saved(
        u64,
        Result<(String, Vec<NoteSummary>), String>,
        String,
        String,
    ),
    Applied(Result<Vec<NoteSummary>, String>),
    DeviceDraft(String),
    NameDraft(String),
    LineDraft(String),
    FinishOnboarding,
    PickAppearance(profile::Appearance),
    CallDraft(String),
    AliasDraft(String, String),
    /// The pencil beside a conversation's name: start typing it, or stop.
    EditAlias,
    FinishAlias,
    /// The icon at the top right of a note: write with nothing else in view, or come back.
    ToggleFocus,
    /// Enter in the title: start writing.
    TitleSubmit,
    /// Ctrl or Cmd + Shift + D: tell the spellchecker the word at the caret is fine.
    AddWord,
    /// Enter in a field that has nothing to submit. It exists so the key is taken by the field and
    /// does not fall through to the page.
    FieldDone,
    /// A key that nothing handled, while the writing page is up. See `ignored_key`.
    TypedIgnored(Typed),
    /// The text in the box that says where a backup goes.
    BackupTo(String),
    /// Copy this notebook's folder to another place.
    BackUp,
    /// A copy finished, or failed.
    BackedUp(Result<profile::BackupReport, String>),
    /// The Send button in the bar over a selection: send only those words, with where they are from.
    SendSelection,
    /// Fold or open a group in the list.
    Fold(String),
    /// The grip on a note in a group was pressed: start moving it.
    DragStart {
        group: String,
        id: String,
    },
    /// Show the graph of notes and links in place of the note, or go back.
    ToggleGraph,
    /// A dot in the graph was clicked.
    GraphPick(String),
    /// The line of link counts under the title: open the right panel on the links.
    ShowLinks,
    /// Open the calendar for this due date, or shut it if it is already open for it.
    OpenCalendar(CalFor),
    CloseCalendar,
    /// Show the month this many months on, or back.
    CalMonth(i32),
    /// A day was chosen in the calendar, as `YYYY-MM-DD`.
    PickDay(String),
    /// No due date.
    ClearDue,
    /// A day counted from today: 0 is today, 1 tomorrow.
    PickQuick(i64),
    /// Move the time on by this many hours and minutes.
    StepTime {
        hours: i32,
        minutes: i32,
    },
    /// A time of day as `HH:MM`, or empty for none.
    SetTime(String),
    SetRepeat(Option<due::Repeat>),
    /// Make the repeat come round every this many more or fewer units.
    RepeatEvery(i32),
    /// Read times on a twelve hour clock (true) or a twenty four hour one (false).
    PickHour12(bool),
    /// Start the calendar's week on Monday (true) or Sunday (false).
    PickWeekMonday(bool),
    /// This computer's zone is home from now on.
    SetHomeHere,
    /// Put the notice that this computer is away from home away until the zone changes.
    DismissTrip,
    /// Time has passed, or the window was brought back. Read the clock again.
    ClockTick,
    /// Look at the tasks this way.
    ShowTaskView(TaskView),
    /// Go back from a note, a conversation, a list of tasks, or the graph to the list it came from,
    /// on a narrow window.
    BackToList,
    /// Show or close the Tasks note over the Tasks tab.
    ToggleTasksNote,
    /// The icon at the top of the right panel: this note's settings, or its links.
    ToggleNoteSettings,
    /// The pointer went onto (true) or off (false) a note's row.
    RowHover(String, bool),
    /// The delete button on a note's row.
    AskDeleteNote(String),
    /// The button beside the small title in focus: pull the formatting bar in, or send it away.
    FocusBar,
    /// A frame of a slide. It changes nothing. Handling it redraws.
    SlideTick,
    WipeDraft(String),
    ExportNotes,
    ConfirmWipe,
    Undo,
    Redo,
    TaskQuery(String),
    BeginTaskEdit {
        id: String,
        line: usize,
    },
    TaskLabel(String),
    CommitTaskRename,
    CancelTaskEdit,
    RemoveTask {
        id: String,
        line: usize,
    },
    CopyChat(String),
    WelcomeStored,
    WelcomeDone,
    ToggleDone,
    AskClearDone,
    CancelClearDone,
    ClearDone,
    MoveTask {
        id: String,
        line: usize,
        up: bool,
    },
    ChatQuery(String),
    FindOpen,
    FindQuery(String),
    FindNext,
    FindPrev,
    Mods {
        command: bool,
        shift: bool,
    },
    PickInvite(InvitePick),
    ToggleNoteSearch,
    ToggleTaskSearch,
    ToggleChatSearch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sheet {
    None,
    Invite,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Notes,
    Tasks,
    Chat,
}

struct TaskRow {
    note_id: String,
    note_title: String,
    line: usize,
    done: bool,
    text: String,
    /// When it is due, read once from the end of the line.
    due: Option<due::Due>,
}

struct OlderNote {
    id: String,
    title: String,
    body: String,
}

/// A due moment being put together: what the calendar has written so far. The date and the time
/// are only ever set from it, so they are always in the one form.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DueDraft {
    /// `YYYY-MM-DD`, or empty for no due date.
    date: String,
    /// `HH:MM`, or empty for no time. Only means something with a date.
    time: String,
    repeat: Option<due::Repeat>,
}

impl DueDraft {
    fn from_due(found: Option<due::Due>) -> Self {
        found.map_or_else(Self::default, |found| Self {
            date: found.date.to_string(),
            time: found.time.map_or_else(String::new, |time| time.to_string()),
            repeat: found.repeat,
        })
    }

    /// The moment this makes, if it has a day.
    fn due(&self) -> Option<due::Due> {
        Some(due::Due {
            date: dates::Date::parse(&self.date)?,
            time: due::Time::parse(&self.time),
            repeat: self.repeat,
        })
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Whose due date the calendar is choosing: the task being added, or one being edited.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CalFor {
    New,
    Edit,
}

/// A way of looking at the tasks, picked in the panel on the left of the Tasks tab.
#[derive(Clone, Debug, Eq, PartialEq)]
enum TaskView {
    /// Every task not done.
    Open,
    Today,
    Overdue,
    Upcoming,
    /// Not done, and no due date.
    NoDate,
    Done,
    /// The tasks in the notes of a group, by its key.
    Group(String),
    /// The tasks in one note.
    Note(String),
}

/// One note in a reading of a group.
struct Chapter {
    name: String,
    body: String,
}

#[derive(Clone, Debug)]
struct SavedDraft {
    id: String,
    title: String,
    body: String,
}

#[derive(Clone, Debug)]
struct MadeChapter {
    notes: Vec<NoteSummary>,
    id: String,
    saved: Option<SavedDraft>,
}

/// One step back. The next save writes whatever the page shows. It does not rewrite the log.
#[derive(Clone)]
struct EditSnap {
    title: String,
    body: String,
    cursor: text_editor::Cursor,
}

impl EditSnap {
    fn same_text(&self, title: &str, body: &str) -> bool {
        self.title == title && self.body == body
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Burst {
    Type,
    Erase,
    Break,
}

enum HistGate {
    Coalesce {
        burst: Option<Burst>,
        at: Option<Instant>,
    },
    Step {
        pushed: bool,
        dropped: Option<EditSnap>,
        redo: Vec<EditSnap>,
        burst: Option<Burst>,
        at: Option<Instant>,
    },
}

struct NoteHistory {
    undo: Vec<EditSnap>,
    redo: Vec<EditSnap>,
    burst: Option<Burst>,
    at: Option<Instant>,
}

impl Default for NoteHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl NoteHistory {
    const CAP: usize = 80;

    fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            burst: None,
            at: None,
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }

    fn coalesced(&self, burst: Burst, now: Instant) -> bool {
        matches!(burst, Burst::Type | Burst::Erase)
            && self.burst == Some(burst)
            && self
                .at
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(800))
            && !self.undo.is_empty()
    }

    fn begin(
        &mut self,
        title: &str,
        body: &str,
        cursor: text_editor::Cursor,
        burst: Burst,
        now: Instant,
    ) -> HistGate {
        if self.coalesced(burst, now) {
            let gate = HistGate::Coalesce {
                burst: self.burst,
                at: self.at,
            };
            self.burst = Some(burst);
            self.at = Some(now);
            return gate;
        }
        let redo = std::mem::take(&mut self.redo);
        let previous_burst = self.burst;
        let previous_at = self.at;
        let snap = EditSnap {
            title: title.to_owned(),
            body: body.to_owned(),
            cursor,
        };
        let mut pushed = false;
        let mut dropped = None;
        if self
            .undo
            .last()
            .is_none_or(|top| !top.same_text(&snap.title, &snap.body))
        {
            self.undo.push(snap);
            pushed = true;
            if self.undo.len() > Self::CAP {
                dropped = Some(self.undo.remove(0));
            }
        }
        self.burst = if burst == Burst::Break {
            None
        } else {
            Some(burst)
        };
        self.at = Some(now);
        HistGate::Step {
            pushed,
            dropped,
            redo,
            burst: previous_burst,
            at: previous_at,
        }
    }

    fn abort(&mut self, gate: HistGate) {
        match gate {
            HistGate::Coalesce { burst, at } => {
                self.burst = burst;
                self.at = at;
            }
            HistGate::Step {
                pushed,
                dropped,
                redo,
                burst,
                at,
            } => {
                if pushed {
                    self.undo.pop();
                }
                if let Some(dropped) = dropped {
                    self.undo.insert(0, dropped);
                }
                self.redo = redo;
                self.burst = burst;
                self.at = at;
            }
        }
    }

    fn undo(&mut self, current: EditSnap) -> Option<EditSnap> {
        while self
            .undo
            .last()
            .is_some_and(|top| top.same_text(&current.title, &current.body))
        {
            self.undo.pop();
        }
        let previous = self.undo.pop()?;
        self.redo.push(current);
        self.burst = None;
        self.at = None;
        Some(previous)
    }

    fn redo(&mut self, current: EditSnap) -> Option<EditSnap> {
        let next = self.redo.pop()?;
        if self
            .undo
            .last()
            .is_none_or(|top| !top.same_text(&current.title, &current.body))
        {
            self.undo.push(current);
            if self.undo.len() > Self::CAP {
                self.undo.remove(0);
            }
        }
        self.burst = None;
        self.at = None;
        Some(next)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WelcomeStep {
    Phrase,
    Guide,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let dir = std::env::args()
            .nth(1)
            .map_or_else(|| PathBuf::from("notebook"), PathBuf::from);
        Self::at(dir)
    }

    /// The app for one folder. A test uses this with a folder of its own.
    #[allow(clippy::too_many_lines)]
    fn at(dir: PathBuf) -> (Self, Task<Message>) {
        let mut app = Self {
            dir,
            notebook: None,
            book_gen: 0,
            search_hits: None,
            open_error: String::new(),
            catalog: Vec::new(),
            notes: Vec::new(),
            tasks: Vec::new(),
            query: String::new(),
            tag: None,
            selected: None,
            title_text: String::new(),
            body: markdown::NoteBody::new(),
            conflicts: Vec::new(),
            note_writer: String::new(),
            message: String::new(),
            screen: Screen::Notes,
            chats: Vec::new(),
            chat_id: None,
            devices: Vec::new(),
            chat: Vec::new(),
            draft_editor: text_editor::Content::new(),
            my_card: String::new(),
            their_card: String::new(),
            add_draft: String::new(),
            adding: false,
            add_call: String::new(),
            relay_ready: false,
            relay_ours: false,
            sync_open: false,
            sync_detail: "Relay is starting.".to_owned(),
            pending: false,
            net_mode: ambient_cli::NetMode::ThisComputer,
            pin_draft: String::new(),
            backup_draft: String::new(),
            advanced: false,
            net_up: false,
            unsaved: Vec::new(),
            disk_title: String::new(),
            disk_body: String::new(),
            recovery_phrase: String::new(),
            restore_phrase: String::new(),
            contact_link: String::new(),
            contact_key: String::new(),
            show_qr: false,
            qr_bytes: None,
            unlock_input: String::new(),
            sheet: Sheet::None,
            appearance: profile::Appearance::Dark,
            system_dark: true,
            loaded: false,
            saving: false,
            save_again: false,
            save_token: 0,
            your_name: String::new(),
            card_line: String::new(),
            device_draft: String::new(),
            name_draft: String::new(),
            line_draft: String::new(),
            call_draft: String::new(),
            wipe_draft: String::new(),
            aliases: BTreeMap::new(),
            people: BTreeMap::new(),
            card_lines: BTreeMap::new(),
            my_words: BTreeSet::new(),
            backup_to: String::new(),
            backup_at: 0,
            backup_busy: false,
            syncing: false,
            edited_at: None,
            pause_token: 0,
            pause_armed: false,
            search_token: 0,
            search_armed: false,
            scroll_token: 0,
            scroll_armed: false,
            writes: 0,
            size: Size::new(1280.0, 820.0),
            detail: false,
            pending_files: Vec::new(),
            task_draft: String::new(),
            confirm_delete: false,
            send_open: false,
            send_choice: None,
            boot_at: Instant::now(),
            breeze_frame: 0,
            drag_from: None,
            last_press: None,
            history: NoteHistory::new(),
            welcome: None,
            seen: true,
            task_query: String::new(),
            task_rename: None,
            task_rename_text: String::new(),
            chat_last: BTreeMap::new(),
            chat_count: BTreeMap::new(),
            seen_count: BTreeMap::new(),
            chat_query: String::new(),
            hide_done: false,
            confirm_clear: false,
            tasks_note_open: false,
            edit_due: DueDraft::default(),
            new_due: DueDraft::default(),
            cal: None,
            cal_month: (1970, 1),
            task_view: TaskView::Open,
            clock: clock::Clock::read(&zone::Zone::utc(), None, clock::now_ms()),
            zone: zone::Zone::utc(),
            zone_name: None,
            pinned_clock: None,
            hour12: true,
            week_monday: false,
            home_zone: String::new(),
            trip: None,
            trip_dismissed: None,
            find_open: false,
            find_query: String::new(),
            find_at: usize::MAX,
            command_down: false,
            shift_down: false,
            pointer_down: false,
            press_x: 0.0,
            press_y: 0.0,
            dragged: false,
            kept_click: false,
            note_search: false,
            task_search: false,
            chat_search: false,
            focus: false,
            note_panel: false,
            hover_row: None,
            send_part: None,
            orders: BTreeMap::new(),
            list_drag: None,
            folded: BTreeSet::new(),
            slide_bar: Slide::settled(true),
            slide_panels: Slide::settled(true),
            alias_editing: None,
            invite_pick: None,
            histories: BTreeMap::new(),
            history_order: Vec::new(),
            history_key: String::new(),
            read_title: String::new(),
            reading: false,
            pending_chapter: None,
            show_list: true,
            show_context: false,
            context_shut: false,
            tags: Vec::new(),
            tag_draft: String::new(),
            links_to: Vec::new(),
            graph_edges: Vec::new(),
            graph_open: false,
            graph: None,
            cached_chapters: Vec::new(),
            cached_backlinks: Vec::new(),
            known_links: None,
            lane: lane::start(),
            proof_body: String::new(),
            proof_warning: String::new(),
            misspellings: Arc::new(BTreeSet::new()),
            search_lines: BTreeMap::new(),
            search_chats: Vec::new(),
            older: None,
            version_wait: None,
            chat_cite: None,
            cite_open: false,
            task_cite: None,
            task_cite_open: false,
        };
        let saved = profile::load_profile(&app.dir);
        app.appearance = saved.appearance;
        app.seen = saved.seen;
        app.your_name.clone_from(&saved.name);
        app.name_draft.clone_from(&saved.name);
        app.card_line.clone_from(&saved.line);
        app.line_draft.clone_from(&saved.line);
        app.device_draft.clone_from(&saved.device);
        app.backup_to.clone_from(&saved.backup_to);
        app.backup_at = saved.backup_at;
        app.hour12 = saved.hour12;
        clock::set_hour12(saved.hour12);
        app.week_monday = saved.week_monday;
        app.home_zone.clone_from(&saved.home_zone);
        app.system_dark = profile::system_is_dark();
        for (id, name) in profile::load_names(&app.dir) {
            if let Some(key) = id.strip_prefix("key:") {
                app.people.insert(key.to_owned(), name);
            } else if let Some(key) = id.strip_prefix("line:") {
                app.card_lines.insert(key.to_owned(), name);
            } else {
                app.aliases.insert(id, name);
            }
        }
        app.orders = profile::load_order(&app.dir);
        app.my_words = profile::load_words(&app.dir).into_iter().collect();
        if !app.my_words.is_empty() {
            app.share_words();
        }
        for (id, count) in profile::load_seen(&app.dir) {
            app.seen_count.insert(id, count);
        }
        let task = if app.dir.join("key").exists() {
            open_task(app.dir.clone())
        } else {
            app.loaded = true;
            Task::none()
        };
        app.boot_at = Instant::now();
        app.refresh_clock();
        (app, task)
    }

    fn subscription(app: &App) -> iced::Subscription<Message> {
        let window = iced::event::listen_with(|event, status, _id| match event {
            iced::Event::Window(
                iced::window::Event::Resized(size) | iced::window::Event::Opened { size, .. },
            ) => Some(Message::Resized(size)),
            iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                Some(Message::FileDropped(path))
            }
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                Some(Message::Mods {
                    command: mods.control() || mods.command(),
                    shift: mods.shift(),
                })
            }
            iced::Event::Window(iced::window::Event::Focused) => Some(Message::ClockTick),
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => scroll_seen(),
            iced::Event::Mouse(
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)
                | iced::mouse::Event::CursorLeft,
            )
            | iced::Event::Window(iced::window::Event::Unfocused)
            | iced::Event::Touch(
                iced::touch::Event::FingerLifted { .. } | iced::touch::Event::FingerLost { .. },
            ) => Some(Message::SelectStop),
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if status == iced::event::Status::Ignored
                    && (modifiers.control() || modifiers.command())
                    && matches!(key.as_ref(), Key::Character(text) if text.eq_ignore_ascii_case("f")) =>
            {
                Some(Message::FindOpen)
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if status == iced::event::Status::Ignored
                    && (modifiers.control() || modifiers.command())
                    && matches!(key.as_ref(), Key::Character(".")) =>
            {
                Some(Message::ToggleFocus)
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if status == iced::event::Status::Ignored => {
                ignored_key(&key, text.as_deref(), modifiers).map(Message::TypedIgnored)
            }
            _ => None,
        });
        let mut parts = vec![window];
        if app.editing_note() && !app.pointer_down && app.body.selection_bounds().is_some() {
            parts.push(iced::event::listen_with(
                |event, _status, _id| match event {
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                        if matches!(
                            key.as_ref(),
                            Key::Named(iced::keyboard::key::Named::Escape)
                        ) =>
                    {
                        Some(Message::ClearSelect)
                    }
                    _ => None,
                },
            ));
        }
        if app.breeze_on() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(32)).map(|_| Message::Breeze),
            );
        }
        if app.pause_armed {
            parts.push(iced::Subscription::run_with(
                Wake::Pause(app.pause_token),
                wake_stream,
            ));
        }
        if app.search_armed {
            parts.push(iced::Subscription::run_with(
                Wake::Search(app.search_token),
                wake_stream,
            ));
        }
        if app.slide_bar.moving() || app.slide_panels.moving() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::SlideTick),
            );
        }
        parts.extend(Self::clock_ticks(app));
        if app.scroll_armed {
            parts.push(iced::Subscription::run_with(
                Wake::Scroll(app.scroll_token),
                wake_stream,
            ));
        }
        let remote = app.net_mode != ambient_cli::NetMode::ThisComputer;
        if app.notebook.is_some() && (app.relay_ready || remote) {
            parts.push(
                iced::time::every(std::time::Duration::from_secs(3)).map(|_| Message::AutoSync),
            );
        }
        parts.push(iced::Subscription::run(lane_messages));
        iced::Subscription::batch(parts)
    }

    /// While the Tasks tab is up, look at the clock every twenty seconds, so a task that has just
    /// become late, or a day that has just turned, shows without a click. Nothing runs otherwise.
    fn clock_ticks(app: &App) -> Option<iced::Subscription<Message>> {
        (app.screen == Screen::Tasks && app.pinned_clock.is_none()).then(|| {
            iced::time::every(std::time::Duration::from_secs(20)).map(|_| Message::ClockTick)
        })
    }

    #[allow(clippy::unused_self)]
    fn title(&self) -> String {
        "Aerodash".to_owned()
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                self.search_hits = None;
                self.search_lines.clear();
                self.search_chats.clear();
                if self.query.trim().is_empty() {
                    self.disarm_search();
                } else {
                    self.arm_search();
                }
                self.refresh_shown();
                Task::none()
            }
            Message::PickTag(tag) => {
                self.tag = if self.tag == tag { None } else { tag };
                self.refresh_shown();
                Task::none()
            }
            Message::Select(id) | Message::GraphPick(id) => {
                self.show_note(&id);
                Task::none()
            }
            Message::NewNote => {
                self.graph_open = false;
                self.graph = None;
                self.clear_editor();
                iced::widget::operation::focus(iced::widget::Id::new("note-title"))
            }
            Message::TitleChanged(title) => {
                if title != self.title_text {
                    self.revise(Burst::Type, |app| {
                        app.title_text = title;
                        true
                    });
                }
                Task::none()
            }
            Message::ShiftItems { back } => {
                match markdown::shift_items(&self.body.text(), &self.body.cursor(), back) {
                    Some(change) => {
                        self.drag_from = None;
                        self.apply_change(change);
                        self.note_follow(true)
                    }
                    None => Task::none(),
                }
            }
            Message::BodyEdited(action) => {
                if matches!(
                    action,
                    text_editor::Action::Click(_) | text_editor::Action::Drag(_)
                ) {
                    return Task::none();
                }
                let structured = match &action {
                    text_editor::Action::Edit(text_editor::Edit::Enter) => {
                        markdown::continue_enter(&self.body.text(), self.body.cursor())
                    }
                    text_editor::Action::Edit(text_editor::Edit::Backspace) => {
                        markdown::exit_empty_item(&self.body.text(), self.body.cursor())
                    }
                    _ => None,
                };
                if let Some(change) = structured {
                    self.drag_from = None;
                    self.apply_change(change);
                    return self.note_follow(true);
                }
                if let Some(burst) = edit_burst(&action) {
                    self.revise(burst, |app| app.body.edit(&action));
                    return self.note_follow(true);
                }
                if self.body.edit(&action) {
                    self.arm_pause();
                }
                self.note_follow(true)
            }
            Message::SelectFrom { byte, x, y } => {
                if self.command_down && self.try_open_at(byte) {
                    self.drag_from = None;
                    self.dragged = false;
                    return Task::none();
                }
                self.press_x = x;
                self.press_y = y;
                self.dragged = false;
                self.arm_press(byte);
                self.note_follow(false)
            }
            Message::ExtendTo { byte, x, y } => {
                if !self.pointer_down {
                    self.drag_from = None;
                    return Task::none();
                }
                let Some(anchor) = self.drag_from else {
                    return Task::none();
                };
                let dx = x - self.press_x;
                let dy = y - self.press_y;
                // A click moves a little. A drag has to leave the letter.
                if dx * dx + dy * dy < 1_024.0 {
                    return Task::none();
                }
                self.dragged = true;
                let byte = markdown::snap_caret(&self.body.text(), byte);
                self.body.select_between(anchor, byte);
                Task::none()
            }
            Message::SelectStop => {
                self.finish_list_drag();
                let was_down = self.pointer_down;
                let dragged = self.dragged;
                let kept = self.kept_click;
                self.pointer_down = false;
                self.dragged = false;
                self.kept_click = false;
                if self.drag_from.take().is_some() {
                    if dragged || kept {
                        self.body.end_select();
                        if dragged {
                            // The next click places the caret. It is not a second click.
                            self.last_press = None;
                        }
                    } else {
                        let byte = markdown::snap_caret(&self.body.text(), self.body.caret_byte());
                        self.body.place_byte(byte);
                    }
                }
                if was_down {
                    self.note_follow(false)
                } else {
                    Task::none()
                }
            }
            Message::SetOpenTask { line, done } => {
                if let Some(id) = self.selected.clone() {
                    let follow = self.set_task(&id, line, done);
                    let back = self.note_follow(false);
                    if self.editor_dirty() {
                        return Task::batch([follow, back, self.start_save(true)]);
                    }
                    return Task::batch([follow, back]);
                }
                let text = self.body.text();
                if let Some((next, _)) = self.checked_or_advanced(&text, line, done) {
                    self.revise(Burst::Break, |app| {
                        let cursor = app.body.cursor();
                        app.body.load(&next);
                        app.body.place(cursor);
                        true
                    });
                }
                self.note_follow(false)
            }
            Message::CopyNote => {
                if let Some(text) = self.body.selection() {
                    clipboard::write(text)
                } else {
                    Task::none()
                }
            }
            Message::CutNote => {
                if let Some(text) = self.body.selection() {
                    self.revise(Burst::Break, |app| {
                        app.body
                            .edit(&text_editor::Action::Edit(text_editor::Edit::Delete))
                    });
                    Task::batch([clipboard::write(text), self.note_follow(true)])
                } else {
                    Task::none()
                }
            }
            Message::Save => self.start_save(true),
            Message::ShowNotes => {
                if self.compact()
                    && self.screen == Screen::Notes
                    && self.detail
                    && self.sheet == Sheet::None
                {
                    self.detail = false;
                } else {
                    self.screen = Screen::Notes;
                    self.sheet = Sheet::None;
                    if self.compact() {
                        self.detail = false;
                    }
                }
                self.sync_watch();
                Task::none()
            }
            Message::ShowTasks => {
                self.refresh_clock();
                self.tasks_note_open = false;
                self.rebuild_tasks();
                self.screen = Screen::Tasks;
                self.sheet = Sheet::None;
                self.detail = false;
                self.sync_watch();
                focus_named("task-input")
            }
            Message::ShowChat => {
                if self.compact()
                    && self.screen == Screen::Chat
                    && self.detail
                    && self.sheet == Sheet::None
                {
                    self.detail = false;
                } else {
                    self.remember_unsaved();
                    self.screen = Screen::Chat;
                    self.alias_editing = None;
                    self.sheet = Sheet::None;
                    if self.compact() {
                        self.detail = false;
                    }
                    self.sync_watch();
                    self.post(lane::Job::Pull);
                }
                Task::none()
            }
            Message::ToggleList => {
                self.show_list = !self.show_list;
                Task::none()
            }
            Message::ToggleContext => {
                self.show_context = !self.show_context;
                self.context_shut = !self.show_context;
                Task::none()
            }
            Message::TagDraft(text) => {
                self.tag_draft = text;
                Task::none()
            }
            Message::PickSuggestion(tag) => {
                self.tag_draft = tag;
                self.add_tag();
                focus_named("note-tag")
            }
            Message::AddTag => {
                self.add_tag();
                Task::none()
            }
            Message::RemoveTag(tag) => {
                self.remove_tag(&tag);
                Task::none()
            }
            Message::CompleteLink(id) => self.complete_link(&id, None),
            Message::CompleteVersion { id, version } => {
                self.complete_link(&id, Some(version.as_str()))
            }
            Message::CloseVersion => {
                self.close_version();
                Task::none()
            }
            Message::OpenCited(target) => {
                let (name, version) = markdown::split_version(&target);
                self.open_note_link(&name, version.as_deref());
                Task::none()
            }
            Message::ToggleChatCite => {
                self.cite_open = !self.cite_open;
                Task::none()
            }
            Message::PickChatCite(id) => {
                self.pick_cite(&id, false);
                Task::none()
            }
            Message::ClearChatCite => {
                self.chat_cite = None;
                Task::none()
            }
            Message::ToggleTaskCite => {
                self.task_cite_open = !self.task_cite_open;
                Task::none()
            }
            Message::PickTaskCite(id) => {
                self.pick_cite(&id, true);
                Task::none()
            }
            Message::ClearTaskCite => {
                self.task_cite = None;
                Task::none()
            }
            Message::Lane(done) => {
                self.apply_lane(done);
                Task::none()
            }
            Message::AutoSync => self.begin_sync(),
            Message::ToggleSyncPop => {
                self.sync_open = !self.sync_open;
                Task::none()
            }
            Message::ToggleRelay => {
                if self.relay_ready && !self.relay_ours {
                    "This window did not start the relay.".clone_into(&mut self.sync_detail);
                    Task::none()
                } else if self.relay_ready {
                    self.stop_relay();
                    Task::none()
                } else {
                    self.start_relay();
                    self.begin_sync()
                }
            }
            Message::ToggleAdvanced => {
                self.advanced = !self.advanced;
                Task::none()
            }
            Message::PinDraft(text) => {
                self.pin_draft = text;
                Task::none()
            }
            Message::BackupDraft(text) => {
                self.backup_draft = text;
                Task::none()
            }
            Message::UseThisComputer => {
                self.use_this_computer();
                self.begin_sync()
            }
            Message::SavePinned => {
                self.save_pinned();
                self.begin_sync()
            }
            Message::OpenPaired => {
                self.open_paired();
                Task::none()
            }
            Message::OpenPerson => {
                self.open_person();
                Task::none()
            }
            Message::OpenOneMore => {
                self.open_one_more();
                Task::none()
            }
            Message::AddDraft(text) => {
                self.add_draft = text;
                Task::none()
            }
            Message::AddCall(text) => {
                self.add_call = text;
                Task::none()
            }
            Message::ToggleAdd => {
                self.adding = !self.adding;
                Task::none()
            }
            Message::AddToChat => {
                self.add_to_chat();
                self.begin_sync()
            }
            Message::CopyCard => self.copy_card(),
            Message::MyCardChanged(card) => {
                self.my_card = card;
                Task::none()
            }
            Message::TheirCardChanged(card) => {
                self.their_card = card;
                Task::none()
            }
            Message::StartConversation => {
                self.start_chat();
                Task::none()
            }
            Message::DraftEdited(action) => {
                self.draft_editor.perform(action);
                Task::none()
            }
            Message::SendChat => {
                self.send_chat();
                Task::none()
            }
            Message::ShowPhrase => {
                self.show_phrase();
                Task::none()
            }
            Message::CopyPhrase => self.copy_phrase(),
            Message::RestorePhraseChanged(phrase) => {
                self.restore_phrase = phrase;
                Task::none()
            }
            Message::StartRestore => {
                self.start_restore();
                Task::none()
            }
            Message::CopyLink => self.copy_link(),
            Message::ToggleQr => {
                self.toggle_qr();
                Task::none()
            }
            Message::SendNoteToChat => {
                self.send_note_to_chat();
                Task::none()
            }
            Message::SelectChat(id) => {
                if self.chat_id.as_deref() != Some(id.as_str()) {
                    self.chat.clear();
                    self.adding = false;
                    self.add_draft.clear();
                }
                self.chat_id = Some(id);
                self.screen = Screen::Chat;
                self.sheet = Sheet::None;
                self.detail = true;
                self.sync_watch();
                self.post(lane::Job::Pull);
                Task::none()
            }
            Message::SetTask { id, line, done } => {
                let follow = self.set_task(&id, line, done);
                if self.editor_dirty() && self.selected.as_deref() == Some(id.as_str()) {
                    Task::batch([follow, self.start_save(true)])
                } else {
                    follow
                }
            }
            Message::Revoke(id) => {
                self.revoke(&id);
                Task::none()
            }
            Message::UnlockChanged(text) => {
                self.unlock_input = text;
                Task::none()
            }
            Message::SubmitUnlock => self.submit_unlock(),
            Message::ShowInvite => {
                self.refresh_link();
                self.invite_pick = None;
                self.sheet = Sheet::Invite;
                Task::none()
            }
            Message::ShowSettings => {
                self.refresh_link();
                self.devices = self
                    .notebook
                    .as_ref()
                    .and_then(|book| book.devices_held().ok())
                    .unwrap_or_default();
                self.sheet = Sheet::Settings;
                Task::none()
            }
            Message::CloseSheet => {
                self.sheet = Sheet::None;
                Task::none()
            }
            Message::ToggleTheme => {
                self.appearance = if self.is_dark() {
                    profile::Appearance::Light
                } else {
                    profile::Appearance::Dark
                };
                self.persist_profile();
                Task::none()
            }
            Message::Resized(size) => {
                let was_compact = self.compact();
                let was_beside = self.side_by_side_context();
                self.size = size;
                // A panel that was beside the page does not jump over it when the window narrows.
                if was_beside && !self.side_by_side_context() {
                    self.show_context = false;
                }
                if !was_compact && self.compact() {
                    self.detail = match self.screen {
                        Screen::Notes => self.selected.is_some() || self.editor_dirty(),
                        Screen::Chat => self.chat_id.is_some(),
                        Screen::Tasks => false,
                    };
                }
                Task::none()
            }
            Message::FileDropped(path) => {
                self.stage_file(path);
                Task::none()
            }
            Message::PickFile => self.pick_file(),
            Message::FilePicked(result) => {
                match result {
                    Ok(path) => self.stage_file(path),
                    Err(err) => {
                        if err != "No file chosen." {
                            self.message = err;
                        }
                    }
                }
                Task::none()
            }
            Message::DropPending(index) => {
                if index < self.pending_files.len() {
                    self.pending_files.remove(index);
                }
                Task::none()
            }
            Message::OpenPath(path) => {
                if let Err(err) = files::open_with_system(&path) {
                    self.message = err;
                }
                Task::none()
            }
            Message::Format(kind) => {
                self.insert_mark(kind);
                self.note_follow(true)
            }
            Message::PasteNote => clipboard::read().map(Message::Pasted),
            Message::Pasted(Some(text)) if !text.is_empty() => {
                self.revise(Burst::Break, |app| {
                    let before = app.body.text();
                    app.body
                        .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                            Arc::new(text),
                        )));
                    app.body.text() != before
                });
                self.note_follow(true)
            }
            // A paste that was handled elsewhere, and a frame of a slide: handling them redraws.
            Message::Pasted(_) | Message::SlideTick | Message::FieldDone => Task::none(),
            Message::ClearSelect => {
                let cursor = self.body.cursor();
                self.body.place(text_editor::Cursor {
                    position: cursor.position,
                    selection: None,
                });
                Task::none()
            }
            Message::ToggleRead => {
                self.toggle_read();
                Task::none()
            }
            Message::ChapterMade(result) => self.finish_chapter(result),
            Message::TaskDraft(text) => {
                self.task_draft = text;
                Task::none()
            }
            Message::AddTask => {
                let follow = self.add_loose_task();
                // The cursor stays in the field, so the next task can be typed at once.
                let back = focus_named("task-input");
                if self.editor_dirty() {
                    Task::batch([follow, self.start_save(true), back])
                } else {
                    Task::batch([follow, back])
                }
            }
            Message::AskDelete => {
                if self.selected.is_some() || self.editor_dirty() {
                    self.confirm_delete = true;
                }
                Task::none()
            }
            Message::CancelDelete => {
                self.confirm_delete = false;
                Task::none()
            }
            Message::DeleteNote => self.delete_open_note(),
            Message::Deleted(result) => self.finish_delete(result),
            Message::ChooseSend(id) => {
                self.send_choice = Some(id);
                Task::none()
            }
            Message::ConfirmSend => {
                self.confirm_send();
                Task::none()
            }
            Message::CancelSend => {
                self.send_open = false;
                self.send_part = None;
                Task::none()
            }
            Message::Breeze => {
                self.breeze_frame = self.breeze_frame.wrapping_add(1);
                Task::none()
            }
            Message::Woke(wake) => match wake {
                Wake::Pause(token) if self.pause_armed && token == self.pause_token => {
                    self.pause_armed = false;
                    self.maybe_proof();
                    self.start_save(true)
                }
                Wake::Search(token) if self.search_armed && token == self.search_token => {
                    self.search_armed = false;
                    if !self.query.trim().is_empty() {
                        self.post(lane::Job::Search(self.query.clone()));
                    }
                    Task::none()
                }
                Wake::Scroll(token) if self.scroll_armed && token == self.scroll_token => {
                    // Handling the wake redraws, and the style sees whether the last scroll is
                    // old, so the thumb goes. If the pane is still moving, look again later.
                    if scrolled_recently() {
                        self.scroll_token = self.scroll_token.wrapping_add(1);
                    } else {
                        self.scroll_armed = false;
                        SCROLL_WAKE.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    Task::none()
                }
                Wake::Pause(_) | Wake::Search(_) | Wake::Scroll(_) => Task::none(),
            },
            Message::Scrolled => {
                self.scroll_token = self.scroll_token.wrapping_add(1);
                self.scroll_armed = true;
                Task::none()
            }
            Message::Opened(result) => self.finish_open(result),
            Message::Saved(token, result, title, body) => {
                self.finish_save(token, result, title, body)
            }
            Message::Applied(result) => self.finish_applied(result),
            Message::DeviceDraft(text) => {
                self.device_draft = text;
                Task::none()
            }
            Message::NameDraft(text) => {
                self.name_draft = clip_chars(text, 48);
                self.your_name.clone_from(&self.name_draft);
                self.persist_profile();
                self.refresh_link();
                Task::none()
            }
            Message::LineDraft(text) => {
                self.line_draft = clip_chars(text, 120);
                self.card_line.clone_from(&self.line_draft);
                self.persist_profile();
                self.refresh_link();
                Task::none()
            }
            Message::FinishOnboarding => self.finish_onboarding(),
            Message::PickAppearance(appearance) => {
                self.appearance = appearance;
                self.persist_profile();
                Task::none()
            }
            Message::CallDraft(text) => {
                self.call_draft = text;
                Task::none()
            }
            Message::AliasDraft(id, text) => {
                self.remember_alias(&id, text);
                Task::none()
            }
            Message::PersonDraft(key, text) => {
                self.remember_person(&key, text);
                Task::none()
            }
            Message::EditAlias => match self.chat_id.clone() {
                Some(id) if self.alias_editing.as_deref() == Some(id.as_str()) => {
                    self.alias_editing = None;
                    Task::none()
                }
                Some(id) => {
                    self.alias_editing = Some(id);
                    focus_named("chat-alias")
                }
                None => Task::none(),
            },
            Message::ToggleFocus => {
                // Focus is for writing a note in the wide layout. Elsewhere there is nothing to turn on.
                if !self.focus && (!self.editing_note() || self.compact()) {
                    return Task::none();
                }
                self.focus = !self.focus;
                // Going in, the bar slides away. Coming out it is back at once.
                self.slide_bar.set(!self.focus);
                self.slide_panels.set(!self.focus);
                if !self.focus {
                    self.slide_bar.jump(true);
                }
                Task::none()
            }
            Message::TitleSubmit => focus_named("note-body"),
            Message::AddWord => {
                self.add_word();
                Task::none()
            }
            Message::TypedIgnored(typed) => self.type_ignored(&typed),
            Message::BackupTo(text) => {
                self.backup_to = text;
                Task::none()
            }
            Message::BackUp => self.start_backup(),
            Message::BackedUp(result) => {
                self.finish_backup(result);
                Task::none()
            }
            Message::SendSelection => {
                self.send_selection();
                Task::none()
            }
            Message::ToggleGraph => {
                self.graph_open = !self.graph_open;
                // On a narrow window the graph is the thing opened from the list, and closing it
                // goes back to the list.
                if self.compact() {
                    self.detail = self.graph_open;
                }
                self.refresh_graph();
                Task::none()
            }
            Message::DragStart { group, id } => {
                self.list_drag = Some(ListDrag {
                    group,
                    id,
                    over: None,
                });
                Task::none()
            }
            Message::Fold(id) => {
                if !self.folded.remove(&id) {
                    self.folded.insert(id);
                }
                Task::none()
            }
            Message::ShowLinks => {
                self.note_panel = false;
                self.show_context = true;
                self.context_shut = false;
                Task::none()
            }
            Message::OpenCalendar(whose) => {
                if self.cal == Some(whose) {
                    self.cal = None;
                } else {
                    // Read the clock first, so the calendar opens on the day it really is.
                    self.refresh_clock();
                    let shown =
                        dates::Date::parse(&self.draft(whose).date).unwrap_or(self.clock.today);
                    self.cal_month = (shown.year, shown.month);
                    self.cal = Some(whose);
                }
                Task::none()
            }
            Message::CloseCalendar => {
                self.cal = None;
                Task::none()
            }
            Message::CalMonth(delta) => {
                let (year, month) = self.cal_month;
                self.cal_month = dates::shift_month(year, month, delta);
                Task::none()
            }
            Message::PickDay(text) => {
                // Only a day that exists is taken, so what is written is always a real date.
                if dates::Date::parse(&text).is_some()
                    && let Some(whose) = self.cal
                {
                    self.draft_mut(whose).date = text;
                }
                self.cal = None;
                Task::none()
            }
            Message::PickQuick(days) => {
                // Worked out here, after reading the clock, and not when the button was drawn.
                self.refresh_clock();
                if let Some(whose) = self.cal
                    && let Some(day) = self.clock.today.plus_days(days)
                {
                    self.draft_mut(whose).date = day.to_string();
                }
                self.cal = None;
                Task::none()
            }
            Message::ClearDue => {
                if let Some(whose) = self.cal {
                    self.draft_mut(whose).clear();
                }
                self.cal = None;
                Task::none()
            }
            Message::StepTime { hours, minutes } => {
                if let Some(whose) = self.cal {
                    self.refresh_clock();
                    let today = self.clock.today;
                    let draft = self.draft_mut(whose);
                    if draft.date.is_empty() {
                        draft.date = today.to_string();
                    }
                    // From the time there is, or from nine in the morning when there is none.
                    let from = due::Time::parse(&draft.time).map_or(9 * 60, due::Time::minutes);
                    let to = (i64::from(from) + i64::from(hours) * 60 + i64::from(minutes))
                        .rem_euclid(24 * 60);
                    draft.time = due::Time::new(
                        u32::try_from(to / 60).unwrap_or(9),
                        u32::try_from(to % 60).unwrap_or(0),
                    )
                    .map_or_else(String::new, |time| time.to_string());
                }
                Task::none()
            }
            Message::SetTime(text) => {
                if let Some(whose) = self.cal {
                    self.refresh_clock();
                    let today = self.clock.today;
                    let draft = self.draft_mut(whose);
                    if text.is_empty() || due::Time::parse(&text).is_some() {
                        if !text.is_empty() && draft.date.is_empty() {
                            draft.date = today.to_string();
                        }
                        draft.time = text;
                    }
                }
                Task::none()
            }
            Message::SetRepeat(repeat) => {
                if let Some(whose) = self.cal {
                    self.refresh_clock();
                    let today = self.clock.today;
                    let draft = self.draft_mut(whose);
                    // A repeat needs a day to start from. Without one it starts today.
                    if repeat.is_some() && draft.date.is_empty() {
                        draft.date = today.to_string();
                    }
                    draft.repeat = repeat;
                }
                Task::none()
            }
            Message::RepeatEvery(delta) => {
                if let Some(whose) = self.cal {
                    let draft = self.draft_mut(whose);
                    if let Some(repeat) = &mut draft.repeat
                        && repeat.unit != due::Unit::Weekday
                    {
                        let every = i64::from(repeat.every) + i64::from(delta);
                        repeat.every = u32::try_from(every.clamp(1, 99)).unwrap_or(1);
                    }
                }
                Task::none()
            }
            Message::ClockTick => {
                self.refresh_clock();
                Task::none()
            }
            Message::PickHour12(on) => {
                self.hour12 = on;
                clock::set_hour12(on);
                self.persist_profile();
                Task::none()
            }
            Message::PickWeekMonday(on) => {
                self.week_monday = on;
                self.persist_profile();
                Task::none()
            }
            Message::SetHomeHere => {
                if let Some(name) = self.zone_name.clone() {
                    self.home_zone = name;
                    self.trip = None;
                    self.persist_profile();
                    self.say("This is home now.");
                }
                Task::none()
            }
            Message::DismissTrip => {
                self.trip_dismissed = Some(self.zone_key());
                Task::none()
            }
            Message::BackToList => {
                self.detail = false;
                self.graph_open = false;
                self.graph = None;
                self.cal = None;
                Task::none()
            }
            Message::ShowTaskView(view) => {
                // On a narrow window the views are the list, and choosing one opens its tasks.
                if self.compact() {
                    self.detail = true;
                }
                self.task_view = view;
                Task::none()
            }
            Message::ToggleTasksNote => {
                self.tasks_note_open = !self.tasks_note_open;
                Task::none()
            }
            Message::ToggleNoteSettings => {
                self.note_panel = !self.note_panel;
                self.show_context = true;
                self.context_shut = false;
                Task::none()
            }
            Message::RowHover(id, over) => {
                if let Some(drag) = &mut self.list_drag {
                    if over {
                        drag.over = Some(id.clone());
                    } else if drag.over.as_deref() == Some(id.as_str()) {
                        drag.over = None;
                    }
                }
                if over {
                    self.hover_row = Some(id);
                } else if self.hover_row.as_deref() == Some(id.as_str()) {
                    self.hover_row = None;
                }
                Task::none()
            }
            Message::AskDeleteNote(id) => {
                if self.selected.as_deref() != Some(id.as_str()) {
                    self.show_note(&id);
                }
                if self.selected.as_deref() == Some(id.as_str()) {
                    self.confirm_delete = true;
                }
                Task::none()
            }
            Message::FocusBar => {
                self.slide_bar.set(!self.slide_bar.open);
                Task::none()
            }
            Message::FinishAlias => {
                self.alias_editing = None;
                Task::none()
            }
            Message::WipeDraft(text) => {
                self.wipe_draft = text;
                Task::none()
            }
            Message::ExportNotes => self.export_notes(),
            Message::ConfirmWipe => self.wipe_notebook(),
            Message::Undo => {
                self.undo_note();
                self.note_follow(true)
            }
            Message::Redo => {
                self.redo_note();
                self.note_follow(true)
            }
            Message::TaskQuery(query) => {
                self.task_query = query;
                Task::none()
            }
            Message::BeginTaskEdit { id, line } => {
                let label = self
                    .tasks
                    .iter()
                    .find(|task| task.note_id == id && task.line == line)
                    .map(|task| task.text.clone())
                    .unwrap_or_default();
                self.edit_due = DueDraft::from_due(due::Due::of(&markdown::without_cites(&label)));
                self.task_rename_text = markdown::task_words(&label);
                self.task_rename = Some((id, line));
                Task::none()
            }
            Message::TaskLabel(text) => {
                self.task_rename_text = text;
                Task::none()
            }
            Message::CommitTaskRename => self.commit_task_rename(),
            Message::CancelTaskEdit => {
                self.task_rename = None;
                self.task_rename_text.clear();
                self.edit_due.clear();
                Task::none()
            }
            Message::RemoveTask { id, line } => self.remove_listed_task(&id, line),
            Message::MoveTask { id, line, up } => self.move_listed_task(&id, line, up),
            Message::ToggleDone => {
                self.hide_done = !self.hide_done;
                Task::none()
            }
            Message::AskClearDone => {
                if self.tasks.iter().any(|task| task.done) {
                    self.confirm_clear = true;
                }
                Task::none()
            }
            Message::CancelClearDone => {
                self.confirm_clear = false;
                Task::none()
            }
            Message::ClearDone => self.clear_done_tasks(),
            Message::CopyChat(text) => {
                self.say("Message copied.");
                clipboard::write(text)
            }
            Message::ChatQuery(query) => {
                self.chat_query = query;
                Task::none()
            }
            Message::FindOpen => {
                self.find_open = !self.find_open;
                if self.find_open {
                    self.find_at = usize::MAX;
                    self.screen = Screen::Notes;
                    self.sheet = Sheet::None;
                    self.detail = true;
                }
                Task::none()
            }
            Message::FindQuery(query) => {
                self.find_query = query;
                self.find_at = usize::MAX;
                Task::none()
            }
            Message::FindNext => {
                self.seek_find(true);
                self.note_reveal()
            }
            Message::FindPrev => {
                self.seek_find(false);
                self.note_reveal()
            }
            Message::Mods { command, shift } => {
                self.command_down = command;
                self.shift_down = shift;
                Task::none()
            }
            Message::PickInvite(pick) => {
                self.invite_pick = Some(pick);
                Task::none()
            }
            Message::ToggleNoteSearch => self.toggle_note_search(),
            Message::ToggleTaskSearch => self.toggle_task_search(),
            Message::ToggleChatSearch => self.toggle_chat_search(),
            Message::WelcomeStored => {
                if !self.recovery_phrase.is_empty() {
                    self.welcome = Some(WelcomeStep::Guide);
                }
                Task::none()
            }
            Message::WelcomeDone => {
                self.welcome = None;
                self.seen = true;
                self.persist_profile();
                self.say("This notebook is on this computer.");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // The screen is always the first layer, so what is over it never moves it in the tree.
        let mut layers = stack![self.screen_view()].width(Fill).height(Fill);
        if let Some(sheet) = self.context_sheet() {
            layers = layers.push(dim_layer(
                sheet,
                Message::ToggleContext,
                iced::alignment::Horizontal::Right,
                0.0,
            ));
        }
        if let Some((card, close)) = self.modal() {
            layers = layers.push(dim_layer(
                card,
                close,
                iced::alignment::Horizontal::Center,
                16.0,
            ));
        }
        Wheel {
            child: layers.into(),
        }
        .into()
    }

    /// Whether a screen is up that anything can float over.
    fn has_screen(&self) -> bool {
        self.notebook.is_some() && self.welcome.is_none() && !self.breeze_on()
    }

    /// The panel on the right as a sheet from the right edge, when there is no room for it beside
    /// the page, or on a narrow window.
    fn context_sheet(&self) -> Option<Element<'_, Message>> {
        let showing = self.has_screen()
            && self.show_context
            && self.context_room()
            && !self.side_by_side_context()
            && !self.focused()
            && (!self.compact() || self.detail);
        showing.then(|| self.context_pane())
    }

    /// A question or a choice that takes the middle of the window and the rest of it waits: the
    /// same on a large window and a small one. The card and what closes it.
    fn modal(&self) -> Option<(Element<'_, Message>, Message)> {
        if !self.has_screen() {
            return None;
        }
        if self.confirm_delete {
            return Some((self.delete_confirm(), Message::CancelDelete));
        }
        if self.send_open {
            return Some((self.send_card(), Message::CancelSend));
        }
        if self.confirm_clear && self.screen == Screen::Tasks {
            return Some((
                container(self.clear_confirm())
                    .width(Fill)
                    .max_width(380)
                    .padding(18)
                    .style(card_style)
                    .into(),
                Message::CancelClearDone,
            ));
        }
        if let (Some(whose), true) = (self.cal, self.screen == Screen::Tasks && self.compact()) {
            return Some((self.calendar_card(whose), Message::CloseCalendar));
        }
        None
    }

    /// Which chat to send the open note to.
    fn send_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut choices = column![].spacing(4).width(Fill);
        for chat in &self.chats {
            let chosen = self.send_choice.as_deref() == Some(chat.id.as_str());
            choices = choices.push(
                button(text(self.chat_name(chat)).size(15).width(Fill))
                    .width(Fill)
                    .padding([8.0, 10.0])
                    .on_press(Message::ChooseSend(chat.id.clone()))
                    .style(list_row(chosen)),
            );
        }
        let list_h = self
            .chats
            .iter()
            .fold(0.0_f32, |height, _| (height + 40.0).min(240.0))
            .max(40.0);
        container(
            column![
                text("Send to which chat?").font(serif()).size(18),
                text("The words are copied into the chat. The note stays as it is.")
                    .size(12)
                    .color(ink.muted),
                vscroll(choices).height(list_h).width(Fill),
                row![
                    ghost_label("Cancel", Message::CancelSend),
                    space().width(Fill),
                    action_label("Send", Message::ConfirmSend),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(10),
        )
        .width(Fill)
        .max_width(380)
        .padding(18)
        .style(card_style)
        .into()
    }

    fn screen_view(&self) -> Element<'_, Message> {
        if self.breeze_on() {
            return self.breeze_screen();
        }
        if self.open_error == "this folder is locked" {
            return self.unlock_screen();
        }
        if self.notebook.is_none() && self.open_error.is_empty() {
            return self.onboarding_screen();
        }
        if self.welcome.is_some() {
            return self.welcome_screen();
        }
        if self.compact() {
            return column![
                self.compact_title(),
                hairline(),
                self.compact_body(),
                hairline(),
                self.bottom_bar(),
            ]
            .height(Fill)
            .into();
        }
        // The panels are always in the row, at a width that eases between its full size and none,
        // so the page does not change place in the tree and lose the caret when they go.
        let t = self.panel_shown();
        let mut left = row![];
        let mut left_full = 28.0;
        if self.show_list {
            left = left.push(self.sidebar()).push(hairline_v());
            left_full += self.side_width() + 1.0;
        }
        left = left.push(self.edge_toggle(true));
        let mut right = row![];
        let mut right_full = 0.0;
        if self.context_room() {
            right = right.push(self.edge_toggle(false));
            right_full += 28.0;
            if self.show_context && self.side_by_side_context() {
                right = right.push(hairline_v()).push(self.context_pane());
                right_full += CONTEXT_WIDTH + 1.0;
            }
        }
        let body = row![
            container(left).width(left_full * t).height(Fill).clip(true),
            self.main(),
            container(right)
                .width(right_full * t)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .clip(true),
        ]
        .width(Fill)
        .height(Fill);
        column![self.toolbar(), hairline(), body]
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut bar = row![
            text("Aerodash")
                .font(serif())
                .size(if self.size.width < 980.0 { 20 } else { 24 })
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding([10.0, 16.0]);
        if self.size.width >= 900.0 {
            bar = bar.push(
                text(self.message.as_str())
                    .size(13)
                    .color(ink.muted)
                    .width(Fill),
            );
        } else {
            bar = bar.push(space().width(Fill));
        }
        bar = bar.push(icon_button(
            gear_handle(self.is_dark()),
            Message::ShowSettings,
        ));
        bar = bar.push(icon_button(
            mode_handle(self.is_dark()),
            Message::ToggleTheme,
        ));
        if self.notebook.is_some() && !self.editor_dirty() {
            bar = bar.push(text("✓").size(18).color(Color::from_rgb8(46, 140, 96)));
        }
        bar = bar.push(anchored(
            icon_button(globe_handle(self.globe_color()), Message::ToggleSyncPop),
            self.sync_open.then(|| self.sync_card()),
            Message::ToggleSyncPop,
        ));
        container(bar).width(Fill).into()
    }

    fn globe_color(&self) -> [u8; 3] {
        let on = if self.net_mode == ambient_cli::NetMode::ThisComputer {
            self.relay_ready
        } else {
            self.net_up
        };
        if !on {
            [140, 133, 122]
        } else if self.pending {
            [184, 78, 62]
        } else {
            [46, 140, 96]
        }
    }

    fn sync_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let on = if self.net_mode == ambient_cli::NetMode::ThisComputer {
            self.relay_ready
        } else {
            self.net_up
        };
        let relay = match self.net_mode {
            ambient_cli::NetMode::ThisComputer if self.relay_ready && self.relay_ours => {
                "Relay is on. Cleartext on this computer only. Not a public server."
            }
            ambient_cli::NetMode::ThisComputer if self.relay_ready => {
                "Relay is on, started by another window. Cleartext on this computer only."
            }
            ambient_cli::NetMode::ThisComputer => "Relay is off.",
            ambient_cli::NetMode::List => "Using the introduction list.",
            ambient_cli::NetMode::Pin => "Using a pinned relay.",
        };
        let switch = if self.relay_ready && self.relay_ours {
            "Turn relay off"
        } else if self.relay_ready {
            "Relay is not this window's"
        } else {
            "Turn relay on"
        };
        let state = if self.editor_dirty() {
            "Still writing."
        } else if on && self.pending {
            "Changes have not synced."
        } else if on {
            "Synced."
        } else {
            "Saved on this computer."
        };
        let cadence = if self.net_mode == ambient_cli::NetMode::ThisComputer && !self.relay_ready {
            "Turn the relay on to sync."
        } else if on {
            "Syncs on its own every few seconds."
        } else if self.sync_detail == "No relay answered." {
            "No relay answered."
        } else {
            "Syncs on its own every few seconds."
        };
        let line = iced::Length::Fixed(if self.advanced { 340.0 } else { 276.0 });
        let switch_button = if self.relay_ready && !self.relay_ours {
            ghost_label(switch, Message::ToggleRelay)
        } else {
            action_label(switch, Message::ToggleRelay)
        };
        let mut column = column![
            text(state).font(serif()).size(20).width(line),
            text(cadence).size(13).color(ink.muted).width(line),
            text(relay).size(13).color(ink.muted).width(line),
            text(self.sync_detail.as_str())
                .size(13)
                .color(ink.text)
                .width(line),
            switch_button,
            ghost_label(
                if self.advanced {
                    "Hide advanced"
                } else {
                    "Advanced"
                },
                Message::ToggleAdvanced,
            ),
        ]
        .spacing(8);
        if self.advanced {
            column = column
                .push(text_input("Pinned relay", &self.pin_draft).on_input(Message::PinDraft));
            column = column.push(
                text_input("Backup relay", &self.backup_draft).on_input(Message::BackupDraft),
            );
            column = column.push(action_label("Save pinned relay", Message::SavePinned));
            column = column.push(ghost_label("This computer", Message::UseThisComputer));
            if ambient_cli::INTRODUCTION.is_empty() {
                column = column.push(
                    text("No public relays are listed yet.")
                        .size(13)
                        .color(ink.muted)
                        .width(line),
                );
            }
        }
        container(column)
            .padding(14)
            .width(if self.advanced { 380 } else { 300 })
            .style(card_style)
            .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let tasks_name = self.tasks_tab();
        let chat_name = self.chat_tab();
        let modes = container(
            row![
                segment("Notes", self.screen == Screen::Notes, Message::ShowNotes),
                segment(
                    &tasks_name,
                    self.screen == Screen::Tasks,
                    Message::ShowTasks,
                ),
                segment(&chat_name, self.screen == Screen::Chat, Message::ShowChat),
            ]
            .spacing(4),
        )
        .padding(4)
        .width(Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(ink.bg)),
            border: iced::Border {
                radius: 10.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        });
        let mut side = column![modes]
            .spacing(14)
            .padding(16)
            .width(Fill)
            .height(Fill);
        if self.screen == Screen::Chat {
            side = side.push(action_fill("Invite", Message::ShowInvite));
            if self.chats.is_empty() {
                side = side.push(text("No conversations yet.").size(13).color(ink.muted));
            }
            for chat in &self.chats {
                let selected = self.chat_id.as_deref() == Some(chat.id.as_str());
                let (preview, time) = self.chat_last.get(&chat.id).cloned().unwrap_or_default();
                side = side.push(chat_row(
                    &self.chat_name(chat),
                    &self.who_line(chat),
                    &preview,
                    &time,
                    &chat.id,
                    selected,
                    self.chat_is_new(&chat.id),
                ));
            }
        } else if self.screen == Screen::Tasks {
            side = side.push(vscroll(self.task_sidebar()).height(Fill).width(Fill));
        } else {
            side = side.push(self.note_index());
        }
        container(side)
            .width(self.side_width())
            .height(Fill)
            .style(surface_pane)
            .into()
    }

    /// The panel on the left of the Tasks tab: the ways to look at the tasks. Dates, the groups
    /// the notes with tasks are in, and the notes.
    fn task_sidebar(&self) -> iced::widget::Column<'_, Message> {
        let ink = self.ink();
        let count = |view: &TaskView| {
            self.tasks
                .iter()
                .filter(|task| {
                    self.in_view(task, view)
                        && if *view == TaskView::Open
                            || matches!(view, TaskView::Group(_) | TaskView::Note(_))
                        {
                            !task.done
                        } else {
                            true
                        }
                })
                .count()
        };
        let label = |text_in: &str| {
            text(text_in.to_uppercase())
                .size(11)
                .color(ink.muted)
                .width(Fill)
        };
        let mut panel = column![self.clock_block(), space().height(8), label("Views")]
            .spacing(2)
            .width(Fill);
        for (name, view) in [
            ("All open", TaskView::Open),
            ("Due today", TaskView::Today),
            ("Overdue", TaskView::Overdue),
            ("Upcoming", TaskView::Upcoming),
            ("No date", TaskView::NoDate),
            ("Done", TaskView::Done),
        ] {
            let n = count(&view);
            panel = panel.push(self.task_view_row(name, n, 0, view));
        }
        // Groups: every group a note with a task is in, and every group that contains it.
        let mut groups: BTreeMap<String, (String, usize, std::collections::BTreeSet<usize>)> =
            BTreeMap::new();
        for (index, task) in self.tasks.iter().enumerate() {
            let Some(note) = self.catalog.iter().find(|note| note.id == task.note_id) else {
                continue;
            };
            for tag in &note.tags {
                let parts = tag_path(tag);
                for at in 0..parts.len() {
                    let key = group_key(parts.get(..=at).unwrap_or(&[]));
                    let name = parts
                        .get(at)
                        .map_or_else(String::new, |part| (*part).to_owned());
                    let entry = groups
                        .entry(key)
                        .or_insert_with(|| (name, 0, std::collections::BTreeSet::new()));
                    if !task.done && entry.2.insert(index) {
                        entry.1 += 1;
                    }
                }
            }
        }
        if !groups.is_empty() {
            panel = panel.push(space().height(10)).push(label("Groups"));
            for (key, (name, open, _)) in &groups {
                let depth = key.matches('/').count();
                panel = panel.push(self.task_view_row(
                    name,
                    *open,
                    depth,
                    TaskView::Group(key.clone()),
                ));
            }
        }
        // Notes: every note that has a task, with how many are still open.
        let mut notes: Vec<(String, String, usize)> = Vec::new();
        for task in &self.tasks {
            if let Some(found) = notes.iter_mut().find(|(id, _, _)| *id == task.note_id) {
                found.2 += usize::from(!task.done);
            } else {
                let title = if task.note_title.is_empty() {
                    "Untitled".to_owned()
                } else {
                    task.note_title.clone()
                };
                notes.push((task.note_id.clone(), title, usize::from(!task.done)));
            }
        }
        notes.sort_by(|a, b| natural_cmp(&a.1, &b.1));
        if !notes.is_empty() {
            panel = panel.push(space().height(10)).push(label("Notes"));
            for (id, title, open) in notes {
                panel = panel.push(self.task_view_row(&title, open, 0, TaskView::Note(id)));
            }
        }
        panel
    }

    /// What day and time it is, and in which zone, so that no date on the page is read against a
    /// guess. It is the reading every date and every late task on this screen is worked out from.
    fn clock_block(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let today = self.clock.today;
        let month = dates::MONTHS
            .get(usize::try_from(today.month.saturating_sub(1)).unwrap_or(0))
            .copied()
            .unwrap_or("");
        let zone = if self.clock.zone.is_empty() {
            self.clock.offset_label()
        } else {
            format!("{} ({})", self.clock.zone, self.clock.offset_label())
        };
        container(
            column![
                text(format!("{}, {month} {}", today.weekday_full(), today.day))
                    .font(serif())
                    .size(17),
                text(format!("{} · {zone}", self.clock.time_label()))
                    .size(12)
                    .color(ink.muted),
            ]
            .spacing(2),
        )
        .padding([6, 8])
        .into()
    }

    fn task_view_row(
        &self,
        name: &str,
        count: usize,
        depth: usize,
        view: TaskView,
    ) -> Element<'_, Message> {
        let ink = self.ink();
        let active = self.task_view == view;
        let mut line = row![].align_y(iced::Alignment::Center);
        for _ in 0..depth {
            line = line.push(space().width(12)).push(hairline_v());
        }
        line.push(
            button(
                row![
                    text(name.to_owned()).size(13).width(Fill),
                    text(count.to_string()).size(12).color(ink.muted),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .width(Fill)
            .padding([6.0, 8.0])
            .on_press(Message::ShowTaskView(view))
            .style(list_row(active)),
        )
        .into()
    }

    fn toggle_note_search(&mut self) -> Task<Message> {
        if self.note_search || !self.query.trim().is_empty() {
            self.note_search = false;
            self.query.clear();
            self.refresh_shown();
            Task::none()
        } else {
            self.note_search = true;
            focus_named("note-search")
        }
    }

    fn toggle_task_search(&mut self) -> Task<Message> {
        if self.task_search || !self.task_query.trim().is_empty() {
            self.task_search = false;
            self.task_query.clear();
            Task::none()
        } else {
            self.task_search = true;
            focus_named("task-search")
        }
    }

    fn toggle_chat_search(&mut self) -> Task<Message> {
        if self.chat_search || !self.chat_query.trim().is_empty() {
            self.chat_search = false;
            self.chat_query.clear();
            Task::none()
        } else {
            self.chat_search = true;
            focus_named("chat-search")
        }
    }

    fn note_index(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let searching = self.note_search || !self.query.trim().is_empty();
        let mut block = column![row![
            space().width(Fill),
            mini_button(
                mini_handle(Mini::Graph, self.is_dark()),
                if self.graph_open {
                    "Back to the note"
                } else {
                    "Show notes and links as a graph"
                },
                Message::ToggleGraph,
            ),
            mini_button(
                mini_handle(Mini::Search, self.is_dark()),
                if searching { "Close search" } else { "Search" },
                Message::ToggleNoteSearch,
            ),
        ]]
        .spacing(8)
        .width(Fill)
        .height(Fill);
        if searching {
            block = block.push(
                text_input("Search notes", &self.query)
                    .id(iced::widget::Id::new("note-search"))
                    .on_input(Message::QueryChanged)
                    .on_submit(Message::FieldDone)
                    .padding(10)
                    .size(14)
                    .style(field_style),
            );
        }
        if self.notes.is_empty() {
            let empty = if self.catalog.is_empty() {
                "No notes yet."
            } else {
                "No notes match."
            };
            block = block.push(text(empty).size(13).color(ink.muted));
        }
        let list = self.list_rows();
        block
            .push(vscroll(list).height(Fill).width(Fill))
            .push(action_fill("New note", Message::NewNote))
            .into()
    }

    /// A group in the list: a handle to fold it, its name, and how many notes are in it. The
    /// name shows only that group's notes.
    fn group_row(
        &self,
        path: &str,
        name: &str,
        depth: usize,
        count: usize,
        open: bool,
    ) -> Element<'_, Message> {
        let ink = self.ink();
        let mut line = row![].align_y(iced::Alignment::Center);
        for _ in 0..depth {
            line = line.push(space().width(14)).push(hairline_v());
        }
        line = line.push(mini_button(
            mini_handle(if open { Mini::Up } else { Mini::Down }, self.is_dark()),
            if open {
                "Fold this group"
            } else {
                "Open this group"
            },
            Message::Fold(path.to_owned()),
        ));
        let name_row = button(
            row![
                text(name.to_owned()).size(13).width(Fill),
                text(count.to_string()).size(12).color(ink.muted),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .padding([4.0, 8.0])
        .on_press(Message::PickTag(Some(path.to_owned())))
        .style(list_row(false));
        // Dropping a note on its own group's name puts it first.
        let top = top_of(path);
        let landing = self
            .list_drag
            .as_ref()
            .is_some_and(|drag| drag.group == path && drag.over.as_deref() == Some(top.as_str()));
        let line = line.push(
            mouse_area(name_row)
                .on_enter(Message::RowHover(top.clone(), true))
                .on_exit(Message::RowHover(top, false)),
        );
        if landing {
            return column![line, drop_bar()].into();
        }
        line.into()
    }

    /// One note in the list, `depth` groups in.
    /// The rows under the search field: groups and notes, or the matches.
    fn list_rows(&self) -> iced::widget::Column<'_, Message> {
        let ink = self.ink();
        let mut list = column![].spacing(2).width(Fill);
        if self.list_drag.is_some() {
            list = list.push(
                text("Drop it on a note to put it beside that note, or on the group's name to put it first.")
                    .size(12)
                    .color(ink.muted),
            );
        }
        if self.query.trim().is_empty() && self.tag.is_none() {
            if !self.catalog.iter().any(|note| !note.tags.is_empty()) && !self.catalog.is_empty() {
                list = list.push(
                    text("Tags make groups. Novel/Part 1 is a group inside Novel.")
                        .size(12)
                        .color(ink.muted),
                );
            }
            for line in group_outline(&self.notes, &self.folded, &self.orders) {
                match line {
                    ListLine::Group {
                        path,
                        name,
                        depth,
                        count,
                        open,
                    } => list = list.push(self.group_row(&path, &name, depth, count, open)),
                    ListLine::Note {
                        index,
                        depth,
                        group,
                    } => {
                        if let Some(note) = self.notes.get(index) {
                            list = list.push(self.note_row(note, depth, Some(&group)));
                        }
                    }
                }
            }
        } else {
            if let Some(tag) = &self.tag {
                list = list.push(
                    button(
                        row![
                            text(format!("In {tag}")).size(13).width(Fill),
                            text("Show all").size(12).color(ink.muted),
                        ]
                        .spacing(6),
                    )
                    .width(Fill)
                    .padding([4.0, 8.0])
                    .on_press(Message::PickTag(Some(tag.clone())))
                    .style(list_row(true)),
                );
                list = list.push(ghost_label(
                    "Read this group, one note after another",
                    Message::ToggleRead,
                ));
            }
            // One group in view: its notes in the order set here, and movable.
            let key = self
                .tag
                .as_ref()
                .map(|tag| group_key(&tag_path(tag)))
                .filter(|_| self.query.trim().is_empty());
            let mut shown: Vec<&NoteSummary> = self.notes.iter().collect();
            if let Some(key) = &key {
                let by_hand = self.orders.get(key).map_or(&[][..], Vec::as_slice);
                shown.sort_by(|a, b| {
                    let place = |id: &str| by_hand.iter().position(|have| have == id);
                    match (place(&a.id), place(&b.id)) {
                        (Some(x), Some(y)) => x.cmp(&y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => natural_cmp(&a.title, &b.title),
                    }
                });
            }
            for note in shown {
                list = list.push(self.note_row(note, 0, key.as_deref()));
            }
        }
        if !self.query.trim().is_empty() && !self.search_chats.is_empty() {
            list = list.push(text("Conversations").size(12).color(ink.muted).width(Fill));
            for chat in &self.search_chats {
                list = list.push(self.chat_hit_row(chat));
            }
        }
        list
    }

    /// One note in the list, `depth` groups in. `group` is the key of the group it is listed in,
    /// when it can be moved there.
    fn note_row(
        &self,
        note: &NoteSummary,
        depth: usize,
        group: Option<&str>,
    ) -> Element<'_, Message> {
        let ink = self.ink();
        let selected = self.selected.as_deref() == Some(note.id.as_str());
        let title = if note.title.is_empty() {
            "Untitled".to_owned()
        } else {
            note.title.clone()
        };
        let mut lines = column![text(title).font(serif()).size(17).width(Fill)].spacing(2);
        if !self.query.trim().is_empty()
            && let Some(line) = self
                .search_lines
                .get(&note.id)
                .filter(|line| !line.is_empty())
        {
            lines = lines.push(text(line.clone()).size(12).color(ink.muted).width(Fill));
        }
        let mut meta = note.writer.clone();
        if note.conflict {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            meta.push_str("conflict");
        }
        if !note.tags.is_empty() {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            for (index, tag) in note.tags.iter().take(3).enumerate() {
                if index > 0 {
                    meta.push(' ');
                }
                meta.push_str(tag);
            }
        }
        if !meta.is_empty() {
            lines = lines.push(text(meta).size(12).color(ink.muted).width(Fill));
        }
        let face = button(lines)
            .width(Fill)
            .padding(iced::Padding {
                top: 8.0,
                right: 10.0,
                bottom: 8.0,
                left: 10.0,
            })
            .on_press(Message::Select(note.id.clone()))
            .style(list_row(selected));
        let over = self.hover_row.as_deref() == Some(note.id.as_str());
        let movable = group.filter(|key| !key.is_empty() && has_exact_tag(&note.tags, key));
        let mut layers = stack![face];
        if over && self.list_drag.is_none() {
            let mut tools = row![].spacing(0).align_y(iced::Alignment::Center);
            if let Some(key) = movable {
                tools = tools.push(grip(mini_handle(Mini::Grip, self.is_dark()), key, &note.id));
            }
            tools = tools.push(mini_button(
                mini_handle(Mini::Trash, self.is_dark()),
                "Delete this note",
                Message::AskDeleteNote(note.id.clone()),
            ));
            layers = layers.push(
                container(tools)
                    .width(Fill)
                    .height(Fill)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Center)
                    .padding([0.0, 4.0]),
            );
        }
        let row_hit = mouse_area(layers)
            .on_enter(Message::RowHover(note.id.clone(), true))
            .on_exit(Message::RowHover(note.id.clone(), false));
        // A level in is a step right, and a thin line down the left shows which group it is in.
        let mut line = row![].align_y(iced::Alignment::Center);
        for _ in 0..depth {
            line = line.push(space().width(14)).push(hairline_v());
        }
        let line = line.push(row_hit);
        // While a note is being moved, a bar over the row it would land beside shows the place.
        let landing = self.list_drag.as_ref().is_some_and(|drag| {
            drag.over.as_deref() == Some(note.id.as_str()) && drag.id != note.id
        });
        if landing {
            return column![drop_bar(), line].into();
        }
        line.into()
    }

    fn chat_hit_row(&self, hit: &lane::ChatHit) -> Element<'_, Message> {
        let ink = self.ink();
        let label = self
            .aliases
            .get(&hit.id)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| hit.label.clone());
        let id = hit.id.clone();
        button(
            column![
                text(label).size(15).width(Fill),
                text(hit.line.clone()).size(12).color(ink.muted).width(Fill),
            ]
            .spacing(2),
        )
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(Message::SelectChat(id))
        .style(list_row(false))
        .into()
    }

    /// The Cite button of the add row. Its menu floats under it.
    fn task_cite_button(&self) -> Element<'_, Message> {
        anchored(
            ghost_label(
                if self.task_cite.is_some() {
                    "Cited"
                } else {
                    "Cite"
                },
                Message::ToggleTaskCite,
            ),
            self.task_cite_open.then(|| self.cite_menu(true)),
            Message::ToggleTaskCite,
        )
    }

    fn chat_cite_button(&self) -> Element<'_, Message> {
        anchored(
            panel_icon(
                mini_handle(Mini::Quote, self.is_dark()),
                if self.chat_cite.is_some() {
                    "A note is cited. Change it"
                } else {
                    "Cite a note"
                },
                Message::ToggleChatCite,
                self.chat_cite.is_some(),
            ),
            self.cite_open.then(|| self.cite_menu(false)),
            Message::ToggleChatCite,
        )
    }

    fn tasks_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let field = text_input("Add a task", &self.task_draft)
            .id(iced::widget::Id::new("task-input"))
            .on_input(Message::TaskDraft)
            .on_submit(Message::AddTask)
            .padding(10)
            .width(Fill)
            .style(field_style);
        // The field and its buttons share a line when there is room. When there is not, the
        // field has the line and the buttons have the one under it, so nothing is squeezed.
        let add: Element<'_, Message> = if self.pane_inner() < 560.0 {
            column![
                field,
                row![
                    self.due_button(CalFor::New, 8.0),
                    self.task_cite_button(),
                    space().width(Fill),
                    action_label("Add", Message::AddTask),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(Fill),
            ]
            .spacing(8)
            .width(Fill)
            .into()
        } else {
            row![
                field,
                self.due_button(CalFor::New, 10.0),
                action_label("Add", Message::AddTask),
                self.task_cite_button(),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Fill)
            .into()
        };
        let searching = self.task_search || !self.task_query.trim().is_empty();
        let tools = self.task_tools(searching);
        let list = self.task_groups(&ink);
        let mut page = column![].spacing(12).width(Fill).height(Fill);
        if self.compact() {
            page = page.push(Self::detail_header("‹ Tasks", Vec::new()));
        }
        page = page.push(add);
        if let Some(banner) = self.trip_banner() {
            page = page.push(banner);
        }
        if let Some((_, title)) = &self.task_cite {
            page = page.push(
                row![
                    text(title.clone()).size(13).width(Fill),
                    ghost_label("Remove", Message::ClearTaskCite),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        if searching {
            page = page.push(
                text_input("Search tasks", &self.task_query)
                    .id(iced::widget::Id::new("task-search"))
                    .on_input(Message::TaskQuery)
                    .padding(10)
                    .width(Fill)
                    .style(field_style),
            );
        }
        page = page.push(tools);
        if self.confirm_clear {
            page = page.push(self.clear_confirm());
        }
        let page = page.push(vscroll(list).height(Fill).width(Fill));
        if self.tasks_note_open {
            return stack![
                page,
                container(self.tasks_note_card())
                    .width(Fill)
                    .height(Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                    .padding(12),
            ]
            .width(Fill)
            .height(Fill)
            .into();
        }
        page.into()
    }

    fn task_groups(&self, ink: &Ink) -> Element<'_, Message> {
        let mut list = column![].spacing(14).width(Fill);
        let view = &self.task_view;
        let open: Vec<&TaskRow> = self
            .tasks
            .iter()
            .filter(|task| !task.done && self.task_hit(task) && self.in_view(task, view))
            .collect();
        // Done tasks sit under the open ones in the plain list, a group, or a note. In a view
        // about dates there are none, and in the Done view they are the list.
        let shows_done = matches!(
            view,
            TaskView::Open | TaskView::Done | TaskView::Group(_) | TaskView::Note(_)
        );
        let done: Vec<&TaskRow> = self
            .tasks
            .iter()
            .filter(|task| {
                task.done
                    && shows_done
                    && self.task_hit(task)
                    && self.in_view(task, view)
                    && (!self.hide_done || *view == TaskView::Done)
            })
            .collect();
        let heading = self.task_view_name();
        if self.tasks.is_empty() {
            list = list.push(text("Nothing to do yet.").font(serif()).size(32));
            list = list.push(
                text("Or write a line like - [ ] call the clinic in any note.")
                    .size(14)
                    .color(ink.muted),
            );
        } else if open.is_empty() && done.is_empty() {
            let empty = if self.task_query.trim().is_empty() {
                format!("Nothing in {heading}.")
            } else {
                "No tasks match.".to_owned()
            };
            list = list.push(text(empty).font(serif()).size(32));
        } else if open.is_empty() {
            list = list.push(
                text(format!("Nothing open in {heading}."))
                    .font(serif())
                    .size(32),
            );
        } else {
            list = list.push(text(heading.clone()).font(serif()).size(32));
            for task in open {
                list = list.push(self.task_row(task));
            }
        }
        if !done.is_empty() && *view != TaskView::Done {
            list = list.push(text("Done").font(serif()).size(22));
            for task in done {
                list = list.push(self.task_row(task));
            }
        }
        list.into()
    }

    /// Read the clock and the zone again, and keep what it says. Called when the Tasks tab
    /// opens, when the window comes back, every so often while the tab is up, and before
    /// anything that writes a day. Returns whether the day, the minute, or the zone changed.
    fn refresh_clock(&mut self) -> bool {
        if self.pinned_clock.is_some() {
            return false;
        }
        // The zone file is read each time, which is cheap, and parsed only when it is not the
        // one already held, so a zone changed by moving or by the system is seen at once.
        if let Some(found) = ambient_cli::host().time_zone() {
            if !self.zone.is_from(&found.tzif) {
                self.zone = zone::Zone::parse(&found.tzif).unwrap_or_else(zone::Zone::utc);
            }
            self.zone_name = found.name;
        } else {
            self.zone = zone::Zone::utc();
            self.zone_name = None;
        }
        let now = clock::Clock::read(&self.zone, self.zone_name.as_deref(), clock::now_ms());
        let changed = now != self.clock;
        self.clock = now;
        self.check_trip();
        changed
    }

    /// Something that tells one zone from another for as long as the zone stays the same: its name,
    /// or its short name and offset when it has none.
    fn zone_key(&self) -> String {
        self.zone_name
            .clone()
            .unwrap_or_else(|| format!("{} {}", self.clock.zone, self.clock.offset))
    }

    /// Notice a computer that is somewhere other than home. The first named zone this folder is
    /// opened in is taken as home, quietly. After that a different one is a trip, and is said, once
    /// for each zone, and times on tasks are left as they are: they follow the clock on the wall.
    fn check_trip(&mut self) {
        if self.notebook.is_none() {
            return;
        }
        if self.home_zone.is_empty() {
            if let Some(name) = self.zone_name.clone() {
                self.home_zone = name;
                self.persist_profile();
            }
            self.trip = None;
            return;
        }
        if self.zone_name.as_deref() == Some(self.home_zone.as_str()) {
            self.trip = None;
            return;
        }
        let home = ambient_cli::host()
            .zone_named(&self.home_zone)
            .and_then(|bytes| zone::Zone::parse(&bytes));
        self.trip = clock::away_from_home(
            self.zone_name.as_deref(),
            &self.zone,
            &self.home_zone,
            home.as_ref(),
            clock::now_ms(),
        );
    }

    fn draft(&self, whose: CalFor) -> &DueDraft {
        match whose {
            CalFor::New => &self.new_due,
            CalFor::Edit => &self.edit_due,
        }
    }

    fn draft_mut(&mut self, whose: CalFor) -> &mut DueDraft {
        match whose {
            CalFor::New => &mut self.new_due,
            CalFor::Edit => &mut self.edit_due,
        }
    }

    /// Whether a task belongs in the view. A done task is only in `Done`, `Group`, and `Note`
    /// and the plain list, so a date view is about what is still to do.
    fn in_view(&self, task: &TaskRow, view: &TaskView) -> bool {
        let state = task
            .due
            .map(|due| due.state(self.clock.today, self.clock.minutes));
        match view {
            TaskView::Open => true,
            TaskView::Done => task.done,
            TaskView::Today => !task.done && state == Some(due::State::Today),
            TaskView::Overdue => !task.done && state == Some(due::State::Late),
            TaskView::Upcoming => !task.done && state == Some(due::State::Later),
            TaskView::NoDate => !task.done && state.is_none(),
            TaskView::Group(key) => self
                .catalog
                .iter()
                .find(|note| note.id == task.note_id)
                .is_some_and(|note| in_group(&note.tags, key)),
            TaskView::Note(id) => task.note_id == *id,
        }
    }

    /// Search, hide done, clear done, and the Tasks note, in a row over the list.
    fn task_tools(&self, searching: bool) -> Element<'_, Message> {
        let mut tools = row![
            mini_button(
                mini_handle(Mini::Search, self.is_dark()),
                if searching { "Close search" } else { "Search" },
                Message::ToggleTaskSearch,
            ),
            ghost_label(
                if self.hide_done {
                    "Show done"
                } else {
                    "Hide done"
                },
                Message::ToggleDone,
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if self.tasks.iter().any(|task| task.done) {
            tools = tools.push(ghost_label("Clear done", Message::AskClearDone));
        }
        if self.catalog.iter().any(is_tasks_note) {
            tools = tools.push(ghost_label("The Tasks note", Message::ToggleTasksNote));
        }
        // Wraps onto a second line on a narrow window instead of running off the edge.
        tools.wrap().vertical_spacing(4).into()
    }

    /// The button that opens the calendar. It says which moment is picked, or asks for one.
    fn due_button(&self, whose: CalFor, pad: f32) -> Element<'_, Message> {
        let label = self.draft(whose).due().map_or_else(
            || "Due date".to_owned(),
            |found| format!("Due {}", self.due_words(&found, true)),
        );
        let face = button(text(label).size(14))
            .padding([pad, 12.0])
            .on_press(Message::OpenCalendar(whose))
            .style(ghost_style);
        // Under the button on a wide window. On a narrow one the calendar is a centered sheet,
        // drawn over the whole window by `modal`, so it is not given here.
        anchored(
            face,
            (self.cal == Some(whose) && !self.compact()).then(|| self.calendar_card(whose)),
            Message::CloseCalendar,
        )
    }

    /// A due moment for a person: `Today 14:30`, `Fri, Nov 27 · Every week`. With `short`, the day
    /// is `Nov 27` and not `Fri, Nov 27`, and a day within the week is its name.
    fn due_words(&self, found: &due::Due, short: bool) -> String {
        let today = self.clock.today;
        let mut words = if short {
            found.date.short(today)
        } else {
            found.date.relative(today)
        };
        if short && (0..=5).contains(&(found.date.serial() - today.serial())) {
            words = found.date.relative(today);
        }
        if let Some(time) = found.time {
            words.push(' ');
            words.push_str(&clock::clock_text(time.hour, time.minute));
        }
        if let Some(repeat) = found.repeat {
            words.push_str(" · ");
            words.push_str(&repeat.label());
        }
        words
    }

    /// A month to pick a day from, with a time and a repeat under it. The only way a due moment
    /// gets written, so it is always in the one form and always a day that exists. It is as wide
    /// as the room allows, up to a limit, and every row spreads across all of it.
    fn calendar_card(&self, whose: CalFor) -> Element<'_, Message> {
        let ink = self.ink();
        let today = self.clock.today;
        let draft = self.draft(whose);
        let chosen = dates::Date::parse(&draft.date);
        let (year, month) = self.cal_month;
        let name = dates::MONTHS
            .get(usize::try_from(month.saturating_sub(1)).unwrap_or(0))
            .copied()
            .unwrap_or("");
        let step = |glyph: &'static str, delta: i32| {
            button(
                container(text(glyph).size(18))
                    .width(Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(40)
            .padding([2, 0])
            .on_press(Message::CalMonth(delta))
            .style(ghost_style)
        };
        let quick = |label: &'static str, days: i64| {
            button(text(label).size(13))
                .padding([5, 10])
                .on_press(Message::PickQuick(days))
                .style(ghost_style)
        };
        let letters = if self.week_monday {
            ["M", "T", "W", "T", "F", "S", "S"]
        } else {
            ["S", "M", "T", "W", "T", "F", "S"]
        };
        let mut card = column![
            row![
                quick("Today", 0),
                quick("Tomorrow", 1),
                quick("Next week", 7),
                space().width(Fill),
                button(text("No date").size(13))
                    .padding([5, 10])
                    .on_press(Message::ClearDue)
                    .style(ghost_style),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
            hairline(),
            row![
                step("‹", -1),
                container(text(format!("{name} {year}")).font(serif()).size(18))
                    .width(Fill)
                    .align_x(iced::alignment::Horizontal::Center),
                step("›", 1),
            ]
            .align_y(iced::Alignment::Center),
            row(letters.map(|letter| {
                container(text(letter).size(12).color(ink.muted))
                    .width(Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .into()
            }))
            .spacing(2),
        ]
        .spacing(8)
        .width(Fill);
        for week in dates::month_grid(year, month, self.week_monday) {
            card = card.push(Self::calendar_week(&week, chosen, today));
        }
        card = card
            .push(hairline())
            .push(self.calendar_time_row(draft))
            .push(self.calendar_repeat_rows(draft))
            .push(hairline())
            .push(
                row![
                    text(format!(
                        "Now {} · {}",
                        self.clock.time_label(),
                        self.clock.offset_label()
                    ))
                    .size(12)
                    .color(ink.muted),
                    space().width(Fill),
                    button(text("Done").size(13))
                        .padding([5, 12])
                        .on_press(Message::CloseCalendar)
                        .style(ghost_style),
                ]
                .align_y(iced::Alignment::Center),
            );
        container(card)
            .width(Fill)
            .max_width(400)
            .padding(18)
            .style(card_style)
            .into()
    }

    /// Hours and minutes to step, or a few usual times when there is none. On a twelve hour clock
    /// the hour is read 1 to 12 with AM or PM beside it, and what is kept is still `HH:MM`.
    fn calendar_time_row(&self, draft: &DueDraft) -> Element<'_, Message> {
        let ink = self.ink();
        let small = |label: String, message: Message| {
            button(text(label).size(13))
                .padding([5, 10])
                .on_press(message)
                .style(ghost_style)
        };
        let head = text("TIME").size(11).color(ink.muted).width(52);
        let Some(time) = due::Time::parse(&draft.time) else {
            let chip = |hour: u32| {
                small(
                    clock::clock_text(hour, 0),
                    Message::SetTime(format!("{hour:02}:00")),
                )
            };
            return row![
                head,
                chip(9),
                chip(12),
                chip(17),
                space().width(Fill),
                text("no time").size(12).color(ink.muted),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center)
            .into();
        };
        let shown_hour = if self.hour12 {
            (time.hour + 11) % 12 + 1
        } else {
            time.hour
        };
        let hour_text = if self.hour12 {
            shown_hour.to_string()
        } else {
            format!("{shown_hour:02}")
        };
        let mut line = row![
            head,
            small(
                "‹".to_owned(),
                Message::StepTime {
                    hours: -1,
                    minutes: 0
                }
            ),
            text(hour_text).size(15),
            small(
                "›".to_owned(),
                Message::StepTime {
                    hours: 1,
                    minutes: 0
                }
            ),
            text(":").size(15),
            small(
                "‹".to_owned(),
                Message::StepTime {
                    hours: 0,
                    minutes: -5
                }
            ),
            text(format!("{:02}", time.minute)).size(15),
            small(
                "›".to_owned(),
                Message::StepTime {
                    hours: 0,
                    minutes: 5
                }
            ),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center);
        if self.hour12 {
            line = line.push(
                button(text(if time.hour < 12 { "AM" } else { "PM" }).size(13))
                    .padding([5, 10])
                    .on_press(Message::StepTime {
                        hours: 12,
                        minutes: 0,
                    })
                    .style(list_row(true)),
            );
        }
        line.push(space().width(Fill))
            .push(small("No time".to_owned(), Message::SetTime(String::new())))
            .into()
    }

    /// How often it comes round: never, or a unit, and how many of the unit.
    fn calendar_repeat_rows(&self, draft: &DueDraft) -> Element<'_, Message> {
        let ink = self.ink();
        let mut choices = row![text("REPEAT").size(11).color(ink.muted).width(52)]
            .spacing(2)
            .align_y(iced::Alignment::Center);
        let units = [
            ("Never", None),
            ("Daily", Some(due::Unit::Day)),
            ("Weekdays", Some(due::Unit::Weekday)),
            ("Weekly", Some(due::Unit::Week)),
            ("Monthly", Some(due::Unit::Month)),
            ("Yearly", Some(due::Unit::Year)),
        ];
        for (label, unit) in units {
            let chosen = draft.repeat.map(|repeat| repeat.unit) == unit;
            // Changing the unit keeps how many, except weekdays, which is only ever every one.
            let every = draft.repeat.map_or(1, |repeat| repeat.every);
            let make = unit.map(|unit| due::Repeat {
                every: if unit == due::Unit::Weekday { 1 } else { every },
                unit,
            });
            choices = choices.push(
                button(
                    container(text(label).size(12))
                        .width(Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                )
                .width(Fill)
                .padding([5, 2])
                .on_press(Message::SetRepeat(make))
                .style(list_row(chosen)),
            );
        }
        let mut rows = column![choices].spacing(6);
        if let Some(repeat) = draft
            .repeat
            .filter(|repeat| repeat.unit != due::Unit::Weekday)
        {
            let plural = match repeat.unit {
                due::Unit::Week => "weeks",
                due::Unit::Month => "months",
                due::Unit::Year => "years",
                _ => "days",
            };
            let small = |label: &'static str, delta: i32| {
                button(text(label).size(13))
                    .padding([5, 10])
                    .on_press(Message::RepeatEvery(delta))
                    .style(ghost_style)
            };
            rows = rows.push(
                row![
                    space().width(52),
                    text("every").size(13).color(ink.muted),
                    small("‹", -1),
                    text(repeat.every.to_string()).size(15),
                    small("›", 1),
                    text(if repeat.every == 1 {
                        plural.trim_end_matches('s')
                    } else {
                        plural
                    })
                    .size(13)
                    .color(ink.muted),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            );
        }
        rows.into()
    }

    /// One row of the calendar: seven days sharing the width, the picked one filled and today
    /// outlined.
    fn calendar_week(
        week: &[Option<dates::Date>; 7],
        chosen: Option<dates::Date>,
        today: dates::Date,
    ) -> Element<'static, Message> {
        let cells = week.map(|cell| -> Element<'static, Message> {
            let Some(day) = cell else {
                return space().width(Fill).height(36).into();
            };
            let picked = chosen == Some(day);
            let now = day == today;
            button(
                container(text(day.day.to_string()).size(14))
                    .width(Fill)
                    .height(Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(Fill)
            .height(36)
            .padding(0)
            .on_press(Message::PickDay(day.to_string()))
            .style(move |theme: &Theme, status| {
                let palette = ink_of(theme);
                let mut style = list_row(picked)(theme, status);
                if now && !picked {
                    style.border = iced::Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: palette.accent,
                    };
                }
                style
            })
            .into()
        });
        row(cells).spacing(2).into()
    }

    /// What the list on the right is called, from the view chosen on the left.
    fn task_view_name(&self) -> String {
        match &self.task_view {
            TaskView::Open => "Open".to_owned(),
            TaskView::Today => "Due today".to_owned(),
            TaskView::Overdue => "Overdue".to_owned(),
            TaskView::Upcoming => "Upcoming".to_owned(),
            TaskView::NoDate => "No date".to_owned(),
            TaskView::Done => "Done".to_owned(),
            TaskView::Group(key) => self
                .catalog
                .iter()
                .flat_map(|note| note.tags.iter())
                .find(|tag| group_key(&tag_path(tag)) == *key)
                .and_then(|tag| tag_path(tag).last().map(|last| (*last).to_owned()))
                .unwrap_or_else(|| key.clone()),
            TaskView::Note(id) => self.catalog.iter().find(|note| note.id == *id).map_or_else(
                || "that note".to_owned(),
                |note| {
                    if note.title.is_empty() {
                        "Untitled".to_owned()
                    } else {
                        note.title.clone()
                    }
                },
            ),
        }
    }

    fn task_hit(&self, task: &TaskRow) -> bool {
        let query = self.task_query.trim();
        if query.is_empty() {
            return true;
        }
        let query = query.to_lowercase();
        task.text.to_lowercase().contains(&query) || task.note_title.to_lowercase().contains(&query)
    }

    /// The line under a task that says when it is due, and whether that has passed. A late one is
    /// in the one color the window has for that, and says so in words too.
    fn due_line(&self, found: &due::Due, done: bool) -> Element<'_, Message> {
        let ink = self.ink();
        let late = !done && found.state(self.clock.today, self.clock.minutes) == due::State::Late;
        let words = self.due_words(found, false);
        if late {
            text(format!("Overdue · {words}"))
                .size(12)
                .color(OVERDUE)
                .into()
        } else {
            text(format!("Due {words}"))
                .size(12)
                .color(ink.muted)
                .into()
        }
    }

    fn task_row(&self, task: &TaskRow) -> Element<'_, Message> {
        let ink = self.ink();
        let id = task.note_id.clone();
        let line = task.line;
        let open_id = task.note_id.clone();
        let title = if task.note_title.is_empty() {
            "Untitled".to_owned()
        } else {
            task.note_title.clone()
        };
        let editing = self
            .task_rename
            .as_ref()
            .is_some_and(|(note, row)| note == &task.note_id && *row == task.line);
        let words = if task.done { ink.muted } else { ink.text };
        let main: Element<'_, Message> = if editing {
            row![
                text_input("Task", &self.task_rename_text)
                    .on_input(Message::TaskLabel)
                    .on_submit(Message::CommitTaskRename)
                    .padding(8)
                    .width(Fill)
                    .style(field_style),
                self.due_button(CalFor::Edit, 8.0),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            let words_text = markdown::task_words(&task.text);
            let label = if words_text.is_empty() {
                "Empty task".to_owned()
            } else {
                words_text
            };
            let mut lines = column![text(label).size(16).color(words)]
                .spacing(2)
                .width(Fill);
            if let Some(found) = task.due {
                lines = lines.push(self.due_line(&found, task.done));
            }
            lines = lines.push(text(title).size(12).color(ink.muted));
            button(lines)
                .width(Fill)
                .padding([2, 0])
                .on_press(Message::Select(open_id))
                .style(quiet_style)
                .into()
        };
        let line_row = row![
            checkbox(task.done).on_toggle(move |done| Message::SetTask {
                id: id.clone(),
                line,
                done,
            }),
            main,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        let mut block = column![line_row].spacing(4).width(Fill);
        for (target, title) in markdown::task_cites(&task.text) {
            let target = target.clone();
            block = block.push(
                button(text(title).size(13))
                    .padding([2, 0])
                    .on_press(Message::OpenCited(target))
                    .style(quiet_style),
            );
        }
        block.push(self.task_actions(task, editing)).into()
    }

    fn task_actions(&self, task: &TaskRow, editing: bool) -> Element<'_, Message> {
        let line = task.line;
        let dark = self.is_dark();
        if editing {
            return row![ghost_label("Cancel", Message::CancelTaskEdit)]
                .spacing(8)
                .into();
        }
        let mut actions = row![].spacing(4);
        if self.task_neighbor(task, true) {
            actions = actions.push(mini_button(
                mini_handle(Mini::Up, dark),
                "Up",
                Message::MoveTask {
                    id: task.note_id.clone(),
                    line,
                    up: true,
                },
            ));
        }
        if self.task_neighbor(task, false) {
            actions = actions.push(mini_button(
                mini_handle(Mini::Down, dark),
                "Down",
                Message::MoveTask {
                    id: task.note_id.clone(),
                    line,
                    up: false,
                },
            ));
        }
        actions = actions.push(mini_button(
            mini_handle(Mini::Pencil, dark),
            "Rename",
            Message::BeginTaskEdit {
                id: task.note_id.clone(),
                line,
            },
        ));
        actions = actions.push(mini_button(
            mini_handle(Mini::Trash, dark),
            "Remove",
            Message::RemoveTask {
                id: task.note_id.clone(),
                line,
            },
        ));
        actions.into()
    }

    fn rebuild_tasks(&mut self) {
        let selected = self.selected.clone();
        let unsaved = self.unsaved.clone();
        let mut rows = Vec::new();
        if let Some(book) = self.notebook.as_ref() {
            let _ = book.visit_held(|id, title, body, conflict| {
                if selected.as_deref() == Some(id) {
                    return;
                }
                if let Some((_, draft_title, draft_body)) = unsaved
                    .iter()
                    .find(|(key, _, _)| key.as_deref() == Some(id))
                {
                    push_tasks(&mut rows, id, draft_title, draft_body);
                    return;
                }
                if !conflict {
                    push_tasks(&mut rows, id, title, body);
                }
            });
        }
        if let Some(id) = &selected
            && self.conflicts.is_empty()
        {
            push_tasks(&mut rows, id, &self.title_text, &self.body.text());
        }
        rows.sort_by(|left, right| {
            left.done
                .cmp(&right.done)
                .then_with(|| match (left.due, right.due) {
                    (Some(earlier), Some(later)) => earlier.key().cmp(&later.key()),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
                .then_with(|| left.note_title.cmp(&right.note_title))
                .then_with(|| left.line.cmp(&right.line))
        });
        self.tasks = rows;
    }

    fn task_source(&self, id: &str) -> Option<(String, bool, String)> {
        if self.selected.as_deref() == Some(id) {
            return Some((
                self.body.text(),
                !self.conflicts.is_empty(),
                self.title_text.clone(),
            ));
        }
        if let Some((_, title, body)) = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            return Some((body.clone(), false, title.clone()));
        }
        let shown = self.notebook.as_ref()?.show_held(id).ok()?;
        let head = shown.heads.first()?;
        Some((head.body.clone(), shown.conflict, head.title.clone()))
    }

    /// The note's text with this task checked or unchecked. A task that repeats is not checked
    /// off: it is set for its next day and left open, so it comes back when it is due. The second
    /// part is that next moment, when it was moved.
    fn checked_or_advanced(
        &mut self,
        body: &str,
        line: usize,
        done: bool,
    ) -> Option<(String, Option<due::Due>)> {
        if done {
            let found = markdown::task_lines(body)
                .into_iter()
                .find(|task| task.line == line && !task.done);
            let current = found
                .as_ref()
                .and_then(|task| due::Due::of(&markdown::without_cites(&task.text)))
                .filter(|current| current.repeat.is_some());
            if let (Some(task), Some(current)) = (found, current) {
                // Read the clock first: "the next one after today" has to mean the real today.
                self.refresh_clock();
                if let Some(moved) = current.advanced(self.clock.today) {
                    let mut label =
                        due::Due::write(&markdown::task_words(&task.text), Some(&moved));
                    for mark in markdown::cite_marks(&task.text) {
                        label.push(' ');
                        label.push_str(&mark);
                    }
                    return markdown::rename_task_line(body, line, &label)
                        .map(|next| (next, Some(moved)));
                }
            }
        }
        markdown::set_task_line(body, line, done).map(|next| (next, None))
    }

    fn set_task(&mut self, id: &str, line: usize, done: bool) -> Task<Message> {
        let Some((body, conflict, title)) = self.task_source(id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let Some((next, moved)) = self.checked_or_advanced(&body, line, done) else {
            return Task::none();
        };
        if let Some(moved) = moved {
            let words = self.due_words(&moved, false);
            self.say(&format!("Done. It repeats: next {words}."));
        }
        self.put_note_text(id, &title, &next)
    }

    fn commit_task_rename(&mut self) -> Task<Message> {
        let Some((id, line)) = self.task_rename.clone() else {
            return Task::none();
        };
        let label = self.task_rename_text.clone();
        let marks = self
            .tasks
            .iter()
            .find(|task| task.note_id == id && task.line == line)
            .map(|task| markdown::cite_marks(&task.text))
            .unwrap_or_default();
        let due = self.edit_due.due();
        if label.trim().is_empty() && due.is_none() {
            self.say("Write the task first.");
            return Task::none();
        }
        let mut label = due::Due::write(&label, due.as_ref());
        for mark in marks {
            label.push(' ');
            label.push_str(&mark);
        }
        let Some((body, conflict, title)) = self.task_source(&id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(&id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        self.task_rename = None;
        self.task_rename_text.clear();
        self.edit_due.clear();
        let Some(next) = markdown::rename_task_line(&body, line, &label) else {
            return Task::none();
        };
        self.store_task_edit(&id, &title, &next)
    }

    fn remove_listed_task(&mut self, id: &str, line: usize) -> Task<Message> {
        if self
            .task_rename
            .as_ref()
            .is_some_and(|(note, row)| note == id && *row == line)
        {
            self.task_rename = None;
            self.task_rename_text.clear();
        }
        let Some((body, conflict, title)) = self.task_source(id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let Some(next) = markdown::remove_task_line(&body, line) else {
            return Task::none();
        };
        self.store_task_edit(id, &title, &next)
    }

    fn move_listed_task(&mut self, id: &str, line: usize, up: bool) -> Task<Message> {
        let Some((body, conflict, title)) = self.task_source(id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let Some(next) = markdown::move_task_line(&body, line, up) else {
            return Task::none();
        };
        self.store_task_edit(id, &title, &next)
    }

    fn clear_done_tasks(&mut self) -> Task<Message> {
        self.confirm_clear = false;
        let mut by_note: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for task in &self.tasks {
            if task.done {
                by_note
                    .entry(task.note_id.clone())
                    .or_default()
                    .push(task.line);
            }
        }
        let mut writes = Vec::new();
        for (id, mut lines) in by_note {
            lines.sort_unstable();
            lines.dedup();
            let Some((body, conflict, title)) = self.task_source(&id) else {
                continue;
            };
            if conflict {
                self.show_note(&id);
                self.say("Two versions. Write the one you want to keep, then save.");
                continue;
            }
            let mut next = body.clone();
            for line in lines.into_iter().rev() {
                if let Some(updated) = markdown::remove_task_line(&next, line) {
                    next = updated;
                }
            }
            if next != body {
                writes.push(self.store_task_edit(&id, &title, &next));
            }
        }
        if writes.is_empty() {
            Task::none()
        } else {
            self.say("Cleared. The older lines stay in the log.");
            Task::batch(writes)
        }
    }

    fn task_neighbor(&self, task: &TaskRow, up: bool) -> bool {
        self.tasks.iter().any(|other| {
            other.note_id == task.note_id
                && if up {
                    other.line < task.line
                } else {
                    other.line > task.line
                }
        })
    }

    fn clear_confirm(&self) -> Element<'_, Message> {
        let ink = self.ink();
        column![
            text(
                "Clear done tasks? Those lines leave their notes. The older copy stays in the log."
            )
            .size(14)
            .color(ink.muted),
            row![
                action_label("Clear done", Message::ClearDone),
                ghost_label("Cancel", Message::CancelClearDone),
            ]
            .spacing(8),
        ]
        .spacing(8)
        .into()
    }

    fn tasks_tab(&self) -> String {
        let open = self.tasks.iter().filter(|task| !task.done).count();
        if open == 0 {
            "Tasks".to_owned()
        } else {
            format!("Tasks {open}")
        }
    }

    fn chat_tab(&self) -> String {
        let fresh = self
            .chats
            .iter()
            .filter(|chat| self.chat_is_new(&chat.id))
            .count();
        if fresh == 0 {
            "Chat".to_owned()
        } else {
            format!("Chat {fresh}")
        }
    }

    fn chat_is_new(&self, id: &str) -> bool {
        match (
            self.chat_count.get(id).copied(),
            self.seen_count.get(id).copied(),
        ) {
            (Some(count), Some(seen)) => count > seen,
            _ => false,
        }
    }

    fn store_task_edit(&mut self, id: &str, title: &str, next: &str) -> Task<Message> {
        let follow = self.put_note_text(id, title, next);
        if self.editor_dirty() && self.selected.as_deref() == Some(id) {
            Task::batch([follow, self.start_save(true)])
        } else {
            follow
        }
    }

    fn put_note_text(&mut self, id: &str, title: &str, next: &str) -> Task<Message> {
        if self.selected.as_deref() == Some(id) {
            let title = title.to_owned();
            let next = next.to_owned();
            self.revise(Burst::Break, |app| {
                let before = app.body.text();
                let before_title = app.title_text.clone();
                let cursor = app.body.cursor();
                app.title_text.clone_from(&title);
                app.body.load(&next);
                app.body.place(cursor);
                app.body.text() != before || app.title_text != before_title
            });
            if self.screen == Screen::Tasks {
                self.rebuild_tasks();
            }
            return Task::none();
        }
        if let Some(slot) = self
            .unsaved
            .iter_mut()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            title.clone_into(&mut slot.1);
            next.clone_into(&mut slot.2);
        }
        self.apply_note(Some(id.to_owned()), title.to_owned(), next.to_owned())
    }

    fn main(&self) -> Element<'_, Message> {
        let body = match self.sheet {
            Sheet::Invite => self.invite_sheet(),
            Sheet::Settings => self.settings_sheet(),
            Sheet::None if self.screen == Screen::Chat => self.chat_pane(),
            Sheet::None if self.screen == Screen::Tasks => self.tasks_pane(),
            Sheet::None if self.graph_open && !self.compact() => self.graph_pane(),
            Sheet::None => self.notes_pane(),
        };
        container(body)
            .padding([self.gutter(), self.gutter_x()])
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn unlock_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        container(
            column![
                text("This folder is locked").font(serif()).size(32),
                text("The passphrase unwraps the key file for this run. It is not the root, not the recovery phrase, and not a platform passkey.")
                    .size(14)
                    .color(ink.muted),
                text_input("Passphrase", &self.unlock_input)
                    .on_input(Message::UnlockChanged)
                    .on_submit(Message::SubmitUnlock)
                    .secure(true)
                    .padding(10)
                    .style(field_style),
                action_label("Unlock", Message::SubmitUnlock),
                text(self.message.as_str()).size(13).color(ink.muted),
            ]
            .spacing(14)
            .max_width(460),
        )
        .padding(48)
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn notes_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![].spacing(14).height(Fill);
        if self.compact() && self.detail {
            body = body.push(Self::detail_header(
                "‹ Notes",
                vec![
                    ghost_label("New", Message::NewNote),
                    mini_button(
                        mini_handle(Mini::Gear, self.is_dark()),
                        "Links and note settings",
                        Message::ToggleContext,
                    ),
                ],
            ));
        }
        if !self.open_error.is_empty() {
            body = body.push(text(format!(
                "Could not open this folder. {}",
                self.open_error
            )));
        }
        if !self.conflicts.is_empty() {
            let mut versions = column![
                text("Two versions. Write the one you want to keep, then save.").color(ink.muted)
            ]
            .spacing(8);
            for (index, (writer, title, note_body)) in self.conflicts.iter().enumerate() {
                let number = index.saturating_add(1);
                versions = versions.push(text(format!("Version {number} · {writer}: {title}")));
                versions = versions.push(text(note_body).color(ink.muted));
            }
            body = body.push(vscroll(versions).height(160));
        }
        body = body.push(self.note_column());
        let page: Element<'_, Message> = body.into();
        self.focus_chrome(page)
    }

    /// The icon at the top right of a note that goes in and out of focus. In focus it has the
    /// small title beside it, and a button that brings the formatting bar in or sends it away.
    fn focus_chrome<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        if self.compact() || self.reading || self.older.is_some() || !self.editing_note() {
            return page;
        }
        let mut corner = row![].spacing(6).align_y(iced::Alignment::Center);
        if self.focused() {
            corner = corner
                .push(
                    text_input("Title", &self.title_text)
                        .id(iced::widget::Id::new("note-title"))
                        .on_input(Message::TitleChanged)
                        .on_submit(Message::TitleSubmit)
                        .font(serif())
                        .size(15)
                        .padding([2.0, 0.0])
                        .width(220)
                        .align_x(iced::alignment::Horizontal::Right)
                        .style(title_style),
                )
                .push(ghost_label("Aa", Message::FocusBar));
        }
        corner = corner.push(mini_button(
            mini_handle(Mini::Focus, self.is_dark()),
            if self.focus {
                "Leave focus"
            } else {
                "Write without distractions"
            },
            Message::ToggleFocus,
        ));
        stack![
            page,
            container(corner)
                .width(Fill)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Top)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 2.0,
                    bottom: 0.0,
                    left: 0.0,
                }),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn delete_confirm(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let saved = self.selected.is_some();
        let prompt = if saved {
            "Delete this note? It leaves the list. Unsaved edits are dropped. The older copy stays in the log."
        } else {
            "Discard this unsaved note?"
        };
        let verb = if saved { "Delete note" } else { "Discard" };
        container(
            column![
                text(prompt).size(14).color(ink.muted),
                row![
                    ghost_label("Cancel", Message::CancelDelete),
                    space().width(Fill),
                    action_label(verb, Message::DeleteNote),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(12),
        )
        .width(Fill)
        .max_width(320)
        .padding(14)
        .style(card_style)
        .into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut lines = column![].spacing(8);
        if self.chat.is_empty() && self.pending_files.is_empty() {
            lines = lines.push(text("No messages yet.").color(ink.muted));
            lines = lines.push(action_label("Invite someone", Message::ShowInvite));
        }
        let query = self.chat_query.trim().to_lowercase();
        let width = self.bubble_width();
        let group = self
            .chats
            .iter()
            .find(|chat| self.chat_id.as_deref() == Some(chat.id.as_str()))
            .is_some_and(|chat| chat.members > 2);
        let mut day = String::new();
        let mut shown = 0_usize;
        let mut previous = String::new();
        for line in &self.chat {
            if !query.is_empty() && !line.text.to_lowercase().contains(&query) {
                continue;
            }
            shown = shown.saturating_add(1);
            if !line.day.is_empty() && line.day != day {
                day.clone_from(&line.day);
                lines = lines.push(
                    container(text(&line.day).size(12).color(ink.muted))
                        .width(Fill)
                        .align_x(iced::alignment::Horizontal::Center)
                        .padding([6, 0]),
                );
            }
            let show_who = group && !line.mine && line.sender != previous;
            previous.clone_from(&line.sender);
            lines = lines.push(self.chat_bubble(line, width, show_who));
        }
        if shown == 0 && !query.is_empty() {
            lines = lines.push(text("No messages match.").color(ink.muted));
        }
        let composer = self.composer();
        let mut page = column![].spacing(12).width(Fill).height(Fill);
        if self.compact() {
            page = page.push(Self::detail_header("‹ Chats", Vec::new()));
        }
        if self.chat_id.is_some() {
            page = page.push(self.chat_header());
        }
        page = page.push(
            vscroll(lines.width(Fill))
                .height(Fill)
                .width(Fill)
                .anchor_bottom(),
        );
        page = page.push(composer);
        container(page).width(Fill).height(Fill).clip(true).into()
    }

    fn composer(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let field = text_editor::TextEditor::new(&self.draft_editor)
            .placeholder("Message")
            .height(72)
            .padding(10)
            .on_action(Message::DraftEdited)
            .key_binding(composer_binding)
            .style(field_editor_style);
        // Icons, so the box has the width. The same line at every width.
        let controls: Element<'_, Message> = row![
            panel_icon(
                mini_handle(Mini::Plus, self.is_dark()),
                "Add a file",
                Message::PickFile,
                false,
            ),
            self.chat_cite_button(),
            field,
            panel_icon(
                mini_handle(Mini::Send, self.is_dark()),
                "Send",
                Message::SendChat,
                false,
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::End)
        .width(Fill)
        .into();
        let mut composer = column![].spacing(8).width(Fill);
        if !self.pending_files.is_empty() {
            let mut chips = column![].spacing(4);
            for (index, path) in self.pending_files.iter().enumerate() {
                let name = path
                    .file_name()
                    .and_then(|text| text.to_str())
                    .unwrap_or("file");
                chips = chips.push(
                    row![
                        text(name).size(13).width(Fill),
                        ghost_label_owned("Remove".to_owned(), Message::DropPending(index)),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }
            composer = composer.push(chips);
            composer = composer.push(
                text(
                    "The file is sealed to the other people in this conversation on the next sync.",
                )
                .size(12)
                .color(ink.muted),
            );
        }
        if let Some((_, title)) = &self.chat_cite {
            composer = composer.push(
                row![
                    text(title.clone()).size(13).width(Fill),
                    ghost_label("Remove", Message::ClearChatCite),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        composer
            .push(controls)
            .push(
                text("Shift+Enter for a new line.")
                    .size(12)
                    .color(ink.muted),
            )
            .into()
    }

    fn chat_header(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let Some(id) = &self.chat_id else {
            return space().height(0).into();
        };
        let editing = self.alias_editing.as_deref() == Some(id.as_str());
        let searching = self.chat_search || !self.chat_query.trim().is_empty();
        let search = mini_button(
            mini_handle(Mini::Search, self.is_dark()),
            if searching { "Close search" } else { "Search" },
            Message::ToggleChatSearch,
        );
        // The name is plain text. The pencil turns it into a field, so the header does not
        // look like a form every time the chat is open.
        let mut head = column![].spacing(4);
        let open = self.chats.iter().find(|chat| &chat.id == id).cloned();
        if editing {
            if let Some(chat) = open.clone().filter(|chat| chat.members > 2) {
                head = head.push(self.group_fields(&chat, search));
            } else {
                let draft = self.aliases.get(id).cloned().unwrap_or_default();
                let id = id.clone();
                head = head
                    .push(
                        row![
                            text_input("What do you call them?", &draft)
                                .id(iced::widget::Id::new("chat-alias"))
                                .on_input(move |text| Message::AliasDraft(id.clone(), text))
                                .on_submit(Message::FinishAlias)
                                .padding(10)
                                .width(Fill)
                                .style(field_style),
                            ghost_label("Done", Message::FinishAlias),
                            search,
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    )
                    .push(
                        text("Only this computer remembers it.")
                            .size(12)
                            .color(ink.muted),
                    );
            }
        } else {
            let name = open
                .as_ref()
                .map_or_else(|| "Conversation".to_owned(), |chat| self.chat_name(chat));
            head = head.push(
                row![
                    text(name).font(serif()).size(22).width(Fill),
                    mini_button(
                        mini_handle(Mini::Pencil, self.is_dark()),
                        if open.as_ref().is_some_and(|chat| chat.members > 2) {
                            "Names"
                        } else {
                            "Rename"
                        },
                        Message::EditAlias,
                    ),
                    search,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        if let Some(chat) = open {
            if chat.members > 2 && !editing {
                let who = self.who_line(&chat);
                if !who.is_empty() {
                    head = head.push(text(who).size(12).color(ink.muted));
                }
            }
            if let Some(row) = self.add_row(&chat) {
                head = head.push(row);
            }
        }
        if searching {
            head = head.push(
                text_input("Search this conversation", &self.chat_query)
                    .id(iced::widget::Id::new("chat-search"))
                    .on_input(Message::ChatQuery)
                    .padding(10)
                    .style(field_style),
            );
        }
        head.into()
    }

    /// Add someone, under the conversation name. Nothing when this window did not start it.
    fn add_row(&self, chat: &ambient_cli::ConversationSummary) -> Option<Element<'_, Message>> {
        if !chat.started {
            return None;
        }
        let ink = self.ink();
        let mut rows = column![].spacing(4);
        if self.adding {
            rows = rows
                .push(
                    text("Paste their Invite link. They wait. They do not start a conversation.")
                        .size(13)
                        .color(ink.muted),
                )
                .push(
                    text_input("Their link", &self.add_draft)
                        .on_input(Message::AddDraft)
                        .padding(10)
                        .style(field_style),
                )
                .push(
                    text("What do you call them? Leave it blank to use the name on their link. Only this computer remembers it.")
                        .size(13)
                        .color(ink.muted),
                )
                .push(
                    text_input("A name on this computer", &self.add_call)
                        .on_input(Message::AddCall)
                        .padding(10)
                        .style(field_style),
                )
                .push(
                    row![
                        action_label("Add", Message::AddToChat),
                        ghost_label("Cancel", Message::ToggleAdd),
                    ]
                    .spacing(8),
                );
        } else {
            rows = rows.push(ghost_label("Add someone", Message::ToggleAdd));
        }
        Some(rows.into())
    }

    /// The conversation's name, then one field for each other person.
    fn group_fields<'a>(
        &self,
        chat: &ambient_cli::ConversationSummary,
        search: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let ink = self.ink();
        let id = chat.id.clone();
        let draft = self.aliases.get(&chat.id).cloned().unwrap_or_default();
        let mut body = column![
            text_input("This conversation", &draft)
                .id(iced::widget::Id::new("chat-alias"))
                .on_input(move |text| Message::AliasDraft(id.clone(), text))
                .on_submit(Message::FinishAlias)
                .padding(10)
                .width(Fill)
                .style(field_style),
            text("Only this computer remembers these names.")
                .size(12)
                .color(ink.muted),
        ]
        .spacing(8);
        for key in &chat.others {
            let draft = self.people.get(key).cloned().unwrap_or_default();
            let short: String = key.chars().take(8).collect();
            let shown = key.clone();
            body = body.push(text(short).size(12).color(ink.muted));
            body = body.push(
                text_input("A name on this computer", &draft)
                    .on_input(move |text| Message::PersonDraft(shown.clone(), text))
                    .padding(10)
                    .style(field_style),
            );
            if let Some(line) = self.card_lines.get(key) {
                body = body.push(text(line.clone()).size(12).color(ink.muted));
            }
        }
        body = body.push(
            row![ghost_label("Done", Message::FinishAlias), search]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        );
        body.into()
    }

    fn person_name(&self, key: &str) -> String {
        self.people
            .get(key)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| key.chars().take(8).collect())
    }

    /// "3 people · Sam, Rin" for a group. Empty for a 1:1.
    fn who_line(&self, chat: &ambient_cli::ConversationSummary) -> String {
        if chat.members <= 2 {
            return String::new();
        }
        let names: Vec<String> = chat
            .others
            .iter()
            .map(|key| self.person_name(key))
            .collect();
        if names.is_empty() {
            return format!("{} people", chat.members);
        }
        format!("{} people · {}", chat.members, names.join(", "))
    }

    fn invite_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut sheet = column![
            Self::sheet_title("Invite"),
            text("One of you starts. The other waits.")
                .size(14)
                .color(ink.muted),
            row![
                choice_button(
                    "I have their link",
                    self.invite_pick == Some(InvitePick::Theirs),
                    Message::PickInvite(InvitePick::Theirs),
                ),
                choice_button(
                    "Send them my link",
                    self.invite_pick == Some(InvitePick::Mine),
                    Message::PickInvite(InvitePick::Mine),
                ),
            ]
            .spacing(8)
            .width(Fill),
        ]
        .spacing(14);
        sheet = match self.invite_pick {
            Some(InvitePick::Theirs) => sheet.push(self.invite_theirs()),
            Some(InvitePick::Mine) => sheet.push(self.invite_mine()),
            None => sheet.push(
                text("Pick one. You do not do both.")
                    .size(13)
                    .color(ink.muted),
            ),
        };
        vscroll(sheet).height(Fill).into()
    }

    fn invite_theirs(&self) -> Element<'_, Message> {
        let ink = self.ink();
        column![
            text("Paste the link they sent you. You start the conversation. They wait.")
                .size(14)
                .color(ink.muted),
            text_input("Their link", &self.their_card)
                .on_input(Message::TheirCardChanged)
                .padding(10)
                .style(field_style),
            text("What do you call them? Leave it blank to use the name on their link. Only this computer remembers it.")
                .size(13)
                .color(ink.muted),
            text_input("A name on this computer", &self.call_draft)
                .on_input(Message::CallDraft)
                .padding(10)
                .style(field_style),
            action_label("Start conversation", Message::StartConversation),
        ]
        .spacing(12)
        .into()
    }

    fn invite_mine(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![
            text("Copy this and send it. They paste it and start. You wait. Do not paste anything here.")
                .size(14)
                .color(ink.muted),
            text("Your link").size(12).color(ink.muted),
            container(text(&self.contact_link).size(14))
                .padding(14)
                .width(Fill)
                .style(field_well),
            row![
                ghost_label("Copy link", Message::CopyLink),
                ghost_label(
                    if self.show_qr { "Hide QR" } else { "Show QR" },
                    Message::ToggleQr,
                ),
            ]
            .spacing(8),
        ]
        .spacing(12);
        if self.show_qr
            && let Some(bytes) = &self.qr_bytes
        {
            body = body.push(
                iced_image::Image::new(iced_image::Handle::from_bytes(bytes.clone()))
                    .width(200)
                    .height(200),
            );
        }
        body = body.push(
            text("They show up in Chat once they start. Nothing else happens on this page.")
                .size(13)
                .color(ink.muted),
        );
        body.into()
    }

    fn devices_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body =
            column![
            text("A passphrase locks the key file on this computer. It is not the recovery phrase.")
                .size(14)
                .color(ink.muted),
            text(self.dir.display().to_string())
                .size(13)
                .color(ink.muted),
            text_input("Passphrase", &self.unlock_input)
                .on_input(Message::UnlockChanged)
                .secure(true)
                .padding(12)
                .style(field_style),
            action_label("Lock the key file", Message::SubmitUnlock),
        ]
            .spacing(12);
        body = body.push(text("Computers").size(13).color(ink.muted));
        if self.devices.is_empty() {
            body = body.push(text("No other computers yet.").size(14).color(ink.muted));
        }
        let can_revoke = self.notebook.as_ref().is_some_and(Notebook::holds_root);
        for device in &self.devices {
            body = body.push(Self::device_row(device, can_revoke));
        }
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            body = body.push(
                text("Another window on this computer does not hold the main key. Revoke only stops new notes. Notes already written stay.")
                    .size(14)
                    .color(ink.muted),
            );
            body = body.push(
                row![
                    ghost_label("Open the other window", Message::OpenPaired),
                    ghost_label("Open another person", Message::OpenPerson),
                ]
                .spacing(8),
            );
            body = body.push(ghost_label("Open one more person", Message::OpenOneMore));
        }
        settings_card("Devices", body.into())
    }

    fn device_row(device: &ambient_cli::DeviceLine, can_revoke: bool) -> Element<'static, Message> {
        let state = if device.revoked {
            "Revoked"
        } else if device.current {
            "This computer"
        } else {
            "Active"
        };
        let label = device.label.clone();
        let mut line = row![
            column![
                text(label.clone()).size(16),
                text(state).size(12).style(|theme: &Theme| {
                    let ink = ink_of(theme);
                    iced::widget::text::Style {
                        color: Some(ink.muted),
                    }
                }),
            ]
            .spacing(2)
            .width(Fill)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);
        if can_revoke && !device.current && !device.revoked {
            line = line.push(ghost_label_owned(
                format!("Revoke {label}"),
                Message::Revoke(device.id.clone()),
            ));
        }
        line.into()
    }

    fn recovery_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![
            text("This phrase can rebuild the notebook on a new computer. It does not bring back old device keys. Keep it offline.")
                .size(14)
                .color(ink.muted),
            action_label("Show recovery phrase", Message::ShowPhrase),
        ]
        .spacing(14);
        if !self.recovery_phrase.is_empty() {
            body = body.push(
                container(text(&self.recovery_phrase).size(15))
                    .padding(14)
                    .width(Fill)
                    .style(field_well),
            );
            body = body.push(ghost_label("Copy phrase", Message::CopyPhrase));
        }
        body = body.push(
            text_input("Paste a recovery phrase", &self.restore_phrase)
                .on_input(Message::RestorePhraseChanged)
                .padding(12)
                .style(field_style),
        );
        body = body.push(action_label(
            "Restore on a new computer",
            Message::StartRestore,
        ));
        settings_card("Recovery", body.into())
    }

    fn sheet_title(title: &str) -> Element<'static, Message> {
        let title = title.to_owned();
        row![
            text(title).font(serif()).size(32).width(Fill),
            ghost_label("Back", Message::CloseSheet),
        ]
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn reload(&mut self) {
        let found = {
            let Some(book) = self.notebook.as_ref() else {
                self.tasks.clear();
                self.catalog.clear();
                self.notes.clear();
                return;
            };
            book.list_held()
        };
        match found {
            Ok(notes) => self.catalog = notes,
            Err(err) => self.message = err.to_string(),
        }
        if self
            .tag
            .as_ref()
            .is_some_and(|tag| !self.catalog.iter().any(|note| in_group(&note.tags, tag)))
        {
            self.tag = None;
        }
        self.refresh_shown();
        if self.screen == Screen::Tasks {
            self.rebuild_tasks();
        }
    }

    fn refresh_shown(&mut self) {
        let base = if self.query.is_empty() {
            self.catalog.clone()
        } else if let Some(hits) = &self.search_hits {
            hits.clone()
        } else {
            let needle = self.query.to_lowercase();
            self.catalog
                .iter()
                .filter(|note| note.title.to_lowercase().contains(&needle))
                .cloned()
                .collect()
        };
        // The Tasks note is the Tasks tab's. It is not in the list of notes.
        let base = base.into_iter().filter(|note| !is_tasks_note(note));
        self.notes = match &self.tag {
            Some(tag) => base.filter(|note| in_group(&note.tags, tag)).collect(),
            None => base.collect(),
        };
    }

    /// The Tasks note's words, for showing it over the Tasks tab.
    fn graph_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let Some(data) = self.graph.clone() else {
            return space().into();
        };
        let (dots, lines) = (data.nodes.len(), data.edges.len());
        let summary = format!(
            "{dots} {} · {lines} {}",
            if dots == 1 { "note" } else { "notes" },
            if lines == 1 { "link" } else { "links" },
        );
        let head: Element<'_, Message> = if self.compact() {
            Self::detail_header(
                "‹ Notes",
                vec![text(summary).size(13).color(ink.muted).into()],
            )
        } else {
            row![
                text(summary).size(13).color(ink.muted).width(Fill),
                ghost_label("Back to the note", Message::ToggleGraph),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        };
        column![
            head,
            container(
                canvas::Canvas::new(GraphView {
                    data,
                    selected: self.selected.clone(),
                })
                .width(Fill)
                .height(Fill),
            )
            .padding(4)
            .width(Fill)
            .height(Fill)
            .style(card_style),
        ]
        .spacing(12)
        .into()
    }

    fn tasks_note_text(&self) -> Option<String> {
        let id = self
            .catalog
            .iter()
            .find(|note| is_tasks_note(note))
            .map(|note| note.id.clone())?;
        self.task_source(&id).map(|(body, _, _)| body)
    }

    fn tasks_note_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let body = self.tasks_note_text().unwrap_or_default();
        let shown = if body.trim().is_empty() {
            "Nothing in it yet.".to_owned()
        } else {
            body
        };
        container(
            column![
                row![
                    text("The Tasks note").font(serif()).size(18),
                    space().width(Fill),
                    ghost_label("Close", Message::ToggleTasksNote),
                ]
                .align_y(iced::Alignment::Center),
                text("Every task added here is a line in it. Check one in the list, not here.")
                    .size(12)
                    .color(ink.muted),
                vscroll(text(shown).size(14).width(Fill))
                    .height(Fill)
                    .width(Fill),
            ]
            .spacing(10),
        )
        .width(Fill)
        .max_width(460)
        .height(360)
        .padding(16)
        .style(card_style)
        .into()
    }

    fn reading_chat(&self) -> bool {
        self.screen == Screen::Chat && self.sheet == Sheet::None && (!self.compact() || self.detail)
    }

    fn mark_open_chat(&mut self) {
        let Some(id) = self.chat_id.clone() else {
            return;
        };
        let Some(count) = self.chat_count.get(&id).copied() else {
            return;
        };
        if self.seen_count.get(&id).copied() == Some(count) {
            return;
        }
        self.seen_count.insert(id, count);
        self.persist_seen();
    }

    fn persist_seen(&mut self) {
        let rows: Vec<(String, usize)> = self
            .seen_count
            .iter()
            .map(|(id, count)| (id.clone(), *count))
            .collect();
        if let Err(err) = profile::store_seen(&self.dir, &rows) {
            self.message = err;
        }
    }

    fn show_note(&mut self, id: &str) {
        self.graph_open = false;
        self.graph = None;
        self.older = None;
        self.version_wait = None;
        self.send_open = false;
        self.confirm_delete = false;
        if self.selected.as_deref() == Some(id) && self.editor_dirty() {
            self.screen = Screen::Notes;
            self.sheet = Sheet::None;
            return;
        }
        if self.selected.as_deref() != Some(id) {
            self.remember_unsaved();
            self.find_at = usize::MAX;
            self.swap_history(id);
        }
        let stash = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.as_deref() == Some(id))
            .map(|(_, title, body)| (title.clone(), body.clone()));
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.show_held(id) {
            Ok(note) => {
                let draft = Draft::from_note(&note);
                self.selected = Some(id.to_owned());
                self.disk_title.clone_from(&draft.title);
                self.disk_body.clone_from(&draft.body);
                if let Some((title, body)) = stash {
                    self.title_text = title;
                    self.load_sealed(&body);
                    self.arm_pause();
                } else {
                    self.title_text.clone_from(&draft.title);
                    self.load_sealed(&draft.body);
                    self.disarm_pause();
                }
                self.conflicts = draft.conflicts;
                self.note_writer = note
                    .heads
                    .first()
                    .map(|head| head.writer.clone())
                    .unwrap_or_default();
                self.save_token = self.save_token.wrapping_add(1);
                self.screen = Screen::Notes;
                self.sheet = Sheet::None;
                self.detail = true;
                self.message.clear();
                self.reading = false;
                self.post_side();
                self.reset_proof();
                self.maybe_proof();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn clear_editor(&mut self) {
        self.older = None;
        self.version_wait = None;
        self.send_open = false;
        self.confirm_delete = false;
        self.remember_unsaved();
        self.find_at = usize::MAX;
        self.swap_history("");
        self.save_token = self.save_token.wrapping_add(1);
        let stash = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.is_none())
            .map(|(_, title, body)| (title.clone(), body.clone()));
        self.selected = None;
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
        self.reading = false;
        if let Some((title, body)) = stash {
            self.title_text = title;
            self.load_sealed(&body);
            self.arm_pause();
            self.reset_proof();
            self.maybe_proof();
            self.post_side();
            return;
        }
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disarm_pause();
        self.message.clear();
        self.reset_proof();
        self.maybe_proof();
        self.post_side();
    }

    fn remember_unsaved(&mut self) {
        if !self.editor_dirty() {
            return;
        }
        let key = self.selected.clone();
        self.unsaved.retain(|(id, _, _)| id != &key);
        self.unsaved
            .push((key, self.title_text.clone(), self.stored_body()));
    }

    fn stored_body(&self) -> String {
        ambient_cli::join_note_tags(&self.body.text(), &self.tags)
    }

    fn load_sealed(&mut self, sealed: &str) {
        let (prose, tags) = ambient_cli::split_note_tags(sealed);
        self.tags = tags;
        self.tag_draft.clear();
        self.body.load(&prose);
    }

    fn editor_dirty(&self) -> bool {
        self.title_text != self.disk_title || self.stored_body() != self.disk_body
    }

    fn add_tag(&mut self) {
        let Some(tag) = clean_tag(&self.tag_draft) else {
            return;
        };
        if self.tags.iter().any(|have| have.eq_ignore_ascii_case(&tag)) {
            self.tag_draft.clear();
            return;
        }
        self.tags.push(tag);
        self.tag_draft.clear();
        self.arm_pause();
    }

    fn remove_tag(&mut self, tag: &str) {
        let before = self.tags.len();
        self.tags.retain(|have| !have.eq_ignore_ascii_case(tag));
        if self.tags.len() != before {
            self.arm_pause();
        }
    }

    /// `2 links · 1 linked from`, under the tags, when the note has any. It opens the panel.
    fn link_counts(&self) -> Option<Element<'_, Message>> {
        let (out, back) = (self.links_to.len(), self.cached_backlinks.len());
        if out == 0 && back == 0 {
            return None;
        }
        let mut parts = Vec::new();
        if out > 0 {
            parts.push(format!("{out} {}", if out == 1 { "link" } else { "links" }));
        }
        if back > 0 {
            parts.push(format!("{back} linked from"));
        }
        Some(
            button(text(parts.join(" · ")).size(12).color(self.ink().muted))
                .padding(0)
                .on_press(Message::ShowLinks)
                .style(ghost_style)
                .into(),
        )
    }

    fn tag_chips(&self) -> Element<'_, Message> {
        let mut chips = row![].spacing(6).align_y(iced::Alignment::Center);
        for tag in &self.tags {
            let label = tag.clone();
            chips = chips.push(
                button(text(format!("{label}  ×")).size(12))
                    .padding([4, 8])
                    .on_press(Message::RemoveTag(label))
                    .style(ghost_style),
            );
        }
        chips = chips.push(
            text_input("Add a tag or group", &self.tag_draft)
                .id(iced::widget::Id::new("note-tag"))
                .on_input(Message::TagDraft)
                .on_submit(Message::AddTag)
                .padding([4, 8])
                .size(13)
                .width(140)
                .style(field_style),
        );
        // The same two rows whether or not there is anything to offer. The field is in the first,
        // and if the shape of what holds it changed on the first key, the field would be built
        // again, lose the caret, and the next keys would go to the page.
        let offered = self.tag_suggestions();
        let mut more = row![].spacing(6).align_y(iced::Alignment::Center);
        if !offered.is_empty() {
            more = more.push(text("Used before").size(12).color(self.ink().muted));
        }
        for tag in offered {
            more = more.push(
                button(text(tag.clone()).size(12))
                    .padding([3, 8])
                    .on_press(Message::PickSuggestion(tag))
                    .style(ghost_style),
            );
        }
        column![chips.wrap().vertical_spacing(4), more]
            .spacing(0)
            .into()
    }

    /// Tags already used in other notes that fit what is typed, and are not on this note. Those
    /// that start with it come first, then those that contain it, then the most used.
    fn tag_suggestions(&self) -> Vec<String> {
        let typed = self.tag_draft.trim().trim_start_matches('#').to_lowercase();
        if typed.is_empty() {
            return Vec::new();
        }
        let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for note in &self.catalog {
            for tag in &note.tags {
                let entry = counts
                    .entry(tag.to_lowercase())
                    .or_insert_with(|| (tag.clone(), 0));
                entry.1 = entry.1.saturating_add(1);
            }
        }
        let mut fits: Vec<(bool, usize, String)> = counts
            .into_iter()
            .filter(|(key, _)| {
                key.contains(typed.as_str())
                    && *key != typed
                    && !self.tags.iter().any(|have| have.to_lowercase() == *key)
            })
            .map(|(key, (shown, count))| (key.starts_with(typed.as_str()), count, shown))
            .collect();
        fits.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(b.1.cmp(&a.1))
                .then_with(|| a.2.to_lowercase().cmp(&b.2.to_lowercase()))
        });
        fits.into_iter().take(6).map(|(_, _, tag)| tag).collect()
    }

    fn post(&self, job: lane::Job) {
        let _ = self.lane.send(job);
    }

    fn post_side(&self) {
        self.post(lane::Job::Side(lane::OpenSnap {
            selected: self.selected.clone(),
            title: self.title_text.clone(),
            stored: self.stored_body(),
            unsaved: self.unsaved.clone(),
        }));
    }

    fn sync_watch(&self) {
        let id = if self.screen == Screen::Chat {
            self.chat_id.clone()
        } else {
            None
        };
        self.post(lane::Job::Watch(id));
    }

    fn apply_side(&mut self, side: lane::SideDone) {
        self.links_to = side.links_to;
        self.graph_edges = side.edges;
        self.cached_backlinks = side.backlinks;
        self.known_links = Some(Arc::new(side.known));
        markdown::set_known_links(self.known_links.clone());
        self.refresh_graph();
    }

    fn refresh_graph(&mut self) {
        self.graph = self
            .graph_open
            .then(|| Arc::new(build_graph(&self.catalog, &self.graph_edges)));
    }

    fn install_latest(&mut self) -> bool {
        let Some((generation, book)) = lane::take_latest(self.book_gen) else {
            return false;
        };
        self.book_gen = generation;
        self.devices = book.devices_held().unwrap_or_default();
        if let Ok(notes) = book.list_held() {
            self.catalog = notes;
            self.refresh_shown();
        }
        self.notebook = Some(book);
        true
    }

    fn apply_inbox(&mut self, rows: &[lane::ChatRow]) {
        self.chats = rows
            .iter()
            .map(|row| ambient_cli::ConversationSummary {
                id: row.id.clone(),
                label: row.label.clone(),
                other: row.other.clone(),
                members: row.members,
                started: row.started,
                others: row.others.clone(),
            })
            .collect();
        let known = self
            .chat_id
            .as_ref()
            .is_some_and(|id| self.chats.iter().any(|chat| &chat.id == id));
        if !known {
            self.chat_id = self.chats.first().map(|chat| chat.id.clone());
            if self.screen == Screen::Chat && self.chat_id.is_some() {
                self.post(lane::Job::Pull);
            }
        }
        self.chat_last.clear();
        let mut learned = false;
        for row in rows {
            self.chat_count.insert(row.id.clone(), row.count);
            if let std::collections::btree_map::Entry::Vacant(slot) =
                self.seen_count.entry(row.id.clone())
            {
                slot.insert(row.count);
                learned = true;
            }
            if row.count > 0 {
                self.chat_last
                    .insert(row.id.clone(), (row.preview.clone(), row.time.clone()));
            }
        }
        if learned {
            self.persist_seen();
        }
        if self.reading_chat() {
            self.mark_open_chat();
        }
    }

    fn apply_lane(&mut self, done: lane::Done) {
        match done {
            lane::Done::Side(side) => self.apply_side(side),
            lane::Done::Chat { id, messages } => {
                if self.chat_id.as_deref() == Some(id.as_str()) {
                    self.chat = bubbles(messages);
                    if self.reading_chat() {
                        self.mark_open_chat();
                    }
                }
            }
            lane::Done::Inbox(rows) => self.apply_inbox(&rows),
            lane::Done::Found {
                query,
                notes,
                chats,
            } => {
                if self.query == query {
                    self.search_lines = notes
                        .iter()
                        .map(|hit| (hit.note.id.clone(), hit.line.clone()))
                        .collect();
                    self.search_chats = chats;
                    self.search_hits = Some(notes.into_iter().map(|hit| hit.note).collect());
                    self.refresh_shown();
                }
            }
            lane::Done::Proof {
                text,
                warning,
                words,
            } => self.apply_proof(&text, warning, words),
            lane::Done::Version {
                id,
                title,
                body,
                current,
            } => self.apply_version(id, title, body, current),
            lane::Done::SendFailed { text, message } => {
                self.message = message;
                if self.draft_editor.text().trim().is_empty() {
                    self.draft_editor = text_editor::Content::with_text(&text);
                }
            }
            lane::Done::Notice(text) => self.message = text,
            lane::Done::Synced {
                detail,
                failed,
                transport,
                pending,
                writes,
                notes,
                side,
            } => {
                self.syncing = false;
                if failed {
                    self.pending = true;
                    if transport {
                        self.net_up = false;
                        if self.net_mode == ambient_cli::NetMode::ThisComputer
                            && detail.contains("unreachable")
                            && !self.relay_ours
                        {
                            self.relay_ready = false;
                        }
                        "No relay answered.".clone_into(&mut self.sync_detail);
                    } else {
                        self.sync_detail = detail;
                    }
                    return;
                }
                self.net_up = true;
                self.sync_detail = detail.clone();
                self.pending = pending || self.writes != writes;
                let installed = self.install_latest();
                if content_arrived(&detail) {
                    if !installed {
                        self.catalog = notes;
                        self.refresh_shown();
                    }
                    if self.screen == Screen::Tasks {
                        self.rebuild_tasks();
                    }
                    self.apply_side(side);
                    self.refresh_open_note();
                }
            }
        }
    }

    fn apply_proof(&mut self, text: &str, warning: Option<String>, words: BTreeSet<String>) {
        if self.body.text() != text {
            return;
        }
        self.proof_warning = warning.unwrap_or_default();
        self.misspellings = Arc::new(words);
        markdown::set_misspellings(Arc::clone(&self.misspellings));
    }

    fn apply_version(&mut self, id: String, title: String, body: String, current: bool) {
        if self.version_wait.as_deref() != Some(id.as_str()) {
            return;
        }
        self.version_wait = None;
        if current {
            self.older = None;
            self.show_note(&id);
            return;
        }
        self.older = Some(OlderNote { id, title, body });
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
        self.reading = false;
    }

    fn note_named(&self, title: &str) -> Option<(String, String)> {
        let title = title.trim();
        if title.is_empty() || markdown::is_note_id(title) {
            return None;
        }
        self.catalog.iter().find_map(|note| {
            note.title
                .eq_ignore_ascii_case(title)
                .then(|| (note.id.clone(), note.title.clone()))
        })
    }

    fn complete_link(&mut self, id: &str, version: Option<&str>) -> Task<Message> {
        let Some(title) = self
            .catalog
            .iter()
            .find(|note| note.id == id)
            .map(|note| note.title.clone())
        else {
            return Task::none();
        };
        let text = self.body.text();
        let caret = self.body.caret_byte();
        let Some((start, _)) = markdown::open_wiki(&text, caret) else {
            return Task::none();
        };
        let label = title.replace(['|', '[', ']'], "");
        let insert = match version {
            Some(version) => format!("[[{id}@{version}|{label}]]"),
            None => format!("[[{id}|{label}]]"),
        };
        let mut next = String::new();
        next.push_str(text.get(..start).unwrap_or(""));
        next.push_str(&insert);
        next.push_str(text.get(caret..).unwrap_or(""));
        let at = start.saturating_add(insert.len());
        self.revise(Burst::Break, |app| {
            app.body.load(&next);
            app.body.place_byte(at);
            true
        });
        self.note_follow(false)
    }

    fn maybe_proof(&mut self) {
        if self.reading || self.older.is_some() {
            return;
        }
        let text = self.body.text();
        if text == self.proof_body {
            return;
        }
        self.proof_body.clone_from(&text);
        self.post(lane::Job::Proof(text));
    }

    fn reset_proof(&mut self) {
        self.proof_body.clear();
        self.proof_warning.clear();
        self.misspellings = Arc::new(BTreeSet::new());
        markdown::set_misspellings(Arc::clone(&self.misspellings));
    }

    fn close_version(&mut self) {
        let id = self.older.as_ref().map(|note| note.id.clone());
        self.older = None;
        if let Some(id) = id {
            self.show_note(&id);
        }
    }

    fn older_page(&self, older: &OlderNote) -> Element<'_, Message> {
        let ink = self.ink();
        let title = if older.title.is_empty() {
            "Untitled".to_owned()
        } else {
            older.title.clone()
        };
        let page = column![
            text("This is an older version.").size(13).color(ink.muted),
            ghost_label("Current note", Message::CloseVersion),
            text(title)
                .font(serif())
                .size(if self.compact() { 32.0 } else { 36.0 }),
            vscroll(text(older.body.clone()).size(16)).height(Fill),
        ]
        .spacing(12)
        .width(Fill)
        .height(Fill);
        self.writing_frame(page)
    }

    fn open_note_link(&mut self, name: &str, version: Option<&str>) {
        let id = if markdown::is_note_id(name) {
            self.catalog
                .iter()
                .find(|note| note.id == name)
                .map(|note| note.id.clone())
        } else {
            self.catalog
                .iter()
                .find(|note| !note.title.is_empty() && note.title.eq_ignore_ascii_case(name))
                .map(|note| note.id.clone())
        };
        let Some(id) = id else {
            if markdown::is_note_id(name) {
                self.say("That note is gone.");
            } else {
                self.say("No note with that name.");
            }
            return;
        };
        let Some(version) = version else {
            self.show_note(&id);
            return;
        };
        if self
            .catalog
            .iter()
            .any(|note| note.id == id && note.version == version)
        {
            self.show_note(&id);
            return;
        }
        self.post(lane::Job::Version {
            id: id.clone(),
            version: version.to_owned(),
        });
        self.version_wait = Some(id);
    }

    fn pick_cite(&mut self, id: &str, task: bool) {
        let Some(note) = self.catalog.iter().find(|note| note.id == id) else {
            return;
        };
        let chosen = (note.id.clone(), note.title.clone());
        if task {
            self.task_cite = Some(chosen);
            self.task_cite_open = false;
        } else {
            self.chat_cite = Some(chosen);
            self.cite_open = false;
        }
    }

    fn cite_menu(&self, task: bool) -> Element<'_, Message> {
        let ink = self.ink();
        let mut notes: Vec<_> = self
            .catalog
            .iter()
            .filter(|note| !note.title.is_empty())
            .collect();
        notes.sort_by(|left, right| left.title.cmp(&right.title));
        notes.truncate(8);
        if notes.is_empty() {
            return text("No notes to cite.").size(12).color(ink.muted).into();
        }
        let mut list = column![].spacing(2).width(Fill);
        for note in notes {
            let id = note.id.clone();
            list = list.push(
                button(text(note.title.clone()).size(14).width(Fill))
                    .width(Fill)
                    .padding([4, 8])
                    .on_press(if task {
                        Message::PickTaskCite(id)
                    } else {
                        Message::PickChatCite(id)
                    })
                    .style(list_row(false)),
            );
        }
        container(list)
            .padding(6)
            .width(280)
            .style(card_style)
            .into()
    }

    fn link_menu(&self, query: &str) -> Element<'_, Message> {
        let ink = self.ink();
        let needle = query.trim().to_lowercase();
        let mut matches: Vec<&NoteSummary> = self
            .catalog
            .iter()
            .filter(|note| {
                !note.title.is_empty()
                    && (needle.is_empty() || note.title.to_lowercase().contains(&needle))
                    && self.selected.as_deref() != Some(note.id.as_str())
            })
            .take(6)
            .collect();
        matches.sort_by(|left, right| {
            let left_rank = left.title.to_lowercase().starts_with(&needle);
            let right_rank = right.title.to_lowercase().starts_with(&needle);
            right_rank
                .cmp(&left_rank)
                .then_with(|| left.title.cmp(&right.title))
        });
        if matches.is_empty() {
            return text("No note with that name yet. Close the link and it stays unresolved.")
                .size(12)
                .color(ink.muted)
                .into();
        }
        let mut list = column![].spacing(2).width(Fill);
        for note in matches {
            let id = note.id.clone();
            let version = note.version.clone();
            let mut line = row![
                button(text(note.title.clone()).size(14).width(Fill))
                    .width(Fill)
                    .padding([4, 8])
                    .on_press(Message::CompleteLink(id.clone()))
                    .style(list_row(false))
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            if !note.conflict && markdown::is_note_id(&version) {
                line = line.push(
                    button(text("version").size(12))
                        .padding([4, 8])
                        .on_press(Message::CompleteVersion { id, version })
                        .style(ghost_style),
                );
            }
            list = list.push(line);
        }
        container(list)
            .padding(6)
            .width(280)
            .style(card_style)
            .into()
    }

    fn ink(&self) -> Ink {
        Ink::from_dark(self.is_dark())
    }

    fn is_dark(&self) -> bool {
        match self.appearance {
            profile::Appearance::Light => false,
            profile::Appearance::Dark => true,
            profile::Appearance::System => self.system_dark,
        }
    }

    fn arm_pause(&mut self) {
        self.edited_at = Some(Instant::now());
        self.pause_token = self.pause_token.wrapping_add(1);
        self.pause_armed = true;
    }

    fn disarm_pause(&mut self) {
        self.edited_at = None;
        self.pause_armed = false;
        self.pause_token = self.pause_token.wrapping_add(1);
    }

    fn arm_search(&mut self) {
        self.search_token = self.search_token.wrapping_add(1);
        self.search_armed = true;
    }

    fn disarm_search(&mut self) {
        self.search_armed = false;
        self.search_token = self.search_token.wrapping_add(1);
    }

    fn start_save(&mut self, force: bool) -> Task<Message> {
        if self.notebook.is_none() || !self.editor_dirty() {
            return Task::none();
        }
        if self.saving {
            self.save_again = true;
            return Task::none();
        }
        if !force
            && self
                .edited_at
                .is_none_or(|at| at.elapsed() < Duration::from_millis(700))
        {
            return Task::none();
        }
        self.maybe_proof();
        self.post_side();
        self.saving = true;
        self.save_again = false;
        let token = self.save_token;
        save_task(
            self.dir.clone(),
            self.selected.clone(),
            self.title_text.clone(),
            self.stored_body(),
            token,
        )
    }

    fn begin_sync(&mut self) -> Task<Message> {
        if self.notebook.is_none()
            || self.syncing
            || (self.net_mode == ambient_cli::NetMode::ThisComputer && !self.relay_ready)
        {
            return Task::none();
        }
        self.syncing = true;
        self.post(lane::Job::Sync {
            writes: self.writes,
        });
        Task::none()
    }

    fn refresh_open_note(&mut self) {
        if self.editor_dirty() {
            return;
        }
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        let Ok(note) = book.show_held(&id) else {
            return;
        };
        let draft = Draft::from_note(&note);
        let writer = note
            .heads
            .first()
            .map(|head| head.writer.clone())
            .unwrap_or_default();
        if draft.title == self.disk_title
            && draft.body == self.disk_body
            && draft.conflicts == self.conflicts
        {
            self.note_writer = writer;
            return;
        }
        self.note_writer = writer;
        self.conflicts = draft.conflicts;
        self.disk_title.clone_from(&draft.title);
        self.disk_body.clone_from(&draft.body);
        self.title_text.clone_from(&draft.title);
        self.load_sealed(&draft.body);
        self.disarm_pause();
        self.post_side();
    }

    fn compact(&self) -> bool {
        self.size.width < 720.0
    }

    /// Whether the panel on the right can sit beside the page. There is room for it, and for the
    /// list, and for a page worth writing on, from this wide. Narrower, it comes in over the page
    /// as a sheet, so it never squeezes the writing.
    fn side_by_side_context(&self) -> bool {
        !self.compact() && self.size.width >= 1020.0
    }

    fn side_width(&self) -> f32 {
        if self.size.width < 1040.0 {
            210.0
        } else {
            280.0
        }
    }

    /// Width of the middle pane, inside its padding. The list and the group panel take their share first.
    fn pane_inner(&self) -> f32 {
        if self.compact() {
            return (self.size.width - self.gutter_x() * 2.0).max(160.0);
        }
        let mut chrome = self.gutter_x() * 2.0 + 28.0;
        if self.show_list {
            chrome += self.side_width() + 1.0;
        }
        if self.context_room() {
            chrome += 28.0;
            if self.show_context && self.side_by_side_context() {
                chrome += CONTEXT_WIDTH + 1.0;
            }
        }
        (self.size.width - chrome).max(160.0)
    }

    /// The writing column. It grows with the middle pane, and stops at a readable measure.
    fn writing_column(&self) -> f32 {
        self.pane_inner().min(720.0)
    }

    /// Where a paragraph wraps. A little inside the column, so the scrollbar is not the margin.
    fn writing_width(&self) -> f32 {
        (self.writing_column() - 8.0).max(120.0)
    }

    fn gutter(&self) -> f32 {
        if self.compact() { 12.0 } else { 22.0 }
    }

    fn gutter_x(&self) -> f32 {
        if self.compact() {
            12.0
        } else if self.size.width < 1000.0 {
            20.0
        } else {
            32.0
        }
    }

    /// Writing with nothing else in view. Only in the wide layout, and only for a note being
    /// written, not a book being read or an older version.
    /// How much of the side panels is in view, 0 to 1. In focus they are away. Leaving focus,
    /// or turning on a layout where focus does not apply, brings them back.
    fn panel_shown(&self) -> f32 {
        if self.focused() || self.slide_panels.moving() {
            self.slide_panels.shown()
        } else {
            1.0
        }
    }

    fn focused(&self) -> bool {
        self.focus
            && self.editing_note()
            && !self.compact()
            && !self.reading
            && self.older.is_none()
    }

    fn editing_note(&self) -> bool {
        self.sheet == Sheet::None
            && self.screen == Screen::Notes
            && (!self.compact() || self.detail)
    }

    fn chat_inner(&self) -> f32 {
        let chrome = if self.compact() {
            self.gutter_x() * 2.0
        } else {
            let side = if self.show_list {
                self.side_width() + 1.0
            } else {
                0.0
            };
            side + self.gutter_x() * 2.0
        };
        (self.size.width - chrome).max(120.0)
    }

    fn bubble_width(&self) -> f32 {
        let inner = self.chat_inner();
        (inner - 36.0).clamp(96.0, 460.0)
    }

    /// The formatting bar. In focus it slides down from the top edge of the page, or away.
    fn format_slot(&self) -> Element<'static, Message> {
        if !self.focused() {
            return Self::format_bar();
        }
        let shown = self.slide_bar.shown();
        if shown <= 0.0 {
            return space().height(0).into();
        }
        if shown >= 1.0 {
            return Self::format_bar();
        }
        // The bar is kept against the bottom of a box that grows, so it comes down into view.
        // It is clipped, and it is only in the tree while it is partly or fully shown.
        container(Self::format_bar())
            .width(Fill)
            .height(FORMAT_BAR * shown)
            .align_y(iced::alignment::Vertical::Bottom)
            .clip(true)
            .into()
    }

    fn format_bar() -> Element<'static, Message> {
        hscroll(
            row![
                format_caption("Word"),
                format_button("B", "Bold", FormatKind::Bold),
                format_button("I", "Italic", FormatKind::Italic),
                format_button("S", "Strike", FormatKind::Strike),
                format_button("==", "Highlight", FormatKind::Highlight),
                format_button("</>", "Code", FormatKind::Code),
                space().width(14),
                format_caption("Line"),
                format_button("H", "Heading", FormatKind::Heading),
                format_button("List", "List", FormatKind::List),
                format_button("Task", "Task", FormatKind::Task),
                format_button("Quote", "Quote", FormatKind::Quote),
                space().width(14),
                format_caption("Insert"),
                format_button("Link", "Link", FormatKind::Link),
                format_button("[[]]", "Link a note", FormatKind::Wiki),
                format_button("Image", "Image", FormatKind::Image),
                format_button("Table", "Table", FormatKind::Table),
                format_button("—", "Rule", FormatKind::Rule),
                format_button("Callout", "Callout", FormatKind::Callout),
                format_button("```", "Code block", FormatKind::CodeBlock),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([2, 0]),
        )
        .width(Fill)
        .into()
    }

    fn bottom_bar(&self) -> Element<'_, Message> {
        let mut bar = column![].width(Fill);
        if self.editing_note() {
            bar = bar.push(Self::format_bar()).push(hairline());
        }
        let tasks_name = self.tasks_tab();
        let chat_name = self.chat_tab();
        let mut nav = row![
            segment(
                "Notes",
                self.screen == Screen::Notes && self.sheet == Sheet::None,
                Message::ShowNotes,
            ),
            segment(
                &tasks_name,
                self.screen == Screen::Tasks && self.sheet == Sheet::None,
                Message::ShowTasks,
            ),
            segment(
                &chat_name,
                self.screen == Screen::Chat && self.sheet == Sheet::None,
                Message::ShowChat,
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        if self.notebook.is_some() && !self.editor_dirty() {
            nav = nav.push(text("✓").size(18).color(Color::from_rgb8(46, 140, 96)));
        }
        nav = nav.push(icon_button(
            gear_handle(self.is_dark()),
            Message::ShowSettings,
        ));
        nav = nav.push(icon_button(
            mode_handle(self.is_dark()),
            Message::ToggleTheme,
        ));
        nav = nav.push(anchored(
            icon_button(globe_handle(self.globe_color()), Message::ToggleSyncPop),
            self.sync_open.then(|| self.sync_card()),
            Message::ToggleSyncPop,
        ));
        bar = bar.push(container(nav).padding([6, 8]).width(Fill));
        bar.into()
    }

    fn compact_title(&self) -> Element<'_, Message> {
        let ink = self.ink();
        row![
            text("Aerodash").font(serif()).size(20),
            text(self.message.as_str())
                .size(12)
                .color(ink.muted)
                .width(Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding([8, 12])
        .into()
    }

    fn compact_body(&self) -> Element<'_, Message> {
        let body = match self.sheet {
            Sheet::Invite => self.invite_sheet(),
            Sheet::Settings => self.settings_sheet(),
            // Every tab is the same two steps on a narrow window: its list, then the thing chosen
            // from it, with a way back at the top of the second.
            Sheet::None if self.screen == Screen::Tasks && !self.detail => self.task_browser(),
            Sheet::None if self.screen == Screen::Tasks => self.tasks_pane(),
            Sheet::None if self.screen == Screen::Chat && !self.detail => self.chat_browser(),
            Sheet::None if self.screen == Screen::Chat => self.chat_pane(),
            Sheet::None if self.graph_open && self.detail => self.graph_pane(),
            Sheet::None if !self.detail => self.note_browser(),
            Sheet::None => self.notes_pane(),
        };
        container(body)
            .padding([self.gutter(), self.gutter_x()])
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// The list at the top of a detail screen on a narrow window: the way back, named for the list,
    /// and what else goes with it.
    fn detail_header<'a>(
        back: &'static str,
        trailing: Vec<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let mut head = row![ghost_label(back, Message::BackToList), space().width(Fill)]
            .spacing(4)
            .align_y(iced::Alignment::Center);
        for item in trailing {
            head = head.push(item);
        }
        head.into()
    }

    /// The Tasks tab's list on a narrow window: the ways to look at the tasks, which the panel on
    /// the left is on a wide one.
    fn task_browser(&self) -> Element<'_, Message> {
        vscroll(self.task_sidebar()).height(Fill).width(Fill).into()
    }

    fn note_browser(&self) -> Element<'_, Message> {
        self.note_index()
    }

    fn chat_browser(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut side = column![action_fill("Invite", Message::ShowInvite)]
            .spacing(8)
            .width(Fill);
        if self.chats.is_empty() {
            side = side.push(text("No conversations yet.").size(13).color(ink.muted));
        }
        for chat in &self.chats {
            let selected = self.chat_id.as_deref() == Some(chat.id.as_str());
            let (preview, time) = self.chat_last.get(&chat.id).cloned().unwrap_or_default();
            side = side.push(chat_row(
                &self.chat_name(chat),
                &self.who_line(chat),
                &preview,
                &time,
                &chat.id,
                selected,
                self.chat_is_new(&chat.id),
            ));
        }
        side.into()
    }

    fn note_media(&self) -> Element<'_, Message> {
        let found = files::note_media(&self.dir, &self.body.text());
        if found.is_empty() {
            return space().height(0).into();
        }
        let mut row = row![].spacing(8);
        for item in found {
            match item {
                files::NoteMedia::Picture(path) => {
                    row = row.push(
                        iced_image::Image::new(iced_image::Handle::from_path(path))
                            .content_fit(iced::ContentFit::Contain)
                            .height(140)
                            .width(180),
                    );
                }
                files::NoteMedia::Video(path, name) => {
                    row = row.push(ghost_label_owned(
                        format!("Video · {name}"),
                        Message::OpenPath(path),
                    ));
                }
                files::NoteMedia::File(path, name) => {
                    row = row.push(ghost_label_owned(name, Message::OpenPath(path)));
                }
            }
        }
        hscroll(row).height(156).into()
    }

    fn chat_bubble(
        &self,
        line: &ChatBubble,
        width: f32,
        show_who: bool,
    ) -> Element<'static, Message> {
        let ink = self.ink();
        let copy_text = line.text.clone();
        let words = line.text.clone();
        let when = line.time.clone();
        let mine = line.mine;
        let conv = self.chat_id.clone().unwrap_or_default();
        let parts = files::pieces(&self.dir, &conv, &words);
        let mut body = column![].spacing(6);
        if parts.is_empty() {
            body = body.push(chat_words(words));
        }
        for part in parts {
            match part {
                files::Piece::Text(words) => {
                    body = body.push(chat_words(words));
                }
                files::Piece::Picture { name, path } => {
                    if let Some(path) = path {
                        body = body.push(
                            iced_image::Image::new(iced_image::Handle::from_path(path))
                                .content_fit(iced::ContentFit::Contain)
                                .width((width - 28.0).max(64.0))
                                .height(180),
                        );
                    } else {
                        body = body.push(
                            text(format!("{name} · not on this computer"))
                                .size(13)
                                .color(if mine { ink.on_accent } else { ink.text }),
                        );
                    }
                }
                files::Piece::Video { name, path } => {
                    body = body.push(file_chip(&name, path, true, mine));
                }
                files::Piece::File { name, path } => {
                    body = body.push(file_chip(&name, path, false, mine));
                }
            }
        }
        let time_color = if mine {
            Color {
                a: 0.72,
                ..ink.on_accent
            }
        } else {
            ink.muted
        };
        let mut meta = row![].spacing(8).align_y(iced::Alignment::Center);
        if !when.is_empty() {
            meta = meta.push(text(when).size(11).color(time_color));
        }
        meta = meta.push(
            button(text("Copy").size(11))
                .padding([0, 2])
                .on_press(Message::CopyChat(copy_text))
                .style(move |theme, status| copy_style(theme, status, mine)),
        );
        if let Some(cite) = line.cite.clone() {
            let label = self
                .catalog
                .iter()
                .find(|note| note.id == cite)
                .map(|note| note.title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "Note".to_owned());
            meta = meta.push(
                button(text(label).size(11))
                    .padding([0, 2])
                    .on_press(Message::OpenCited(cite))
                    .style(move |theme, status| copy_style(theme, status, mine)),
            );
        }
        body = body.push(meta);
        let bubble = container(body)
            .padding([8, 12])
            .max_width(width)
            .clip(true)
            .style(if mine { mine_style } else { theirs_style });
        let mut stack = column![].spacing(4);
        if show_who && !line.sender.is_empty() {
            stack = stack.push(speaker(&self.person_name(&line.sender), &line.sender));
        }
        stack = stack.push(bubble);
        container(stack)
            .width(Fill)
            .align_x(if mine {
                iced::alignment::Horizontal::Right
            } else {
                iced::alignment::Horizontal::Left
            })
            .into()
    }

    fn stage_file(&mut self, path: PathBuf) {
        if !path.is_file() {
            self.say("Choose a file, not a folder.");
            return;
        }
        if self.screen == Screen::Chat {
            if self.chat_id.is_none() {
                self.say("Invite someone before adding a file.");
                return;
            }
            self.pending_files.push(path);
            self.detail = true;
            self.sheet = Sheet::None;
            return;
        }
        match files::store_for_note(&self.dir, &path) {
            Ok(markdown) => {
                let insert = if self.body.text().is_empty() || self.body.text().ends_with('\n') {
                    markdown
                } else {
                    format!("\n{markdown}")
                };
                self.revise(Burst::Break, |app| {
                    let before = app.body.text();
                    app.body
                        .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                            Arc::new(insert),
                        )));
                    app.body.text() != before
                });
                self.screen = Screen::Notes;
                self.sheet = Sheet::None;
                self.detail = true;
                self.say("Added to the note on this computer.");
            }
            Err(err) => self.message = err,
        }
    }

    fn pick_file(&mut self) -> Task<Message> {
        if self.screen == Screen::Chat && self.chat_id.is_none() {
            self.say("Invite someone before adding a file.");
            return Task::none();
        }
        Task::perform(
            async {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(files::choose());
                });
                rx.recv()
                    .unwrap_or_else(|_| Err("No file chosen.".to_owned()))
            },
            Message::FilePicked,
        )
    }

    fn insert_mark(&mut self, kind: FormatKind) {
        let change = markdown::format_change(&self.body.text(), self.body.cursor(), kind);
        if kind == FormatKind::Wiki
            && let Some(inner) = change
                .insert
                .strip_prefix("[[")
                .and_then(|rest| rest.strip_suffix("]]"))
            && let Some((id, title)) = self.note_named(inner)
        {
            let change =
                markdown::note_link_change(&self.body.text(), &self.body.cursor(), &id, &title);
            self.apply_change(change);
            return;
        }
        self.apply_change(change);
    }

    fn apply_change(&mut self, change: markdown::Change) {
        self.revise(Burst::Break, |app| {
            let before = app.body.text();
            if change.anchor == change.end {
                app.body.place(text_editor::Cursor {
                    position: change.anchor,
                    selection: None,
                });
            } else {
                app.body.place(text_editor::Cursor {
                    position: change.end,
                    selection: Some(change.anchor),
                });
            }
            app.body
                .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                    Arc::new(change.insert),
                )));
            app.body.place(change.after);
            app.body.text() != before
        });
    }

    fn revise(&mut self, burst: Burst, change: impl FnOnce(&mut Self) -> bool) {
        let now = Instant::now();
        let gate = if self.history.coalesced(burst, now) {
            self.history.begin("", "", self.body.cursor(), burst, now)
        } else {
            let cursor = self.body.cursor();
            let title = self.title_text.clone();
            let body = self.body.text();
            self.history.begin(&title, &body, cursor, burst, now)
        };
        if change(self) {
            self.arm_pause();
        } else {
            self.history.abort(gate);
        }
    }

    fn snap(&self) -> EditSnap {
        EditSnap {
            title: self.title_text.clone(),
            body: self.body.text(),
            cursor: self.body.cursor(),
        }
    }

    fn restore_snap(&mut self, snap: EditSnap) {
        self.title_text = snap.title;
        self.body.load(&snap.body);
        self.body.place(snap.cursor);
        self.drag_from = None;
        self.last_press = None;
        if self.editor_dirty() {
            self.arm_pause();
        } else {
            self.disarm_pause();
        }
    }

    fn undo_note(&mut self) {
        let Some(previous) = self.history.undo(self.snap()) else {
            return;
        };
        self.restore_snap(previous);
    }

    fn redo_note(&mut self) {
        let Some(next) = self.history.redo(self.snap()) else {
            return;
        };
        self.restore_snap(next);
    }

    fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Keeps the steps with the note that just left. Memory only, and not the log.
    fn swap_history(&mut self, next: &str) {
        if self.history_key == next {
            return;
        }
        let leaving = std::mem::take(&mut self.history);
        let key = self.history_key.clone();
        if leaving.undo.is_empty() && leaving.redo.is_empty() {
            self.histories.remove(&key);
            self.history_order.retain(|id| id != &key);
        } else {
            self.histories.insert(key.clone(), leaving);
            self.history_order.retain(|id| id != &key);
            self.history_order.push(key);
        }
        while self.history_order.len() > 8 {
            let oldest = self.history_order.remove(0);
            self.histories.remove(&oldest);
        }
        self.history = self.histories.remove(next).unwrap_or_default();
        self.history_order.retain(|id| id != next);
        next.clone_into(&mut self.history_key);
    }

    fn try_open_at(&mut self, byte: usize) -> bool {
        let Some(link) = markdown::opened_at(&self.body.text(), byte) else {
            return false;
        };
        match link {
            markdown::OpenLink::Url(url) => {
                if let Err(err) = files::open_url(&url) {
                    self.message = err;
                }
            }
            markdown::OpenLink::Note(target) => {
                self.open_note_link(&target.name, target.version.as_deref());
            }
        }
        true
    }

    fn find_bar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let hits = markdown::find_bytes(&self.body.text(), &self.find_query);
        let place = if self.find_query.trim().is_empty() {
            String::new()
        } else if hits.is_empty() {
            "None".to_owned()
        } else if self.find_at == usize::MAX || self.find_at >= hits.len() {
            format!("{} found", hits.len())
        } else {
            format!("{} of {}", self.find_at.saturating_add(1), hits.len())
        };
        let mut bar = row![
            text_input("Find in this note", &self.find_query)
                .on_input(Message::FindQuery)
                .on_submit(Message::FindNext)
                .padding(8)
                .width(Fill)
                .style(field_style),
            ghost_label("Previous", Message::FindPrev),
            ghost_label("Next", Message::FindNext),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if !place.is_empty() {
            bar = bar.push(text(place).size(13).color(ink.muted));
        }
        bar.into()
    }

    fn seek_find(&mut self, forward: bool) {
        let query = self.find_query.trim().to_owned();
        if query.is_empty() {
            self.say("Type what to find.");
            return;
        }
        let hits = markdown::find_bytes(&self.body.text(), &query);
        if hits.is_empty() {
            self.find_at = usize::MAX;
            self.say("Not in this note.");
            return;
        }
        let next = if self.find_at == usize::MAX || self.find_at >= hits.len() {
            if forward {
                0
            } else {
                hits.len().saturating_sub(1)
            }
        } else if forward {
            self.find_at.saturating_add(1) % hits.len()
        } else if self.find_at == 0 {
            hits.len().saturating_sub(1)
        } else {
            self.find_at.saturating_sub(1)
        };
        self.find_at = next;
        let Some(&start) = hits.get(next) else {
            self.find_at = usize::MAX;
            return;
        };
        let end = start.saturating_add(query.len());
        self.body.select_between(start, end);
        self.body.end_select();
        self.message.clear();
    }

    fn note_reveal(&mut self) -> Task<Message> {
        let y = (self.body.reveal(self.writing_width()) - 72.0).max(0.0);
        Task::batch([
            iced::widget::operation::focus(iced::widget::Id::new("note-body")),
            iced::widget::operation::scroll_to(
                iced::widget::Id::new("note-scroll"),
                iced::widget::operation::AbsoluteOffset { x: 0.0, y },
            ),
        ])
    }

    fn arm_press(&mut self, byte: usize) {
        self.pointer_down = true;
        self.kept_click = false;
        let text = self.body.text();
        let byte = markdown::snap_caret(&text, byte);
        if self.shift_down {
            let anchor = self
                .body
                .anchor_byte()
                .unwrap_or_else(|| self.body.caret_byte());
            self.drag_from = Some(anchor);
            self.kept_click = true;
            self.body.select_between(anchor, byte);
            self.last_press = None;
            return;
        }
        let repeat = self.last_press.is_some_and(|(when, previous, _)| {
            when.elapsed() < Duration::from_millis(400)
                && markdown::same_line(&text, previous, byte)
        });
        let count = if repeat {
            self.last_press
                .map_or(1, |(_, _, count)| count.saturating_add(1))
        } else {
            1
        };
        if count >= 3 {
            let (from, to) = markdown::line_bounds(&text, byte);
            self.body.select_between(from, to);
            self.drag_from = Some(from);
            self.kept_click = true;
            self.last_press = None;
            return;
        }
        if count == 2
            && self
                .last_press
                .is_some_and(|(_, previous, _)| markdown::same_word(&text, previous, byte))
        {
            let (from, to) = markdown::word_bounds(&text, byte);
            self.body.select_between(from, to);
            self.drag_from = Some(from);
            self.kept_click = true;
            self.last_press = Some((Instant::now(), byte, 2));
            return;
        }
        self.last_press = Some((Instant::now(), byte, 1));
        self.drag_from = Some(byte);
        if markdown::line_index(&text, byte) == self.body.open_line() {
            self.body.place_byte(byte);
        } else {
            // Opening that line now would reflow the page under the pointer.
            self.body.place_byte_soft(byte);
        }
    }

    fn note_column(&self) -> Element<'_, Message> {
        if let Some(older) = &self.older {
            return self.older_page(older);
        }
        let chapters = &self.cached_chapters;
        let mut page = column![].spacing(14).width(Fill).height(Fill);
        if self.reading && !chapters.is_empty() {
            page = page.push(
                row![
                    text(self.read_title.clone())
                        .font(serif())
                        .size(if self.compact() { 32.0 } else { 36.0 })
                        .width(Fill),
                    ghost_label("Back to writing", Message::ToggleRead),
                ]
                .align_y(iced::Alignment::Center),
            );
            page = page.push(self.read_pages(chapters));
        } else {
            let focused = self.focused();
            if focused {
                // The title is small, at the top right of the note, so the writing starts a
                // little below it. This keeps a line from running under it.
                page = page.push(space().height(FOCUS_TOP));
            } else {
                page = page.push(
                    text_input("Title", &self.title_text)
                        .id(iced::widget::Id::new("note-title"))
                        .on_input(Message::TitleChanged)
                        .on_submit(Message::TitleSubmit)
                        .font(serif())
                        .size(if self.compact() { 32.0 } else { 36.0 })
                        .padding([2.0, 0.0])
                        .style(title_style),
                );
                page = page.push(self.tag_chips());
                if let Some(line) = self.link_counts() {
                    page = page.push(line);
                }
            }
            if self.find_open {
                page = page.push(self.find_bar());
            }
            if !self.compact() {
                page = page.push(self.format_slot());
            }
            if let Some((_, query)) = markdown::open_wiki(&self.body.text(), self.body.caret_byte())
            {
                page = page.push(self.link_menu(&query));
            }
            if !self.proof_warning.is_empty() && !focused {
                page = page.push(
                    text(self.proof_warning.clone())
                        .size(12)
                        .color(self.ink().muted),
                );
            }
            page = page.push(
                vscroll_flush(self.editor_page())
                    .id(iced::widget::Id::new("note-scroll"))
                    .height(Fill),
            );
            page = page.push(self.note_media());
        }
        self.writing_frame(page)
    }

    fn writing_frame<'a>(&self, page: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
        container(page.max_width(self.writing_column()).clip(true))
            .width(Fill)
            .height(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }

    fn editor_page(&self) -> Element<'_, Message> {
        markdown::set_known_links(self.known_links.clone());
        markdown::set_misspellings(Arc::clone(&self.misspellings));
        let dragging = self.pointer_down && self.drag_from.is_some();
        let page = markdown::view(&self.body, self.is_dark(), dragging, self.writing_width());
        let Some((y, height)) = (!dragging)
            .then(|| self.body.selection_origin(self.writing_width()))
            .flatten()
        else {
            return page;
        };
        let lift = if y >= 38.0 {
            y - 34.0
        } else {
            y + height + 4.0
        };
        stack![
            page,
            container(Self::format_pill()).padding(iced::Padding {
                top: lift,
                right: 8.0,
                bottom: 0.0,
                left: 0.0,
            })
        ]
        .into()
    }

    fn format_pill() -> Element<'static, Message> {
        container(
            row![
                pill_button("B", "Bold", Message::Format(FormatKind::Bold)),
                pill_button("I", "Italic", Message::Format(FormatKind::Italic)),
                pill_button("S", "Strike", Message::Format(FormatKind::Strike)),
                pill_button("==", "Highlight", Message::Format(FormatKind::Highlight)),
                pill_button("Link", "Link", Message::Format(FormatKind::Link)),
                pill_button("Copy", "Copy", Message::CopyNote),
                pill_button("Cut", "Cut", Message::CutNote),
                pill_button("Paste", "Paste", Message::PasteNote),
                pill_button(
                    "Send",
                    "Send this selection to a chat",
                    Message::SendSelection
                ),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 6])
        .style(pill_style)
        .into()
    }

    fn context_room(&self) -> bool {
        self.sheet == Sheet::None && self.screen == Screen::Notes
    }

    fn context_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut block = column![self.panel_header()]
            .spacing(14)
            .padding(iced::Padding {
                top: 18.0,
                right: 20.0,
                bottom: 18.0,
                left: 20.0,
            })
            .width(Fill)
            .height(Fill);
        if self.note_panel {
            return container(self.note_settings(block))
                .width(CONTEXT_WIDTH)
                .height(Fill)
                .style(surface_pane)
                .into();
        }
        if self.cached_backlinks.is_empty() && self.links_to.is_empty() {
            block = block.push(
                text("Notes this one links to, and notes that link here, gather in this panel. Link with [[. Tags under the title make groups: Novel/Part 1 is a group inside Novel.")
                    .size(13)
                    .color(ink.muted)
                    .line_height(1.4),
            );
        }
        if !self.links_to.is_empty() {
            block = block.push(self.link_section("Links to", &self.links_to));
        }
        if !self.cached_backlinks.is_empty() {
            block = block.push(self.link_section("Linked from", &self.cached_backlinks));
        }
        container(block)
            .width(CONTEXT_WIDTH)
            .height(Fill)
            .style(surface_pane)
            .into()
    }

    /// The panel's title, large, with the switch for settings or links at its right, and a line
    /// under both.
    fn panel_header(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut head = row![
            text(if self.note_panel {
                "This note"
            } else {
                "Links"
            })
            .font(serif())
            .size(22)
            .color(ink.text),
            space().width(Fill),
            panel_icon(
                mini_handle(Mini::Gear, self.is_dark()),
                if self.note_panel {
                    "Links and backlinks"
                } else {
                    "Note settings"
                },
                Message::ToggleNoteSettings,
                self.note_panel,
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        if !self.side_by_side_context() {
            // As a sheet over the page it has a way to go, as well as a press on the page.
            head = head.push(ghost_label("Close", Message::ToggleContext));
        }
        column![head, hairline(),].spacing(12).into()
    }

    /// A small heading with a count, and one button per note under it.
    fn link_section<'a>(
        &'a self,
        heading: &'a str,
        notes: &'a [(String, String)],
    ) -> Element<'a, Message> {
        let ink = self.ink();
        let mut list = column![
            row![
                text(heading.to_uppercase())
                    .size(11)
                    .color(ink.muted)
                    .width(Fill),
                text(notes.len().to_string()).size(11).color(ink.muted),
            ]
            .padding([0, 8])
        ]
        .spacing(2)
        .width(Fill);
        for (id, title) in notes {
            list = list.push(
                button(text(title.clone()).size(14).width(Fill))
                    .width(Fill)
                    .padding([7, 10])
                    .on_press(Message::Select(id.clone()))
                    .style(list_row(false)),
            );
        }
        list.into()
    }

    /// What belongs to this one note and not to the page: where it was written, and what can be
    /// done with it as a whole.
    fn note_settings<'a>(
        &'a self,
        mut block: iced::widget::Column<'a, Message>,
    ) -> iced::widget::Column<'a, Message> {
        let ink = self.ink();
        let saved = self.selected.is_some();
        let home = if !saved {
            "Not saved yet.".to_owned()
        } else if self.note_writer.is_empty() {
            "This computer.".to_owned()
        } else {
            self.note_writer.clone()
        };
        block = block.push(
            column![
                text("HOME DEVICE").size(11).color(ink.muted),
                text(home).size(15),
            ]
            .spacing(4)
            .padding([0, 8]),
        );
        let has_words = saved || !self.body.text().is_empty() || !self.title_text.is_empty();
        if has_words || saved || self.editor_dirty() {
            block = block.push(hairline());
        }
        if has_words {
            block = block.push(action_fill("Send to a chat", Message::SendNoteToChat));
        }
        if saved || self.editor_dirty() {
            block = block.push(ghost_label(
                if saved { "Delete note" } else { "Discard" },
                Message::AskDelete,
            ));
        }
        block
    }

    fn edge_toggle(&self, list: bool) -> Element<'_, Message> {
        let open = if list {
            self.show_list
        } else {
            self.show_context
        };
        let mark = match (list, open) {
            (true, true) | (false, false) => "‹",
            _ => "›",
        };
        let hint = match (list, open) {
            (true, true) => "Hide the list",
            (true, false) => "Show the list",
            (false, true) => "Hide the group",
            (false, false) => "Show the group",
        };
        let message = if list {
            Message::ToggleList
        } else {
            Message::ToggleContext
        };
        let face = button(
            container(text(mark).size(18))
                .width(Fill)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .width(28)
        .height(Fill)
        .padding(0)
        .on_press(message)
        .style(icon_style);
        hover_label(face, hint)
    }

    fn read_pages(&self, chapters: &[Chapter]) -> Element<'_, Message> {
        markdown::set_known_links(self.known_links.clone());
        markdown::set_misspellings(Arc::clone(&self.misspellings));
        let ink = self.ink();
        let mut pages = column![].spacing(0).width(Fill);
        for (index, chapter) in chapters.iter().enumerate() {
            if index > 0 {
                pages = pages.push(space().height(36));
                pages = pages.push(hairline());
                pages = pages.push(space().height(36));
            }
            let mut part = column![text(chapter.name.clone()).font(serif()).size(24)]
                .spacing(16)
                .width(Fill);
            if chapter.body.trim().is_empty() {
                part = part.push(text("Not written yet.").size(14).color(ink.muted));
            } else {
                part = part.push(markdown::reading(&chapter.body, self.is_dark()));
            }
            pages = pages.push(part);
        }
        vscroll(pages).height(Fill).into()
    }

    fn find_title(&self, name: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }
        if self.title_text.eq_ignore_ascii_case(name)
            && let Some(id) = &self.selected
        {
            return Some(id.clone());
        }
        for (key, title, _) in &self.unsaved {
            if title.eq_ignore_ascii_case(name)
                && let Some(id) = key
            {
                return Some(id.clone());
            }
        }
        self.catalog
            .iter()
            .find(|note| !note.title.is_empty() && note.title.eq_ignore_ascii_case(name))
            .map(|note| note.id.clone())
    }

    /// Read the notes of the group in view, one after another, in title order. The notes are
    /// not changed and no combined note is written.
    /// The mouse button came up. If a note was being dragged and the pointer is over a note in
    /// its group, or over the group's name, it goes there.
    fn finish_list_drag(&mut self) {
        let Some(drag) = self.list_drag.take() else {
            return;
        };
        let Some(over) = drag.over else {
            return;
        };
        let mut members = self.exact_members(&drag.group);
        let ids: Vec<String> = members.iter().map(|(id, _)| id.clone()).collect();
        let target = if over == top_of(&drag.group) {
            None
        } else if ids.contains(&over) {
            Some(over)
        } else {
            return;
        };
        let moved = move_in_order(&ids, &drag.id, target.as_deref());
        if moved == ids {
            return;
        }
        members.clear();
        self.orders.insert(drag.group, moved);
        if let Err(err) = profile::store_order(&self.dir, &self.orders) {
            self.say(&err);
        }
    }

    /// The notes tagged with exactly this group, as id and title, in the order they are shown.
    fn exact_members(&self, group: &str) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = self
            .catalog
            .iter()
            .filter(|note| !is_tasks_note(note) && has_exact_tag(&note.tags, group))
            .map(|note| (note.id.clone(), note.title.clone()))
            .collect();
        order_ids(
            &mut found,
            self.orders.get(group).map_or(&[][..], Vec::as_slice),
        );
        found
    }

    fn toggle_read(&mut self) {
        if self.reading {
            self.reading = false;
            return;
        }
        let Some(group) = self.tag.clone() else {
            return;
        };
        let mut members: Vec<(String, String)> = self
            .catalog
            .iter()
            .filter(|note| !is_tasks_note(note) && in_group(&note.tags, &group))
            .map(|note| (note.id.clone(), note.title.clone()))
            .collect();
        order_ids(
            &mut members,
            self.orders
                .get(&group_key(&tag_path(&group)))
                .map_or(&[][..], Vec::as_slice),
        );
        let chapters: Vec<Chapter> = members
            .into_iter()
            .map(|(id, title)| {
                let body = self
                    .task_source(&id)
                    .map(|(body, _, _)| ambient_cli::split_note_tags(&body).0)
                    .unwrap_or_default();
                Chapter {
                    name: if title.is_empty() {
                        "Untitled".to_owned()
                    } else {
                        title
                    },
                    body,
                }
            })
            .collect();
        if chapters.is_empty() {
            return;
        }
        self.read_title = tag_path(&group)
            .last()
            .map_or_else(String::new, |last| (*last).to_owned());
        self.cached_chapters = chapters;
        self.graph_open = false;
        self.graph = None;
        self.reading = true;
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
    }

    fn open_chapter(&mut self, name: &str) -> Task<Message> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Task::none();
        }
        if markdown::is_note_id(&name) {
            if self.catalog.iter().any(|note| note.id == name) {
                self.reading = false;
                self.show_note(&name);
            } else {
                self.say("That note is gone.");
            }
            return Task::none();
        }
        if let Some(id) = self.find_title(&name) {
            self.reading = false;
            self.show_note(&id);
            return Task::none();
        }
        if self.saving {
            self.pending_chapter = Some(name);
            return Task::none();
        }
        self.saving = true;
        let dir = self.dir.clone();
        let current = if self.editor_dirty() {
            Some((
                self.selected.clone(),
                self.title_text.clone(),
                self.stored_body(),
            ))
        } else {
            None
        };
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = write_chapter(&dir, current.as_ref(), &name);
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Could not add that chapter.".to_owned()))
            },
            Message::ChapterMade,
        )
    }

    fn finish_chapter(&mut self, result: Result<MadeChapter, String>) -> Task<Message> {
        self.saving = false;
        let made = match result {
            Ok(made) => made,
            Err(err) => {
                self.message = err;
                return self.take_pending_chapter();
            }
        };
        if let Some(saved) = made.saved {
            if self.title_text == saved.title && self.stored_body() == saved.body {
                self.disk_title = saved.title;
                self.disk_body = saved.body;
                self.disarm_pause();
            }
            if self.selected.is_none() {
                if self.history_key.is_empty() {
                    self.history_key.clone_from(&saved.id);
                }
                self.selected = Some(saved.id.clone());
            }
        }
        if self.install_latest() {
            self.post_side();
        } else {
            self.take_notes(made.notes);
        }
        self.reading = false;
        self.show_note(&made.id);
        self.take_pending_chapter()
    }

    fn take_pending_chapter(&mut self) -> Task<Message> {
        let Some(name) = self.pending_chapter.take() else {
            return Task::none();
        };
        self.open_chapter(&name)
    }

    fn note_follow(&mut self, scroll: bool) -> Task<Message> {
        let jump = if scroll {
            self.body.follow(self.writing_width())
        } else {
            None
        };
        let focus = iced::widget::operation::focus(iced::widget::Id::new("note-body"));
        if let Some(top) = jump {
            let y = (top - 72.0).max(0.0);
            return Task::batch([
                focus,
                iced::widget::operation::scroll_to(
                    iced::widget::Id::new("note-scroll"),
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y },
                ),
            ]);
        }
        focus
    }

    /// Copy the notebook folder to the place typed in settings.
    ///
    /// A note that was typed in the last moment is saved first, so it is in the copy. The copy
    /// runs on its own thread and holds the notes lock and the chat lock, so no save or sync lands
    /// in the middle of it.
    fn start_backup(&mut self) -> Task<Message> {
        if self.backup_busy {
            return Task::none();
        }
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        if self.saving || self.editor_dirty() {
            self.say("Saving your note first. Press Back up again in a moment.");
            return self.start_save(true);
        }
        let typed = self.backup_to.trim();
        let dest = if typed.is_empty() {
            profile::default_backup_dir()
        } else {
            profile::expand_home(typed)
        };
        self.backup_to = dest.display().to_string();
        self.backup_busy = true;
        self.say("Backing up.");
        let dir = self.dir.clone();
        let name = format!(
            "aerodash-{}",
            clock::stamp(clock::now_ms(), &clock::load_zone().0)
        );
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = {
                        let _notes = notes_lock();
                        let _chat = chat_lock();
                        profile::backup_folder(&dir, &dest, &name)
                    };
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("The copy stopped before it finished.".to_owned()))
            },
            Message::BackedUp,
        )
    }

    fn finish_backup(&mut self, result: Result<profile::BackupReport, String>) {
        self.backup_busy = false;
        match result {
            Ok(report) => {
                self.backup_at = clock::now_ms();
                self.persist_profile();
                self.say(&format!(
                    "Backed up {} files, {}, to {}.",
                    report.files,
                    size_label(report.bytes),
                    report.path.display()
                ));
            }
            Err(err) => self.message = err,
        }
    }

    /// Type a key that the editor ignored. Only while the writing page is the thing on screen and
    /// nothing else, such as a popup, wants keys. A field that has focus takes its own keys, so
    /// those never get here.
    fn type_ignored(&mut self, typed: &Typed) -> Task<Message> {
        let writing = self.notebook.is_some()
            && self.welcome.is_none()
            && !self.breeze_on()
            && self.editing_note()
            && !self.reading
            && self.older.is_none()
            && !self.confirm_delete
            && !self.send_open
            && self.conflicts.is_empty();
        if !writing {
            return Task::none();
        }
        let edit = |edit: text_editor::Edit| Message::BodyEdited(text_editor::Action::Edit(edit));
        let messages: Vec<Message> = match typed {
            Typed::Text(text) => text
                .chars()
                .map(|ch| edit(text_editor::Edit::Insert(ch)))
                .collect(),
            Typed::Enter => vec![edit(text_editor::Edit::Enter)],
            Typed::Backspace => vec![edit(text_editor::Edit::Backspace)],
            Typed::Delete => vec![edit(text_editor::Edit::Delete)],
        };
        Task::batch(messages.into_iter().map(|message| self.update(message)))
    }

    fn share_words(&self) {
        let words: Vec<String> = self.my_words.iter().cloned().collect();
        markdown::set_personal_words(&words);
    }

    /// Add the word at the caret to the dictionary, and check the page again so its underline goes.
    fn add_word(&mut self) {
        if !self.editing_note() {
            return;
        }
        let Some(word) = markdown::word_at(&self.body.text(), self.body.caret_byte()) else {
            self.say("Put the cursor in a word first.");
            return;
        };
        if !self.my_words.insert(word.clone()) {
            self.say("That word is already in your dictionary.");
            return;
        }
        let words: Vec<String> = self.my_words.iter().cloned().collect();
        if let Err(err) = profile::store_words(&self.dir, &words) {
            self.message = err;
            return;
        }
        self.share_words();
        self.proof_body.clear();
        self.maybe_proof();
        self.say(&format!("Added {word} to your dictionary."));
    }

    fn say(&mut self, text: &str) {
        text.clone_into(&mut self.message);
    }

    fn start_relay(&mut self) {
        match ambient_cli::start_loopback_relay() {
            Ok(()) => {
                self.relay_ready = true;
                self.relay_ours = true;
                "Relay is listening on 127.0.0.1:8787.".clone_into(&mut self.sync_detail);
            }
            Err(err) => {
                if err.to_string().contains("already running") {
                    self.relay_ready = true;
                    self.relay_ours = false;
                    self.sync_detail = err.to_string();
                } else {
                    self.relay_ready = false;
                    self.relay_ours = false;
                    self.sync_detail = err.to_string();
                }
            }
        }
    }

    fn stop_relay(&mut self) {
        match ambient_cli::stop_loopback_relay() {
            Ok(()) => {
                self.relay_ready = false;
                self.relay_ours = false;
                self.net_up = false;
                "Relay stopped.".clone_into(&mut self.sync_detail);
            }
            Err(err) => self.sync_detail = err.to_string(),
        }
    }

    fn use_this_computer(&mut self) {
        let choice = ambient_cli::net_setting(&self.dir)
            .map(|setting| setting.choice)
            .unwrap_or_default();
        let setting = ambient_cli::NetSetting {
            mode: ambient_cli::NetMode::ThisComputer,
            pin: self.pin_draft.clone(),
            backup: self.backup_draft.clone(),
            choice,
        };
        if let Err(err) = ambient_cli::set_net_setting(&self.dir, &setting) {
            self.sync_detail = err.to_string();
            return;
        }
        self.net_mode = ambient_cli::NetMode::ThisComputer;
        self.net_up = false;
        self.start_relay();
    }

    fn save_pinned(&mut self) {
        let choice = ambient_cli::net_setting(&self.dir)
            .map(|setting| setting.choice)
            .unwrap_or_default();
        let setting = ambient_cli::NetSetting {
            mode: ambient_cli::NetMode::Pin,
            pin: self.pin_draft.trim().to_owned(),
            backup: self.backup_draft.trim().to_owned(),
            choice,
        };
        match ambient_cli::set_net_setting(&self.dir, &setting) {
            Ok(()) => {
                self.net_mode = ambient_cli::NetMode::Pin;
                self.net_up = false;
                "Pinned relay saved.".clone_into(&mut self.sync_detail);
            }
            Err(err) => self.sync_detail = err.to_string(),
        }
    }

    fn mark_pending(&mut self) {
        self.pending = true;
        self.writes = self.writes.saturating_add(1);
    }

    fn open_paired(&mut self) {
        if !self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            self.say("This window is the paired device. It does not hold the root.");
            return;
        }
        let path = sibling(&self.dir, "-device");
        if path.join("key").exists() {
            self.spawn_at(
                &path,
                "Opened the paired window. Close a duplicate if one was already open.",
            );
            return;
        }
        let offer = match ambient_cli::device_offer(&path, "other-computer") {
            Ok(offer) => offer,
            Err(err) => {
                self.message = err.to_string();
                return;
            }
        };
        match self
            .notebook
            .as_ref()
            .map(|book| book.pair_device(&offer, "other-computer"))
        {
            Some(Err(err)) => {
                let _ = std::fs::remove_dir_all(&path);
                self.message = err.to_string();
                return;
            }
            None => return,
            Some(Ok(())) => {}
        }
        self.spawn_at(
            &path,
            "Paired. That window does not hold the root. Both windows sync while the relay is on.",
        );
    }

    fn open_person(&mut self) {
        if !self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            self.say("Open the other person from the window that holds the root.");
            return;
        }
        let path = sibling(&self.dir, "-person");
        if !path.join("key").exists()
            && let Err(err) = Notebook::create(&path)
        {
            self.message = err.to_string();
            return;
        }
        self.spawn_at(
            &path,
            "Opened the other person. In that window open Invite and copy the link. Paste it here. Only this window presses Start conversation.",
        );
    }

    fn open_one_more(&mut self) {
        if !self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            self.say("Open another person from the window that holds the root.");
            return;
        }
        for n in 2..=8 {
            let path = sibling(&self.dir, &format!("-person-{n}"));
            if path.join("key").exists() {
                continue;
            }
            if let Err(err) = Notebook::create(&path) {
                self.message = err.to_string();
                return;
            }
            self.spawn_at(
                &path,
                "Opened one more person. In that window open Invite and copy the link. Paste it in the conversation and press Add someone. They wait. They do not start a conversation.",
            );
            return;
        }
        self.say("Eight extra people are already on this computer.");
    }

    fn add_to_chat(&mut self) {
        let Some(id) = self.chat_id.clone() else {
            self.say("Open a conversation first.");
            return;
        };
        let card = self.add_draft.trim().to_owned();
        if card.is_empty() {
            self.say("Paste their link first. In their window open Invite and copy the link.");
            return;
        }
        let before = self
            .chats
            .iter()
            .find(|chat| chat.id == id)
            .map(|chat| chat.others.clone())
            .unwrap_or_default();
        let call = self.add_call.trim().to_owned();
        match ambient_cli::add_member_in(&self.dir, &id, &card) {
            Ok(()) => {
                if !call.is_empty() {
                    self.name_new_people(&id, &call, &before);
                }
                self.accept_offer(&card);
                self.add_draft.clear();
                self.add_call.clear();
                self.adding = false;
                self.say(
                    "Added. They show up once both windows sync. They do not press Start conversation.",
                );
                self.sync_watch();
                self.post(lane::Job::Inbox);
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn copy_card(&mut self) -> Task<Message> {
        match ambient_cli::contact_card(&self.dir) {
            Ok(card) => {
                self.my_card.clone_from(&card);
                self.say("Card copied. The other window wants the invite link, not this card.");
                clipboard::write(card)
            }
            Err(err) => {
                self.message = err.to_string();
                Task::none()
            }
        }
    }

    fn start_chat(&mut self) {
        let card = self.their_card.trim().to_owned();
        if card.is_empty() {
            self.say("Paste their link first. In their window open Invite and copy the link.");
            return;
        }
        match ambient_cli::start_conversation(&self.dir, &card) {
            Ok(id) => {
                let call = self.call_draft.trim().to_owned();
                if !call.is_empty() {
                    self.aliases.insert(id.clone(), call.clone());
                    self.name_new_people(&id, &call, &[]);
                    self.persist_names();
                }
                self.accept_offer(&card);
                self.call_draft.clear();
                self.chat_id = Some(id);
                self.screen = Screen::Chat;
                self.sheet = Sheet::None;
                self.their_card.clear();
                self.say("Conversation saved here. Both windows sync while the relay is on.");
                self.sync_watch();
                self.post(lane::Job::Inbox);
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn send_chat(&mut self) {
        let text = self.draft_editor.text().trim().to_owned();
        let Some(id) = self.chat_id.clone() else {
            self.say("Invite someone before sending.");
            return;
        };
        let files = std::mem::take(&mut self.pending_files);
        if text.is_empty() && files.is_empty() {
            self.say("Write a message, or add a file.");
            return;
        }
        self.draft_editor = text_editor::Content::new();
        self.screen = Screen::Chat;
        self.detail = true;
        self.sync_watch();
        let cite = self.chat_cite.take().map(|(id, _)| id);
        self.cite_open = false;
        self.post(lane::Job::Send {
            id,
            text,
            files,
            cite,
        });
    }

    fn spawn_at(&mut self, path: &Path, ok_message: &str) {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(err) => {
                self.message = format!("could not open another window: {err}");
                return;
            }
        };
        match std::process::Command::new(exe).arg(path).spawn() {
            Ok(_) => ok_message.clone_into(&mut self.message),
            Err(err) => self.message = format!("could not open another window: {err}"),
        }
    }

    fn show_phrase(&mut self) {
        match ambient_cli::recovery_phrase(&self.dir) {
            Ok(phrase) => {
                self.recovery_phrase = phrase;
                if self.welcome.is_none() {
                    self.say("Recovery phrase shown. Copy it and store it offline.");
                }
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn copy_phrase(&mut self) -> Task<Message> {
        if self.recovery_phrase.is_empty() {
            self.say("Show the phrase first.");
            return Task::none();
        }
        self.say("Phrase copied.");
        clipboard::write(self.recovery_phrase.clone())
    }

    fn start_restore(&mut self) {
        let phrase = self.restore_phrase.trim();
        if phrase.is_empty() {
            self.say("Paste the recovery phrase first.");
            return;
        }
        let path = sibling(&self.dir, "-restored");
        if path.join("key").exists() {
            self.say("Restored folder already exists. Open it separately or delete it first.");
            return;
        }
        match ambient_cli::restore_folder(&path, phrase, ambient_cli::LOOPBACK_URL) {
            Ok(()) => {
                self.spawn_at(
                    &path,
                    "Restored. That window holds the root and enrolled this computer. Old devices stay until you revoke them.",
                );
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn copy_link(&mut self) -> Task<Message> {
        self.refresh_link();
        if self.contact_link.is_empty() {
            self.say("No invite link yet.");
            return Task::none();
        }
        self.say("Invite link copied. It carries the name on your card.");
        clipboard::write(self.contact_link.clone())
    }

    fn toggle_qr(&mut self) {
        self.show_qr = !self.show_qr;
        if self.show_qr && self.qr_bytes.is_none() {
            match generate_qr(&self.contact_link) {
                Ok(bytes) => self.qr_bytes = Some(bytes),
                Err(err) => {
                    self.message = err;
                    self.show_qr = false;
                }
            }
        }
    }

    fn revoke(&mut self, id: &str) {
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.revoke_device(id) {
            Ok(()) => {
                self.devices = book.devices().unwrap_or_default();
                self.say("Revoked. New events are no longer sealed to that device. Notes it already wrote stay.");
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn send_selection(&mut self) {
        let Some((from, to)) = self.body.selection_bounds() else {
            self.say("Select some words first.");
            return;
        };
        let text = self.body.text();
        let Some(words) = text.get(from..to) else {
            return;
        };
        let line_of = |at: usize| {
            text.get(..at)
                .map_or(0, |head| head.matches('\n').count())
                .saturating_add(1)
        };
        let part = (words.to_owned(), line_of(from), line_of(to));
        self.send_note_to_chat();
        if self.send_open {
            self.send_part = Some(part);
        }
    }

    fn send_note_to_chat(&mut self) {
        self.send_part = None;
        if self.title_text.trim().is_empty() && self.body.text().trim().is_empty() {
            self.say("Write a note first.");
            return;
        }
        if self.chats.is_empty() {
            self.say("Start a conversation before sending a note.");
            return;
        }
        if self
            .send_choice
            .as_ref()
            .is_none_or(|id| !self.chats.iter().any(|chat| &chat.id == id))
        {
            self.send_choice = self
                .chat_id
                .clone()
                .filter(|id| self.chats.iter().any(|chat| &chat.id == id))
                .or_else(|| self.chats.first().map(|chat| chat.id.clone()));
        }
        self.send_open = true;
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
    }

    fn confirm_send(&mut self) {
        let Some(id) = self.send_choice.clone() else {
            self.say("Choose a chat.");
            return;
        };
        if self.title_text.trim().is_empty() && self.body.text().trim().is_empty() {
            self.say("Write a note first.");
            return;
        }
        let mut text = String::new();
        if let Some((words, first, last)) = self.send_part.take() {
            let home = if self.selected.is_some() {
                self.note_writer.as_str()
            } else {
                ""
            };
            text = selection_message(&self.title_text, (first, last), home, &words);
        } else {
            if !self.title_text.is_empty() {
                text.push_str(&self.title_text);
                text.push_str("\n\n");
            }
            text.push_str(&self.body.text());
        }
        let label = self
            .chats
            .iter()
            .find(|chat| chat.id == id)
            .map_or_else(|| "that chat".to_owned(), |chat| self.chat_name(chat));
        self.send_open = false;
        self.chat_id = Some(id.clone());
        self.say(&format!(
            "Sent to {label}. The words sync while a relay is on."
        ));
        self.mark_pending();
        self.post(lane::Job::Send {
            id,
            text,
            files: Vec::new(),
            cite: self.selected.clone(),
        });
    }

    fn add_loose_task(&mut self) -> Task<Message> {
        let mut label = self.task_draft.trim().replace(['\n', '\r'], " ");
        if label.is_empty() {
            self.say("Write the task first.");
            return Task::none();
        }
        // The date goes last among the words, ahead of a cite, which is where a due day is read.
        label = due::Due::write(&label, self.new_due.due().as_ref());
        if let Some((id, title)) = self.task_cite.clone() {
            let title = title.replace(['|', '[', ']'], "");
            label = format!("{label} [[{id}|{title}]]");
        }
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        let line = format!("- [ ] {label}");
        let existing = self
            .catalog
            .iter()
            .find(|note| note.title == "Tasks" && !note.conflict)
            .map(|note| note.id.clone());
        let Some(id) = existing else {
            self.task_draft.clear();
            self.new_due.clear();
            self.task_cite = None;
            self.task_cite_open = false;
            self.say("Added to Tasks.");
            return self.apply_note(None, "Tasks".to_owned(), format!("{line}\n"));
        };
        let Some((body, conflict, title)) = self.task_source(&id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(&id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let mut next = body;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(&line);
        next.push('\n');
        // The last task left "Added to Tasks." in the message. Clear it, so that a message after
        // the write means the write said something is wrong, and not that it said it last time.
        self.message.clear();
        let follow = self.write_note(&id, &title, &next);
        if self.message.is_empty() {
            self.task_draft.clear();
            self.new_due.clear();
            self.task_cite = None;
            self.task_cite_open = false;
            self.say("Added to Tasks.");
        }
        follow
    }

    fn delete_open_note(&mut self) -> Task<Message> {
        self.confirm_delete = false;
        if self.saving {
            self.say("Still saving. Try delete again in a moment.");
            return Task::none();
        }
        let Some(id) = self.selected.clone() else {
            self.discard_draft();
            self.say("Discarded.");
            return Task::none();
        };
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        self.saving = true;
        self.save_token = self.save_token.wrapping_add(1);
        self.unsaved
            .retain(|(key, _, _)| key.as_deref() != Some(id.as_str()));
        self.selected = None;
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.disarm_pause();
        self.histories.remove(&id);
        self.history_order.retain(|key| key != &id);
        if self.history_key == id {
            self.history.clear();
        }
        self.swap_history("");
        self.catalog.retain(|note| note.id != id);
        self.refresh_shown();
        self.post_side();
        self.say("Deleted. It has left the list.");
        let dir = self.dir.clone();
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = write_snapshot(&dir, Some(&id), "", ambient_cli::DELETED_BODY)
                        .map(|(_id, notes)| notes);
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Delete stopped before it finished.".to_owned()))
            },
            Message::Deleted,
        )
    }

    fn finish_delete(&mut self, result: Result<Vec<NoteSummary>, String>) -> Task<Message> {
        self.saving = false;
        match result {
            Ok(notes) => {
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if !self.install_latest() {
                    self.take_notes(notes);
                }
            }
            Err(err) => self.message = err,
        }
        if self.save_again && self.editor_dirty() {
            self.save_again = false;
            return self.start_save(true);
        }
        self.save_again = false;
        Task::none()
    }

    fn discard_draft(&mut self) {
        self.unsaved.retain(|(key, _, _)| key.is_some());
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.disarm_pause();
        self.confirm_delete = false;
        self.save_token = self.save_token.wrapping_add(1);
        self.histories.remove("");
        self.history_order.retain(|key| !key.is_empty());
        if self.history_key.is_empty() {
            self.clear_history();
        }
    }

    fn write_note(&mut self, id: &str, title: &str, body: &str) -> Task<Message> {
        if self.selected.as_deref() == Some(id) {
            return self.put_note_text(id, title, body);
        }
        if let Some(slot) = self
            .unsaved
            .iter_mut()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            title.clone_into(&mut slot.1);
            body.clone_into(&mut slot.2);
        }
        self.apply_note(Some(id.to_owned()), title.to_owned(), body.to_owned())
    }

    fn breeze_on(&self) -> bool {
        !self.loaded || self.boot_at.elapsed() < Duration::from_secs(3)
    }

    fn breeze_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let phase = self.boot_at.elapsed().as_secs_f32();
        canvas(Breeze {
            phase,
            bg: ink.bg,
            ink: ink.muted,
            second: ink.accent,
            word: ink.text,
        })
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn submit_unlock(&mut self) -> Task<Message> {
        let passphrase = self.unlock_input.trim().to_owned();
        if self.open_error == "this folder is locked" {
            match ambient_cli::unlock_folder(&self.dir, &passphrase) {
                Ok(()) => {
                    self.unlock_input.clear();
                    self.open_error.clear();
                    match Notebook::open(&self.dir) {
                        Ok(book) => {
                            self.notebook = Some(book);
                            self.reload();
                            self.post(lane::Job::Hello(self.dir.clone()));
                            self.post(lane::Job::Inbox);
                            self.sync_watch();
                            if let Ok(card) = ambient_cli::contact_card(&self.dir) {
                                self.my_card = card;
                            }
                            self.refresh_link();
                            self.say("Unlocked for this run. The key file on disk stays wrapped.");
                            self.pending = false;
                            self.start_relay();
                            return self.begin_sync();
                        }
                        Err(err) => self.open_error = err.to_string(),
                    }
                }
                Err(err) => self.message = err.to_string(),
            }
            return Task::none();
        }
        match ambient_cli::set_folder_unlock(&self.dir, &passphrase) {
            Ok(()) => {
                self.unlock_input.clear();
                self.say(
                    "Key file wrapped. The passphrase is not the root and not the recovery phrase.",
                );
            }
            Err(err) => self.message = err.to_string(),
        }
        Task::none()
    }

    fn finish_open(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(()) => {
                let Some(book) = take_notebook() else {
                    "could not open this folder".clone_into(&mut self.open_error);
                    self.loaded = true;
                    return Task::none();
                };
                self.adopt_open_book(book);
                self.loaded = true;
                if self.open_error != "this folder is locked"
                    && self.net_mode == ambient_cli::NetMode::ThisComputer
                {
                    self.start_relay();
                }
                self.begin_sync()
            }
            Err(err) => {
                self.open_error = err;
                self.loaded = true;
                Task::none()
            }
        }
    }

    fn adopt_open_book(&mut self, book: Notebook) {
        let paired = !book.holds_root();
        self.notebook = Some(book);
        self.reload();
        if let Some(side) = lane::take_side() {
            self.apply_side(side);
        }
        if let Some(rows) = lane::take_inbox() {
            self.apply_inbox(&rows);
        }
        self.post(lane::Job::Hello(self.dir.clone()));
        self.sync_watch();
        if let Ok(card) = ambient_cli::contact_card(&self.dir) {
            self.my_card = card;
        }
        self.refresh_link();
        self.devices = self
            .notebook
            .as_ref()
            .and_then(|book| book.devices_held().ok())
            .unwrap_or_default();
        if self.device_draft.is_empty()
            && let Some(label) = self
                .devices
                .iter()
                .find(|device| device.current)
                .map(|device| device.label.clone())
        {
            self.device_draft = label;
            self.persist_profile();
        }
        if let Ok(setting) = ambient_cli::net_setting(&self.dir) {
            self.net_mode = setting.mode;
            self.pin_draft = setting.pin;
            self.backup_draft = setting.backup;
        }
        self.pending = ambient_cli::has_unsent(&self.dir).unwrap_or(false);
        if paired && self.notes.is_empty() && self.seen {
            self.say(
                "Paired device. It does not hold the root. It syncs on its own while the relay is on.",
            );
        }
        if !self.seen {
            self.welcome = Some(WelcomeStep::Phrase);
            self.show_phrase();
        }
    }

    fn finish_save(
        &mut self,
        token: u64,
        result: Result<(String, Vec<NoteSummary>), String>,
        title: String,
        body: String,
    ) -> Task<Message> {
        self.saving = false;
        let current = token == self.save_token;
        match result {
            Ok((id, notes)) => {
                if current && self.selected.is_none() {
                    if self.history_key.is_empty() {
                        self.history_key.clone_from(&id);
                    }
                    self.selected = Some(id);
                }
                if current && self.title_text == title && self.stored_body() == body {
                    self.disk_title = title;
                    self.disk_body = body;
                    self.disarm_pause();
                }
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if self.install_latest() {
                    self.post_side();
                    if self.screen == Screen::Tasks {
                        self.rebuild_tasks();
                    }
                } else {
                    self.take_notes(notes);
                }
                if current
                    && self.editor_dirty()
                    && (self.save_again
                        || self
                            .edited_at
                            .is_some_and(|at| at.elapsed() >= Duration::from_millis(700)))
                {
                    self.save_again = false;
                    return self.start_save(true);
                }
                self.save_again = false;
            }
            Err(err) => {
                self.message = err;
                if current && self.save_again && self.editor_dirty() {
                    self.save_again = false;
                    return self.start_save(true);
                }
                self.save_again = false;
            }
        }
        self.take_pending_chapter()
    }

    fn finish_applied(&mut self, result: Result<Vec<NoteSummary>, String>) -> Task<Message> {
        match result {
            Ok(notes) => {
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if self.install_latest() {
                    self.post_side();
                    // The tasks come from the notebook that was just installed. Without this the
                    // tab kept showing the old list until it was left and opened again.
                    if self.screen == Screen::Tasks {
                        self.rebuild_tasks();
                    }
                } else {
                    self.take_notes(notes);
                }
            }
            Err(err) => self.message = err,
        }
        Task::none()
    }

    fn take_notes(&mut self, notes: Vec<NoteSummary>) {
        if self
            .tag
            .as_ref()
            .is_some_and(|tag| !notes.iter().any(|note| in_group(&note.tags, tag)))
        {
            self.tag = None;
        }
        self.catalog = notes;
        self.refresh_shown();
        self.post_side();
        if self.screen == Screen::Tasks {
            self.rebuild_tasks();
        }
    }

    fn apply_note(&self, id: Option<String>, title: String, body: String) -> Task<Message> {
        #[cfg(test)]
        tests_support::record_write(id.as_deref(), &title, &body);
        let dir = self.dir.clone();
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = write_snapshot(&dir, id.as_deref(), &title, &body)
                        .map(|(_id, notes)| notes);
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Save stopped before it finished.".to_owned()))
            },
            Message::Applied,
        )
    }

    fn persist_profile(&mut self) {
        let profile = profile::Profile {
            device: self.device_draft.clone(),
            name: self.your_name.clone(),
            line: self.card_line.clone(),
            appearance: self.appearance,
            seen: self.seen,
            backup_to: self.backup_to.clone(),
            backup_at: self.backup_at,
            hour12: self.hour12,
            week_monday: self.week_monday,
            home_zone: self.home_zone.clone(),
        };
        if let Err(err) = profile::store_profile(&self.dir, &profile) {
            self.message = err;
        }
    }

    fn persist_names(&mut self) {
        let mut names: Vec<(String, String)> = self
            .aliases
            .iter()
            .map(|(id, name)| (id.clone(), name.clone()))
            .collect();
        for (key, name) in &self.people {
            names.push((format!("key:{key}"), name.clone()));
        }
        for (key, line) in &self.card_lines {
            names.push((format!("line:{key}"), line.clone()));
        }
        if let Err(err) = profile::store_names(&self.dir, &names) {
            self.message = err;
        }
    }

    fn refresh_link(&mut self) {
        if self.notebook.is_none() {
            return;
        }
        if let Ok(link) = ambient_cli::contact_link(&self.dir)
            && link != self.contact_link
        {
            self.show_qr = false;
            self.qr_bytes = None;
            self.contact_link = link;
        }
        if let Ok(key) = ambient_cli::contact_key(&self.dir) {
            self.contact_key = key;
        }
    }

    fn accept_offer(&mut self, link: &str) {
        let Some(face) = ambient_cli::offered_card(link) else {
            return;
        };
        let mut wrote = false;
        if self
            .people
            .get(&face.key)
            .is_none_or(|name| name.trim().is_empty())
        {
            self.people.insert(face.key.clone(), face.name);
            wrote = true;
        }
        if !face.line.is_empty() && !self.card_lines.contains_key(&face.key) {
            self.card_lines.insert(face.key, face.line);
            wrote = true;
        }
        if wrote {
            self.persist_names();
        }
    }

    fn remember_alias(&mut self, id: &str, text: String) {
        let one = self
            .chats
            .iter()
            .find(|chat| chat.id == id && chat.members <= 2)
            .and_then(|chat| chat.others.first().cloned());
        if text.trim().is_empty() {
            self.aliases.remove(id);
            if let Some(key) = one {
                self.people.remove(&key);
            }
        } else {
            if let Some(key) = one {
                self.people.insert(key, text.clone());
            }
            self.aliases.insert(id.to_owned(), text);
        }
        self.persist_names();
    }

    fn remember_person(&mut self, key: &str, text: String) {
        if text.trim().is_empty() {
            self.people.remove(key);
        } else {
            self.people.insert(key.to_owned(), text);
        }
        self.persist_names();
    }

    /// Name each member of `id` who was not already in `before`. This computer only.
    fn name_new_people(&mut self, id: &str, name: &str, before: &[String]) {
        if name.trim().is_empty() {
            return;
        }
        let Ok(chats) = ambient_cli::list_conversations(&self.dir) else {
            return;
        };
        let Some(chat) = chats.into_iter().find(|chat| chat.id == id) else {
            return;
        };
        let mut wrote = false;
        for key in chat.others {
            if before.iter().any(|old| old == &key) {
                continue;
            }
            self.people.insert(key, name.to_owned());
            wrote = true;
        }
        if wrote {
            self.persist_names();
        }
    }

    fn finish_onboarding(&mut self) -> Task<Message> {
        let device = self.device_draft.trim().to_owned();
        if device.is_empty() {
            self.say("Name this computer.");
            return Task::none();
        }
        if device.len() > 32 {
            self.say("Use a shorter name for this computer. 32 bytes is the limit.");
            return Task::none();
        }
        let name = self.name_draft.trim().to_owned();
        if name.is_empty() {
            self.say("Say what the card should call you.");
            return Task::none();
        }
        let line = self.line_draft.trim().to_owned();
        if self.dir.join("key").exists() {
            self.say("This folder already has a notebook.");
            return Task::none();
        }
        match Notebook::create_with_label(&self.dir, &device) {
            Ok(book) => {
                self.device_draft = device;
                self.your_name = name;
                self.name_draft.clone_from(&self.your_name);
                self.card_line = line;
                self.line_draft.clone_from(&self.card_line);
                self.seen = false;
                self.persist_profile();
                self.open_error.clear();
                self.adopt_open_book(book);
                self.loaded = true;
                if self.net_mode == ambient_cli::NetMode::ThisComputer {
                    self.start_relay();
                }
                self.begin_sync()
            }
            Err(err) => {
                self.message = err.to_string();
                Task::none()
            }
        }
    }

    fn export_notes(&mut self) -> Task<Message> {
        let Some(book) = self.notebook.as_ref() else {
            self.say("Nothing to export yet.");
            return Task::none();
        };
        let mut notes = Vec::new();
        if let Err(err) = book.visit_held(|id, title, body, _conflict| {
            notes.push((id.to_owned(), title.to_owned(), body.to_owned()));
        }) {
            self.message = err.to_string();
            return Task::none();
        }
        if self.editor_dirty() {
            if let Some(id) = &self.selected {
                if let Some(slot) = notes.iter_mut().find(|(note_id, _, _)| note_id == id) {
                    slot.1.clone_from(&self.title_text);
                    slot.2 = self.stored_body();
                }
            } else if !self.title_text.is_empty() || !self.stored_body().is_empty() {
                notes.push((
                    "unsaved".to_owned(),
                    self.title_text.clone(),
                    self.stored_body(),
                ));
            }
        }
        match profile::export_notes(&self.dir, &notes) {
            Ok(path) => self.say(&format!("Exported to {}.", path.display())),
            Err(err) => self.message = err,
        }
        Task::none()
    }

    fn wipe_notebook(&mut self) -> Task<Message> {
        if self.wipe_draft.trim() != "delete" {
            self.say("Type delete to erase this notebook.");
            return Task::none();
        }
        if self.saving {
            self.say("Still saving. Try again in a moment.");
            return Task::none();
        }
        self.notebook = None;
        let wiped = {
            let _notes = notes_lock();
            let _chat = chat_lock();
            profile::wipe_folder(&self.dir)
        };
        if let Err(err) = wiped {
            self.message = err;
            return Task::none();
        }
        self.reset_after_wipe();
        Task::none()
    }

    fn reset_after_wipe(&mut self) {
        // The words in the personal dictionary are from the notes that were just erased. They
        // must not be written back into the next notebook's file.
        self.my_words.clear();
        self.orders.clear();
        self.list_drag = None;
        self.share_words();
        self.focus = false;
        self.slide_bar.jump(true);
        self.slide_panels.jump(true);
        self.note_panel = false;
        self.catalog.clear();
        self.notes.clear();
        self.tasks.clear();
        self.query.clear();
        self.tag = None;
        self.selected = None;
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.chats.clear();
        self.chat.clear();
        self.chat_id = None;
        self.devices.clear();
        self.draft_editor = text_editor::Content::new();
        self.my_card.clear();
        self.their_card.clear();
        self.unsaved.clear();
        self.recovery_phrase.clear();
        self.restore_phrase.clear();
        self.contact_link.clear();
        self.show_qr = false;
        self.qr_bytes = None;
        self.pending_files.clear();
        self.task_draft.clear();
        self.task_query.clear();
        self.task_rename = None;
        self.task_rename_text.clear();
        self.chat_last.clear();
        self.chat_count.clear();
        self.seen_count.clear();
        self.chat_query.clear();
        self.hide_done = false;
        self.confirm_clear = false;
        self.edit_due.clear();
        self.find_open = false;
        self.find_query.clear();
        self.find_at = usize::MAX;
        self.command_down = false;
        self.shift_down = false;
        self.pointer_down = false;
        self.drag_from = None;
        self.last_press = None;
        self.note_search = false;
        self.task_search = false;
        self.chat_search = false;
        self.invite_pick = None;
        self.call_draft.clear();
        self.wipe_draft.clear();
        self.aliases.clear();
        self.people.clear();
        self.your_name.clear();
        self.name_draft.clear();
        self.card_line.clear();
        self.line_draft.clear();
        self.card_lines.clear();
        self.device_draft.clear();
        self.appearance = profile::Appearance::Dark;
        self.seen = true;
        self.welcome = None;
        self.clear_history();
        self.histories.clear();
        self.history_order.clear();
        self.history_key.clear();
        self.open_error.clear();
        self.message.clear();
        self.disarm_pause();
        self.saving = false;
        self.save_again = false;
        self.pending = false;
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = false;
        self.confirm_delete = false;
        self.send_open = false;
        self.save_token = self.save_token.wrapping_add(1);
        self.loaded = true;
        self.boot_at = Instant::now();
        self.breeze_frame = 0;
    }

    fn onboarding_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let width = (self.size.width - 64.0).clamp(280.0, 440.0);
        container(
            column![
                text("Welcome").font(serif()).size(40),
                text("Name this computer, then the card an invite will offer.")
                    .size(16)
                    .color(ink.muted),
                column![
                    text("This computer").size(13).color(ink.muted),
                    text_input("A short name, like laptop", &self.device_draft)
                        .on_input(Message::DeviceDraft)
                        .padding(12)
                        .style(field_style),
                    text("Saved with this computer. It is not the name on the card.")
                        .size(13)
                        .color(ink.muted),
                ]
                .spacing(8),
                column![
                    text("Your name").size(13).color(ink.muted),
                    text_input("What should people see?", &self.name_draft)
                        .on_input(Message::NameDraft)
                        .padding(12)
                        .style(field_style),
                    text("This goes on the link you send. It is signed by your key.")
                        .size(13)
                        .color(ink.muted),
                ]
                .spacing(8),
                column![
                    text("One line").size(13).color(ink.muted),
                    text_input("Optional. A sentence.", &self.line_draft)
                        .on_input(Message::LineDraft)
                        .padding(12)
                        .style(field_style),
                    text("You can leave this blank.").size(13).color(ink.muted),
                ]
                .spacing(8),
                column![
                    text("Look").size(13).color(ink.muted),
                    self.appearance_row(),
                ]
                .spacing(8),
                action_label("Continue", Message::FinishOnboarding),
                text(self.message.as_str()).size(13).color(ink.muted),
            ]
            .spacing(22)
            .width(width),
        )
        .center(Fill)
        .padding(32)
        .into()
    }

    fn welcome_screen(&self) -> Element<'_, Message> {
        let page = match self.welcome {
            Some(WelcomeStep::Guide) => self.guide_screen(),
            _ => self.phrase_screen(),
        };
        container(vscroll(page).width(Fill).height(Fill))
            .padding(32)
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn phrase_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let width = (self.size.width - 64.0).clamp(280.0, 520.0);
        let mut body = column![
            text("Recovery phrase").font(serif()).size(40),
            text("These words rebuild the notebook on a new computer. They are not a passphrase. A passphrase is optional. It only locks the key file on this computer, and you can set it later in Settings.")
                .size(16)
                .color(ink.muted),
        ]
        .spacing(16)
        .width(width);
        if self.recovery_phrase.is_empty() {
            body = body.push(text(self.message.as_str()).size(14).color(ink.muted));
            body = body.push(action_label("Show recovery phrase", Message::ShowPhrase));
        } else {
            body = body.push(
                container(text(&self.recovery_phrase).size(16))
                    .padding(14)
                    .width(Fill)
                    .style(field_well),
            );
            body = body.push(
                row![
                    ghost_label("Copy phrase", Message::CopyPhrase),
                    action_label("I've stored it", Message::WelcomeStored),
                ]
                .spacing(8),
            );
            body = body.push(
                text("Store it offline. Settings can show it again.")
                    .size(13)
                    .color(ink.muted),
            );
        }
        container(body)
            .width(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }

    fn guide_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let width = (self.size.width - 64.0).clamp(280.0, 560.0);
        container(
            column![
                text("Before the notes").font(serif()).size(40),
                text("The card is ready. Settings holds the rest.")
                    .size(16)
                    .color(ink.muted),
                self.guide_card(),
                guide_line(
                    "Passphrase",
                    "Optional. Settings, then Devices. It locks the key file on this computer. It is not the recovery phrase, and this welcome does not ask for one.",
                ),
                guide_line(
                    "Devices",
                    "This computer already has a name. Another computer can pair later. Revoke stops new notes from a computer you no longer use. Notes it already wrote stay.",
                ),
                guide_line(
                    "Recovery",
                    "The phrase is in Settings if you need to copy it again. Restoring on a new computer does not bring back old device keys.",
                ),
                guide_line(
                    "Your notes",
                    "Export a copy into this folder, or erase this computer's notebook. Erasing does not reach another computer.",
                ),
                guide_line(
                    "People",
                    "What you call someone stays on this computer. Their link can offer their name. If you already typed one, yours stays.",
                ),
                guide_line(
                    "Chat",
                    "Either you paste their link and start, or you send yours and wait. The words can sync. A file stays on this computer.",
                ),
                action_label("Open the notebook", Message::WelcomeDone),
            ]
            .spacing(18)
            .width(width),
        )
        .width(Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .into()
    }

    fn settings_sheet(&self) -> Element<'_, Message> {
        let width = (self.size.width - self.gutter_x() * 2.0 - 24.0).clamp(280.0, 640.0);
        container(
            vscroll(
                container(
                    column![
                        Self::sheet_title("Settings"),
                        self.card_section(),
                        self.appearance_section(),
                        self.time_section(),
                        self.devices_sheet(),
                        self.recovery_sheet(),
                        self.people_section(),
                        self.backup_section(),
                        self.data_section(),
                        Self::keys_section(),
                    ]
                    .spacing(16)
                    .width(width)
                    .padding(iced::Padding {
                        top: 4.0,
                        right: 0.0,
                        bottom: 36.0,
                        left: 0.0,
                    }),
                )
                .width(Fill)
                .align_x(iced::alignment::Horizontal::Center),
            )
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// The keys, since most of them have no button to find them by.
    fn keys_section() -> Element<'static, Message> {
        const KEYS: [(&str, &str); 17] = [
            ("Ctrl or Cmd + S", "Save the note now"),
            ("Ctrl or Cmd + N", "A new note"),
            ("Ctrl or Cmd + F", "Find in the note"),
            ("Ctrl or Cmd + .", "Write without distractions, or leave it"),
            ("Ctrl or Cmd + Z", "Undo"),
            ("Ctrl or Cmd + Shift + Z", "Redo"),
            ("Ctrl or Cmd + B", "Bold"),
            ("Ctrl or Cmd + I", "Italic"),
            ("Ctrl or Cmd + K", "Link"),
            ("Ctrl or Cmd + E", "Code"),
            ("Ctrl or Cmd + Shift + X", "Strike"),
            ("Ctrl or Cmd + Shift + H", "Highlight"),
            ("Tab", "Nest a list item, or every one you have selected"),
            ("Shift + Tab", "Bring a list item out one level"),
            (
                "Enter on an empty item",
                "End the list, or step a nested item out",
            ),
            (
                "Ctrl or Cmd + Shift + D",
                "Add the underlined word to your dictionary",
            ),
            ("Enter in the title", "Start writing"),
        ];
        let mut rows = column![].spacing(8);
        for (keys, does) in KEYS {
            rows = rows.push(
                row![
                    text(keys).size(14).width(210),
                    text(does).size(14).width(Fill),
                ]
                .spacing(12),
            );
        }
        settings_card("Keys", rows.into())
    }

    fn card_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let short: String = self.contact_key.chars().take(8).collect();
        let mut body = column![].spacing(10);
        if !self.your_name.trim().is_empty() && !self.contact_key.is_empty() {
            body = body.push(speaker(&self.your_name, &self.contact_key));
        }
        body = body
            .push(
                text("This name goes with the link you send. It is signed by your key. Changing it does not make a new key.")
                    .size(14)
                    .color(ink.muted),
            )
            .push(
                text_input("Your name", &self.name_draft)
                    .on_input(Message::NameDraft)
                    .padding(12)
                    .style(field_style),
            )
            .push(
                text_input("One line, optional", &self.line_draft)
                    .on_input(Message::LineDraft)
                    .padding(12)
                    .style(field_style),
            );
        if !short.is_empty() {
            body = body.push(text(short).size(13).color(ink.muted));
        }
        body = body.push(ghost_label("Copy link", Message::CopyLink));
        settings_card("Card", body.into())
    }

    fn guide_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        if self.your_name.trim().is_empty() || self.contact_key.is_empty() {
            return space().height(0).into();
        }
        let short: String = self.contact_key.chars().take(8).collect();
        let mut body = column![
            text("Your card").font(serif()).size(22),
            speaker(&self.your_name, &self.contact_key),
        ]
        .spacing(6);
        if !self.card_line.trim().is_empty() {
            body = body.push(text(&self.card_line).size(14).color(ink.muted));
        }
        body = body.push(text(short).size(13).color(ink.muted));
        body = body.push(
            text("This is what their computer can store when you send the link.")
                .size(14)
                .color(ink.muted),
        );
        body.into()
    }

    fn appearance_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        settings_card(
            "Appearance",
            column![
                text("Light, dark, or the same as this computer.")
                    .size(14)
                    .color(ink.muted),
                self.appearance_row(),
            ]
            .spacing(12)
            .into(),
        )
    }

    /// How times are shown, which day the week starts on, and where this computer says it is.
    fn time_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![
            text("How times are shown, and which day a week begins on.")
                .size(14)
                .color(ink.muted),
            row![
                text("Clock").size(14).width(110),
                choice_button("12 hour (3:42 PM)", self.hour12, Message::PickHour12(true),),
                choice_button("24 hour (15:42)", !self.hour12, Message::PickHour12(false),),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            row![
                text("Week begins").size(14).width(110),
                choice_button("Sunday", !self.week_monday, Message::PickWeekMonday(false),),
                choice_button("Monday", self.week_monday, Message::PickWeekMonday(true)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12);
        body = body.push(hairline());
        body = body.push(self.zone_lines());
        settings_card("Time", body.into())
    }

    /// Where this computer says it is, where home is, and whether those differ.
    fn zone_lines(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let here = if self.zone_name.is_none() && self.clock.offset == 0 && self.zone.is_utc() {
            "This computer did not say what time zone it is in, so times are shown in UTC."
                .to_owned()
        } else {
            format!(
                "This computer is in {} ({}).",
                self.zone_name.as_deref().unwrap_or(&self.clock.zone),
                self.clock.offset_label()
            )
        };
        let mut lines = column![text(here).size(14)].spacing(8);
        if !self.home_zone.is_empty() {
            lines = lines.push(
                text(format!("Home is {}.", self.home_zone))
                    .size(14)
                    .color(ink.muted),
            );
        }
        if let Some(trip) = &self.trip {
            lines = lines.push(text(self.trip_words(trip)).size(14).color(ink.muted));
            lines = lines.push(row![action_label("Make this home", Message::SetHomeHere),]);
        }
        lines = lines.push(
            text("A time on a task is the clock on the wall where you are. It does not move to follow a trip.")
                .size(12)
                .color(ink.muted),
        );
        lines.into()
    }

    /// What it is at home, and how far that is, in a sentence.
    fn trip_words(&self, trip: &clock::Trip) -> String {
        match &trip.home {
            Some(home) => format!(
                "Home is {}, where it is {} now, {} here.",
                trip.home_name,
                clock::clock_text(home.minutes / 60, home.minutes % 60),
                clock::gap_words(home.offset, self.clock.offset)
            ),
            None => format!("Home is {}.", trip.home_name),
        }
    }

    /// The notice on the Tasks tab that this computer is away from home. It says what the clocks
    /// are and that nothing on a task has changed, and asks nothing before it goes.
    fn trip_banner(&self) -> Option<Element<'_, Message>> {
        let trip = self.trip.as_ref()?;
        if self.trip_dismissed.as_deref() == Some(self.zone_key().as_str()) {
            return None;
        }
        let ink = self.ink();
        let here = self.zone_name.as_deref().unwrap_or(&self.clock.zone);
        Some(
            container(
                column![
                    text(format!(
                        "You are in {here} ({}), away from home.",
                        self.clock.offset_label()
                    ))
                    .size(14),
                    text(format!(
                        "{} Task times are the clock where you are now, so a task due at 9:00 AM is due at 9:00 AM here. Nothing on a task was changed.",
                        self.trip_words(trip)
                    ))
                    .size(13)
                    .color(ink.muted),
                    row![
                        ghost_label("Hide this", Message::DismissTrip),
                        ghost_label("Make this home", Message::SetHomeHere),
                    ]
                    .spacing(4),
                ]
                .spacing(6),
            )
            .padding(14)
            .width(Fill)
            .style(card_style)
            .into(),
        )
    }

    fn appearance_row(&self) -> Element<'_, Message> {
        row![
            choice_button(
                "Light",
                self.appearance == profile::Appearance::Light,
                Message::PickAppearance(profile::Appearance::Light),
            ),
            choice_button(
                "Dark",
                self.appearance == profile::Appearance::Dark,
                Message::PickAppearance(profile::Appearance::Dark),
            ),
            choice_button(
                "System",
                self.appearance == profile::Appearance::System,
                Message::PickAppearance(profile::Appearance::System),
            ),
        ]
        .spacing(8)
        .width(Fill)
        .into()
    }

    fn people_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut people = column![].spacing(16);
        if self.chats.is_empty() {
            people = people.push(
                text("No one yet. Invite someone from Chat.")
                    .size(14)
                    .color(ink.muted),
            );
        }
        for chat in &self.chats {
            let tail = if chat.other.is_empty() {
                "Key unknown".to_owned()
            } else {
                format!("Key {}", chat.other)
            };
            let draft = self.aliases.get(&chat.id).cloned().unwrap_or_default();
            let id = chat.id.clone();
            people = people.push(
                column![
                    text(tail).size(12).color(ink.muted),
                    text_input("What do you call them?", &draft)
                        .on_input(move |text| Message::AliasDraft(id.clone(), text))
                        .padding(12)
                        .style(field_style),
                ]
                .spacing(6),
            );
        }
        settings_card(
            "People",
            column![
                text("Names you type here stay on this computer. A link can offer their name. If you already typed one, yours stays.")
                    .size(14)
                    .color(ink.muted),
                people,
            ]
            .spacing(14)
            .into(),
        )
    }

    /// Where the notes go if this computer's disk does not survive.
    fn backup_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let warn = Color::from_rgb8(184, 78, 62);
        let wrapped = ambient_cli::folder_key_is_wrapped(&self.dir);
        let last = if self.backup_at == 0 {
            text("No backup yet.").size(14).color(warn)
        } else {
            let when = clock::when(Some(self.backup_at), &clock::load_zone().0, clock::now_ms());
            text(format!("Last backup: {}, {}.", when.day, when.time))
                .size(14)
                .color(ink.muted)
        };
        let key = if wrapped {
            text("The key file is locked with your passphrase, so the copy is locked too.")
                .size(14)
                .color(ink.muted)
        } else {
            text("The key file is not locked with a passphrase. Anyone who gets the copy can read your notes. Lock the key file, above, first.")
                .size(14)
                .color(warn)
        };
        let default = profile::default_backup_dir().display().to_string();
        settings_card(
            "Back up",
            column![
                text("Your notes are only on this computer until you copy them somewhere else, such as a USB drive or a folder that syncs. The copy has everything: the sealed notes, your chats, and the key file. It leaves out the readable Markdown in the export folder, which has no lock.")
                    .size(14)
                    .color(ink.muted),
                key,
                text_input(&default, &self.backup_to)
                    .on_input(Message::BackupTo)
                    .on_submit(Message::BackUp)
                    .padding(12)
                    .style(field_style),
                action_label(
                    if self.backup_busy { "Backing up" } else { "Back up now" },
                    Message::BackUp,
                ),
                last,
                text(format!("To restore, close the app, put the copy where this notebook is, {}, and open it again.", self.dir.display()))
                    .size(13)
                    .color(ink.muted),
            ]
            .spacing(12)
            .into(),
        )
    }

    fn data_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        settings_card(
            "Your notes",
            column![
                text("Save a copy of your notes into this folder, or erase this computer's copy. Erasing does not reach another computer.")
                    .size(14)
                    .color(ink.muted),
                action_label("Export notes", Message::ExportNotes),
                text("Type delete, then erase. This cannot be undone.")
                    .size(13)
                    .color(ink.muted),
                text_input("delete", &self.wipe_draft)
                    .on_input(Message::WipeDraft)
                    .padding(12)
                    .style(field_style),
                danger_label("Erase this notebook", Message::ConfirmWipe),
            ]
            .spacing(12)
            .into(),
        )
    }

    fn chat_name(&self, chat: &ambient_cli::ConversationSummary) -> String {
        self.aliases
            .get(&chat.id)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| chat.label.clone())
    }
}

#[derive(Clone, Copy)]
struct Ink {
    bg: Color,
    text: Color,
    muted: Color,
    accent: Color,
    on_accent: Color,
    surface: Color,
    hairline: Color,
    field: Color,
}

impl Ink {
    fn from_dark(dark: bool) -> Self {
        if dark {
            Self {
                bg: Color::from_rgb8(0x1C, 0x19, 0x17),
                text: Color::from_rgb8(0xED, 0xEA, 0xDE),
                muted: Color::from_rgb8(0xA8, 0xA2, 0x9E),
                accent: Color::from_rgb8(0x66, 0xA5, 0xE1),
                on_accent: Color::from_rgb8(0x1C, 0x19, 0x17),
                surface: Color::from_rgb8(0x26, 0x22, 0x20),
                hairline: Color::from_rgb8(0x3A, 0x34, 0x2E),
                field: Color::from_rgb8(0x14, 0x12, 0x10),
            }
        } else {
            Self {
                bg: Color::from_rgb8(0xED, 0xEA, 0xDE),
                text: Color::from_rgb8(0x1C, 0x19, 0x17),
                muted: Color::from_rgb8(0x6B, 0x65, 0x60),
                accent: Color::from_rgb8(0x16, 0x44, 0x6F),
                on_accent: Color::from_rgb8(0xED, 0xEA, 0xDE),
                surface: Color::from_rgb8(0xE4, 0xE0, 0xD2),
                hairline: Color::from_rgb8(0xD0, 0xCB, 0xBC),
                field: Color::from_rgb8(0xF7, 0xF5, 0xEE),
            }
        }
    }
}

fn content_arrived(detail: &str) -> bool {
    if detail == "Nothing new." {
        return false;
    }
    let mut saw = false;
    let mut arrived = false;
    for part in detail.split(',') {
        let part = part.trim();
        if let Some(count) = part.strip_suffix(" notes in") {
            saw = true;
            if count.trim() != "0" {
                arrived = true;
            }
        } else if let Some(count) = part.strip_suffix(" messages in") {
            saw = true;
            if count.trim() != "0" {
                arrived = true;
            }
        }
    }
    !saw || arrived
}

fn ink_of(theme: &Theme) -> Ink {
    Ink::from_dark(theme.palette().background.r < 0.5)
}

fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

#[derive(Clone, Debug)]
struct SyncOutcome {
    detail: String,
    pending: bool,
    failed: bool,
    transport: bool,
}

/// Notes, the search index, and snapshots. Never take the chat lock first.
fn notes_lock() -> std::sync::MutexGuard<'static, ()> {
    static NOTES: Mutex<()> = Mutex::new(());
    NOTES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Conversation logs. Never take this and then the notes lock.
fn chat_lock() -> std::sync::MutexGuard<'static, ()> {
    static CHATS: Mutex<()> = Mutex::new(());
    CHATS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn bubbles(lines: Vec<ambient_cli::ChatMessage>) -> Vec<ChatBubble> {
    let tz = clock::load_zone().0;
    let now = clock::now_ms();
    lines
        .into_iter()
        .map(|line| {
            let when = clock::when(line.unix_ms, &tz, now);
            ChatBubble {
                text: line.text,
                time: when.time,
                day: when.day,
                mine: line.mine,
                sender: line.sender,
                cite: line.cite,
            }
        })
        .collect()
}

fn lane_messages() -> impl iced::futures::Stream<Item = Message> + Send {
    use iced::futures::StreamExt;
    lane::listen().map(Message::Lane)
}

fn wake_stream(wake: &Wake) -> iced::futures::stream::BoxStream<'static, Message> {
    use iced::futures::stream::StreamExt;
    let delay = match *wake {
        Wake::Pause(_) => Duration::from_millis(700),
        Wake::Search(_) => Duration::from_millis(200),
        Wake::Scroll(_) => SCROLL_SHOWN + Duration::from_millis(200),
    };
    let fired = *wake;
    iced::futures::stream::once(async move {
        tokio::time::sleep(delay).await;
        Message::Woke(fired)
    })
    .boxed()
}

fn sync_outcome(dir: &Path) -> SyncOutcome {
    let skip = lane::watched();
    match ambient_cli::sync_notebook_except(dir, skip.as_deref()) {
        Ok(detail) => SyncOutcome {
            pending: ambient_cli::has_unsent(dir).unwrap_or(true),
            detail,
            failed: false,
            transport: false,
        },
        Err(err) => {
            let detail = err.to_string();
            let transport = matches!(
                detail.as_str(),
                "no relay answered"
                    | "relay unreachable"
                    | "relay write failed"
                    | "relay read failed"
                    | "relay reply missing"
                    | "relay reply rejected"
                    | "unavailable"
                    | "too_large"
                    | "bad_request"
            );
            SyncOutcome {
                detail,
                pending: true,
                failed: true,
                transport,
            }
        }
    }
}

fn action_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => mix(ink.accent, Color::WHITE, 0.12),
        button::Status::Pressed => mix(ink.accent, Color::BLACK, 0.16),
        button::Status::Disabled => mix(ink.accent, ink.bg, 0.45),
        button::Status::Active => ink.accent,
    };
    button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: ink.on_accent,
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn quiet_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => Some(iced::Background::Color(ink.surface)),
        button::Status::Pressed => Some(iced::Background::Color(mix(ink.accent, ink.bg, 0.85))),
        _ => None,
    };
    button::Style {
        background,
        text_color: ink.text,
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn ghost_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => Some(iced::Background::Color(ink.surface)),
        button::Status::Pressed => Some(iced::Background::Color(mix(ink.accent, ink.bg, 0.85))),
        _ => None,
    };
    button::Style {
        background,
        text_color: ink.text,
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

/// Width of the panel on the right, inside its edge.
const CONTEXT_WIDTH: f32 = 264.0;

/// A larger icon button for the panel header, with a soft background while its panel is the one
/// showing.
fn panel_icon(
    handle: iced_image::Handle,
    hint_text: &'static str,
    message: Message,
    active: bool,
) -> Element<'static, Message> {
    let face = button(iced_image::Image::new(handle).width(22).height(22))
        .padding(8)
        .on_press(message)
        .style(move |theme: &Theme, status| {
            let ink = ink_of(theme);
            let mut style = icon_style(theme, status);
            if active && style.background.is_none() {
                style.background = Some(iced::Background::Color(mix(ink.bg, ink.accent, 0.18)));
            }
            style
        });
    hover_label(face, hint_text)
}

fn icon_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Some(iced::Background::Color(ink.surface))
        }
        _ => None,
    };
    button::Style {
        background,
        text_color: ink.text,
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn list_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let ink = ink_of(theme);
        let background = if selected {
            Some(iced::Background::Color(mix(ink.bg, ink.accent, 0.18)))
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Some(iced::Background::Color(mix(ink.surface, ink.bg, 0.35)))
        } else {
            None
        };
        button::Style {
            background,
            text_color: ink.text,
            border: iced::Border {
                radius: 8.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        }
    }
}

fn segment_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if active {
            action_style(theme, status)
        } else {
            let ink = ink_of(theme);
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(iced::Background::Color(mix(ink.bg, ink.surface, 0.6)))
                }
                _ => None,
            };
            button::Style {
                background,
                text_color: ink.muted,
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        }
    }
}

fn field_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let ink = ink_of(theme);
    text_input::Style {
        background: iced::Background::Color(ink.field),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        icon: ink.muted,
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn title_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let ink = ink_of(theme);
    text_input::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: iced::Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        icon: ink.muted,
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn editor_style(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let ink = ink_of(theme);
    text_editor::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: iced::Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn field_editor_style(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let ink = ink_of(theme);
    text_editor::Style {
        background: iced::Background::Color(ink.field),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn surface_pane(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        ..Default::default()
    }
}

/// A selection as a chat line: the words quoted, then where they came from. The note itself goes
/// along as the line's cite, so the message links back to it.
fn selection_message(title: &str, lines: (usize, usize), home: &str, words: &str) -> String {
    let mut out = String::new();
    for line in words.trim_end().lines() {
        out.push_str("> ");
        out.push_str(line);
        out.push('\n');
    }
    let name = if title.trim().is_empty() {
        "Untitled"
    } else {
        title.trim()
    };
    let place = if lines.0 == lines.1 {
        format!("line {}", lines.0)
    } else {
        format!("lines {}–{}", lines.0, lines.1)
    };
    out.push_str("\n— ");
    out.push_str(name);
    out.push_str(", ");
    out.push_str(&place);
    if !home.is_empty() {
        out.push_str(", written on ");
        out.push_str(home);
    }
    out
}

/// The parts of a tag read as a path: `Novel / Part 1` is `Part 1` inside `Novel`.
fn tag_path(tag: &str) -> Vec<&str> {
    tag.split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

/// The path as one lowercase key, so `Novel` and `novel` are the same group.
fn group_key(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether a note is in the group at `path`: it has that tag, or a tag inside it.
fn in_group(tags: &[String], path: &str) -> bool {
    let want = group_key(&tag_path(path));
    tags.iter().any(|tag| {
        let have = group_key(&tag_path(tag));
        have == want
            || have
                .strip_prefix(want.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// A note being dragged: the group it is in, the note, and what the pointer is over now, which
/// is another note's id or `top_of(group)` for the group's name.
#[derive(Debug)]
struct ListDrag {
    group: String,
    id: String,
    over: Option<String>,
}

/// What the pointer reports over a group's name, which is a drop at the top of that group.
fn top_of(group: &str) -> String {
    format!("top:{group}")
}

/// Whether one of the tags is exactly this group, not a group inside it.
fn has_exact_tag(tags: &[String], group: &str) -> bool {
    tags.iter().any(|tag| group_key(&tag_path(tag)) == group)
}

/// Titles compared the way a person counts: `Chapter 2` before `Chapter 10`, and capitals
/// ignored.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let number = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(&c) = chars.peek() {
                        if !c.is_ascii_digit() {
                            break;
                        }
                        digits.push(c);
                        chars.next();
                    }
                    digits.trim_start_matches('0').to_owned()
                };
                let (one, two) = (number(&mut left), number(&mut right));
                let order = one.len().cmp(&two.len()).then_with(|| one.cmp(&two));
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
                left.next();
                right.next();
            }
        }
    }
}

/// Put notes, as id and title, in the order they are shown in a group: the ones placed by hand
/// first, in that order, then the rest alphabetically.
fn order_ids(items: &mut [(String, String)], by_hand: &[String]) {
    items.sort_by(|a, b| {
        let place = |id: &str| by_hand.iter().position(|have| have == id);
        match (place(&a.0), place(&b.0)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => natural_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)),
        }
    });
}

/// `ids` with `moved` taken out and put before `before`, or first when `before` is none. A note
/// moved down goes after the one it was dropped on, so it lands where the pointer is.
fn move_in_order(ids: &[String], moved: &str, before: Option<&str>) -> Vec<String> {
    let Some(from) = ids.iter().position(|id| id == moved) else {
        return ids.to_vec();
    };
    let mut out: Vec<String> = ids.to_vec();
    out.remove(from);
    let at = match before {
        None => 0,
        Some(target) => {
            let (Some(to), Some(now)) = (
                ids.iter().position(|id| id == target),
                out.iter().position(|id| id == target),
            ) else {
                return ids.to_vec();
            };
            if to > from { now + 1 } else { now }
        }
    };
    out.insert(at.min(out.len()), moved.to_owned());
    out
}

/// One line of the note list.
#[derive(Debug, Eq, PartialEq)]
enum ListLine {
    /// A group. `path` is its key, `count` the notes in it and in the groups inside it.
    Group {
        path: String,
        name: String,
        depth: usize,
        count: usize,
        open: bool,
    },
    Note {
        index: usize,
        depth: usize,
        /// The key of the group it is listed in, empty for a note with none.
        group: String,
    },
}

#[derive(Default)]
struct GroupNode {
    name: String,
    children: BTreeMap<String, GroupNode>,
    notes: Vec<usize>,
}

impl GroupNode {
    fn everyone(&self, into: &mut BTreeSet<usize>) {
        into.extend(self.notes.iter().copied());
        for child in self.children.values() {
            child.everyone(into);
        }
    }
}

/// File note `index` under the group at the end of `parts`, making the groups on the way.
fn place(level: &mut BTreeMap<String, GroupNode>, parts: &[&str], at: usize, index: usize) -> bool {
    let Some(part) = parts.get(at) else {
        return false;
    };
    let key = group_key(parts.get(..=at).unwrap_or(&[]));
    let node = level.entry(key).or_insert_with(|| GroupNode {
        name: (*part).to_owned(),
        ..GroupNode::default()
    });
    if at + 1 == parts.len() {
        if !node.notes.contains(&index) {
            node.notes.push(index);
        }
        true
    } else {
        place(&mut node.children, parts, at + 1, index)
    }
}

/// The note list as groups inside groups, from the notes' tags. A group is a tag, and a tag
/// with slashes is a group inside a group. A note with two tags is in both. A note with no tag
/// is listed after the groups. A folded group hides what is inside it.
fn group_outline(
    notes: &[NoteSummary],
    folded: &BTreeSet<String>,
    orders: &BTreeMap<String, Vec<String>>,
) -> Vec<ListLine> {
    fn walk(
        key: &str,
        node: &GroupNode,
        depth: usize,
        shown: (
            &[NoteSummary],
            &BTreeSet<String>,
            &BTreeMap<String, Vec<String>>,
        ),
        out: &mut Vec<ListLine>,
    ) {
        let (notes, folded, orders) = shown;
        let mut all = BTreeSet::new();
        node.everyone(&mut all);
        let open = !folded.contains(key);
        out.push(ListLine::Group {
            path: key.to_owned(),
            name: node.name.clone(),
            depth,
            count: all.len(),
            open,
        });
        if !open {
            return;
        }
        for (child_key, child) in &node.children {
            walk(child_key, child, depth + 1, shown, out);
        }
        // Placed by hand first, then alphabetical, whatever order they were filed in.
        let mut items: Vec<(String, String)> = node
            .notes
            .iter()
            .filter_map(|index| notes.get(*index))
            .map(|note| (note.id.clone(), note.title.clone()))
            .collect();
        order_ids(&mut items, orders.get(key).map_or(&[][..], Vec::as_slice));
        for (id, _) in items {
            if let Some(index) = notes.iter().position(|note| note.id == id) {
                out.push(ListLine::Note {
                    index,
                    depth: depth + 1,
                    group: key.to_owned(),
                });
            }
        }
    }
    let mut roots: BTreeMap<String, GroupNode> = BTreeMap::new();
    let mut loose = Vec::new();
    for (index, note) in notes.iter().enumerate() {
        let mut placed = false;
        for tag in &note.tags {
            let parts = tag_path(tag);
            if parts.is_empty() {
                continue;
            }
            placed |= place(&mut roots, &parts, 0, index);
        }
        if !placed {
            loose.push(index);
        }
    }
    let mut out = Vec::new();
    for (key, node) in &roots {
        walk(key, node, 0, (notes, folded, orders), &mut out);
    }
    let mut loose_items: Vec<(String, String)> = loose
        .iter()
        .filter_map(|index| notes.get(*index))
        .map(|note| (note.id.clone(), note.title.clone()))
        .collect();
    order_ids(&mut loose_items, &[]);
    for (id, _) in loose_items {
        if let Some(index) = notes.iter().position(|note| note.id == id) {
            out.push(ListLine::Note {
                index,
                depth: 0,
                group: String::new(),
            });
        }
    }
    out
}

/// A dot in the graph. `x` and `y` are in world units, which are pixels at scale one.
#[derive(Debug)]
struct GraphNode {
    id: String,
    title: String,
    group: usize,
    x: f32,
    y: f32,
}

/// A group of dots and the circle drawn behind them. `shade` picks its color, and is none for the
/// notes that have no group.
#[derive(Debug)]
struct GraphGroup {
    name: String,
    shade: Option<usize>,
    x: f32,
    y: f32,
    r: f32,
}

/// The notes as dots, the groups as circles around them, and the links as lines.
#[derive(Debug)]
struct GraphData {
    nodes: Vec<GraphNode>,
    groups: Vec<GraphGroup>,
    edges: Vec<(usize, usize)>,
    /// The box that holds every circle: left, top, right, bottom.
    bounds: (f32, f32, f32, f32),
}

/// The room one dot takes in its group, and the space kept round a group's dots.
const DOT_ROOM: f32 = 34.0;
const GROUP_PAD: f32 = 30.0;
const GROUP_GAP: f32 = 60.0;

/// Groups notes by the top of their first tag. `Novel/Part 1` is in `Novel`. A note with no tag is
/// in a last group of its own. Each group is laid out on its own, then the groups are moved
/// apart, with the ones that link to each other kept nearest. All of it is fixed, so the same
/// notes give the same picture.
fn build_graph(catalog: &[NoteSummary], links: &[(String, String)]) -> GraphData {
    let notes: Vec<&NoteSummary> = catalog.iter().filter(|note| !is_tasks_note(note)).collect();
    let tops: Vec<Option<(String, String)>> = notes
        .iter()
        .map(|note| {
            note.tags.iter().find_map(|tag| {
                let top = *tag_path(tag).first()?;
                Some((top.to_lowercase(), top.to_owned()))
            })
        })
        .collect();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for (key, shown) in tops.iter().flatten() {
        names.entry(key.clone()).or_insert_with(|| shown.clone());
    }
    let keys: Vec<String> = names.keys().cloned().collect();
    let loose = notes.iter().zip(&tops).any(|(_, top)| top.is_none());
    let group_of: Vec<usize> = tops
        .iter()
        .map(|top| match top {
            Some((key, _)) => keys.iter().position(|have| have == key).unwrap_or(0),
            None => keys.len(),
        })
        .collect();
    let count = keys.len() + usize::from(loose);
    let edges: Vec<(usize, usize)> = links
        .iter()
        .filter_map(|(from, to)| {
            let from = notes.iter().position(|note| note.id == *from)?;
            let to = notes.iter().position(|note| note.id == *to)?;
            Some((from, to))
        })
        .collect();

    // Each group on its own: dots in a sunflower, then a few rounds to bring linked dots close.
    let mut local = vec![(0.0_f32, 0.0_f32); notes.len()];
    let mut radius = vec![0.0_f32; count];
    for group in 0..count {
        let members: Vec<usize> = (0..notes.len())
            .filter(|&index| group_of.get(index) == Some(&group))
            .collect();
        let spots = settle_group(&members, &edges);
        let mut far = 0.0_f32;
        for (&index, &(x, y)) in members.iter().zip(&spots) {
            if let Some(spot) = local.get_mut(index) {
                *spot = (x, y);
            }
            far = far.max(x.hypot(y));
        }
        if let Some(r) = radius.get_mut(group) {
            *r = far + GROUP_PAD;
        }
    }

    // The groups apart from each other, nearest where they link most.
    let between = links_between(&edges, &group_of, count);
    let centers = place_groups(&radius, &between);

    let groups: Vec<GraphGroup> = (0..count)
        .map(|group| {
            let (x, y) = centers.get(group).copied().unwrap_or((0.0, 0.0));
            GraphGroup {
                name: keys
                    .get(group)
                    .and_then(|key| names.get(key))
                    .cloned()
                    .unwrap_or_else(|| "No group".to_owned()),
                shade: (group < keys.len()).then_some(group),
                x,
                y,
                r: radius.get(group).copied().unwrap_or(GROUP_PAD),
            }
        })
        .collect();
    let dots: Vec<GraphNode> = notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let group = group_of.get(index).copied().unwrap_or(0);
            let (cx, cy) = centers.get(group).copied().unwrap_or((0.0, 0.0));
            let (lx, ly) = local.get(index).copied().unwrap_or((0.0, 0.0));
            GraphNode {
                id: note.id.clone(),
                title: if note.title.is_empty() {
                    "Untitled".to_owned()
                } else {
                    note.title.clone()
                },
                group,
                x: cx + lx,
                y: cy + ly,
            }
        })
        .collect();
    let bounds = graph_bounds(&groups);
    GraphData {
        nodes: dots,
        groups,
        edges,
        bounds,
    }
}

/// The box that holds every circle: left, top, right, bottom.
fn graph_bounds(groups: &[GraphGroup]) -> (f32, f32, f32, f32) {
    if groups.is_empty() {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let mut bounds = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for group in groups {
        bounds.0 = bounds.0.min(group.x - group.r);
        bounds.1 = bounds.1.min(group.y - group.r);
        bounds.2 = bounds.2.max(group.x + group.r);
        bounds.3 = bounds.3.max(group.y + group.r);
    }
    bounds
}

/// How many links join each pair of groups.
fn links_between(edges: &[(usize, usize)], group_of: &[usize], count: usize) -> Vec<Vec<u32>> {
    let mut between = vec![vec![0_u32; count]; count];
    for &(from, to) in edges {
        let (Some(&a), Some(&b)) = (group_of.get(from), group_of.get(to)) else {
            continue;
        };
        if a != b {
            if let Some(cell) = between.get_mut(a).and_then(|row| row.get_mut(b)) {
                *cell += 1;
            }
            if let Some(cell) = between.get_mut(b).and_then(|row| row.get_mut(a)) {
                *cell += 1;
            }
        }
    }
    between
}

/// Spots, round the group's middle, for these notes: a sunflower, then linked pairs pulled
/// together for a few rounds with the rest kept apart. Indexes are into the note list.
#[allow(clippy::cast_precision_loss)]
fn settle_group(members: &[usize], edges: &[(usize, usize)]) -> Vec<(f32, f32)> {
    let n = members.len();
    let mut at: Vec<(f32, f32)> = (0..n)
        .map(|k| {
            let turn = k as f32 * 2.399_963;
            let reach = DOT_ROOM * ((k as f32) + 0.5).sqrt() * 0.85;
            (reach * turn.cos(), reach * turn.sin())
        })
        .collect();
    if n < 3 {
        return at;
    }
    let slot = |note: usize| members.iter().position(|&member| member == note);
    let links: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|&(from, to)| Some((slot(from)?, slot(to)?)))
        .collect();
    for _ in 0..(4000 / n).clamp(10, 60) {
        let mut push = vec![(0.0_f32, 0.0_f32); n];
        for a in 0..n {
            for b in (a + 1)..n {
                let (Some(&(ax, ay)), Some(&(bx, by))) = (at.get(a), at.get(b)) else {
                    continue;
                };
                let (dx, dy) = (ax - bx, ay - by);
                let far = dx.hypot(dy).max(0.01);
                if far < DOT_ROOM {
                    let move_by = (DOT_ROOM - far) * 0.5;
                    let (fx, fy) = (dx / far * move_by, dy / far * move_by);
                    if let Some(p) = push.get_mut(a) {
                        p.0 += fx;
                        p.1 += fy;
                    }
                    if let Some(p) = push.get_mut(b) {
                        p.0 -= fx;
                        p.1 -= fy;
                    }
                }
            }
        }
        for &(a, b) in &links {
            let (Some(&(ax, ay)), Some(&(bx, by))) = (at.get(a), at.get(b)) else {
                continue;
            };
            let (dx, dy) = (bx - ax, by - ay);
            let far = dx.hypot(dy).max(0.01);
            if far > DOT_ROOM {
                let move_by = (far - DOT_ROOM) * 0.15;
                let (fx, fy) = (dx / far * move_by, dy / far * move_by);
                if let Some(p) = push.get_mut(a) {
                    p.0 += fx;
                    p.1 += fy;
                }
                if let Some(p) = push.get_mut(b) {
                    p.0 -= fx;
                    p.1 -= fy;
                }
            }
        }
        for (spot, force) in at.iter_mut().zip(&push) {
            // A pull toward the middle keeps the group round and the circle small.
            spot.0 = spot.0 * 0.97 + force.0;
            spot.1 = spot.1 * 0.97 + force.1;
        }
    }
    at
}

/// Centers for circles of these radii, none touching another, with the pairs that have the most
/// links between them kept nearest. `between[a][b]` is how many links join group `a` and `b`.
#[allow(clippy::cast_precision_loss)]
fn place_groups(radius: &[f32], between: &[Vec<u32>]) -> Vec<(f32, f32)> {
    let n = radius.len();
    let mut at: Vec<(f32, f32)> = Vec::with_capacity(n);
    let mut reach = 0.0_f32;
    for (k, r) in radius.iter().enumerate() {
        let turn = k as f32 * 2.399_963;
        reach += r + GROUP_GAP * 0.5;
        at.push((reach * 0.6 * turn.cos(), reach * 0.6 * turn.sin()));
    }
    for _ in 0..300 {
        let mut push = vec![(0.0_f32, 0.0_f32); n];
        for a in 0..n {
            for b in (a + 1)..n {
                let (Some(&(ax, ay)), Some(&(bx, by))) = (at.get(a), at.get(b)) else {
                    continue;
                };
                let (dx, dy) = (bx - ax, by - ay);
                let far = dx.hypot(dy).max(0.01);
                let (ra, rb) = (
                    radius.get(a).copied().unwrap_or(0.0),
                    radius.get(b).copied().unwrap_or(0.0),
                );
                let want = ra + rb + GROUP_GAP;
                let linked = between
                    .get(a)
                    .and_then(|row| row.get(b))
                    .copied()
                    .unwrap_or(0);
                // Too close: apart. Linked and far: a little nearer, never inside `want`.
                let move_by = if far < want {
                    -(want - far) * 0.5
                } else if linked > 0 {
                    (far - want) * 0.04 * (linked.min(6) as f32)
                } else {
                    // Everyone drifts in a little, so unlinked groups do not wander off.
                    (far - want) * 0.005
                };
                let (fx, fy) = (dx / far * move_by, dy / far * move_by);
                if let Some(p) = push.get_mut(a) {
                    p.0 += fx;
                    p.1 += fy;
                }
                if let Some(p) = push.get_mut(b) {
                    p.0 -= fx;
                    p.1 -= fy;
                }
            }
        }
        for (spot, force) in at.iter_mut().zip(&push) {
            spot.0 += force.0;
            spot.1 += force.1;
        }
    }
    at
}

/// What the graph canvas draws and answers.
struct GraphView {
    data: Arc<GraphData>,
    selected: Option<String>,
}

#[derive(Default)]
struct GraphHover {
    over: Option<usize>,
}

/// Room kept clear round the graph, in pixels, so nothing is drawn to the edge.
const GRAPH_MARGIN: f32 = 56.0;

impl GraphView {
    /// How the world fits the canvas: the scale, and where the world's left and top land. The
    /// scale is the same both ways, so a circle stays round, and it is never more than 1.3 so a
    /// few notes are not blown up.
    fn fit(&self, bounds: iced::Rectangle) -> (f32, f32, f32) {
        let (left, top, right, bottom) = self.data.bounds;
        let (world_w, world_h) = ((right - left).max(1.0), (bottom - top).max(1.0));
        let room_w = (bounds.width - GRAPH_MARGIN * 2.0).max(40.0);
        let room_h = (bounds.height - GRAPH_MARGIN * 2.0).max(40.0);
        let scale = (room_w / world_w).min(room_h / world_h).min(1.3);
        let x = GRAPH_MARGIN + (room_w - world_w * scale) / 2.0 - left * scale;
        let y = GRAPH_MARGIN + (room_h - world_h * scale) / 2.0 - top * scale;
        (scale, x, y)
    }

    fn spot(&self, x: f32, y: f32, bounds: iced::Rectangle) -> iced::Point {
        let (scale, ox, oy) = self.fit(bounds);
        iced::Point::new(ox + x * scale, oy + y * scale)
    }

    /// The groups first: a wide soft circle, and the group's name at the top of it.
    fn draw_groups(
        &self,
        frame: &mut canvas::Frame,
        area: iced::Rectangle,
        scale: f32,
        ink: &Ink,
        shade_of: &dyn Fn(usize) -> Color,
    ) {
        for (index, group) in self.data.groups.iter().enumerate() {
            let color = shade_of(index);
            let middle = self.spot(group.x, group.y, area);
            let circle = canvas::Path::circle(middle, group.r * scale);
            frame.fill(&circle, Color { a: 0.13, ..color });
            frame.stroke(
                &circle,
                canvas::Stroke::default()
                    .with_width(1.2)
                    .with_color(Color { a: 0.4, ..color }),
            );
            frame.fill_text(canvas::Text {
                content: group.name.clone(),
                position: iced::Point::new(middle.x, middle.y - group.r * scale + 8.0),
                max_width: (group.r * scale * 1.6).max(60.0),
                color: Color {
                    a: 0.85,
                    ..ink.text
                },
                size: iced::Pixels(13.0),
                font: sans(),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Top,
                ..canvas::Text::default()
            });
        }
    }

    fn nearest(&self, at: iced::Point, bounds: iced::Rectangle) -> Option<usize> {
        self.data
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let spot = self.spot(node.x, node.y, bounds);
                (index, (spot.x - at.x).hypot(spot.y - at.y))
            })
            .filter(|(_, far)| *far <= 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
}

/// The color of a task that is late. The only status color the window has so far.
const OVERDUE: Color = Color {
    r: 0.86,
    g: 0.36,
    b: 0.31,
    a: 1.0,
};

const GRAPH_SHADES: [Color; 6] = [
    Color {
        r: 0.38,
        g: 0.65,
        b: 0.88,
        a: 1.0,
    },
    Color {
        r: 0.93,
        g: 0.62,
        b: 0.35,
        a: 1.0,
    },
    Color {
        r: 0.55,
        g: 0.78,
        b: 0.50,
        a: 1.0,
    },
    Color {
        r: 0.80,
        g: 0.50,
        b: 0.75,
        a: 1.0,
    },
    Color {
        r: 0.90,
        g: 0.80,
        b: 0.40,
        a: 1.0,
    },
    Color {
        r: 0.50,
        g: 0.75,
        b: 0.75,
        a: 1.0,
    },
];

impl canvas::Program<Message> for GraphView {
    type State = GraphHover;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let over = cursor
            .position_in(bounds)
            .and_then(|at| self.nearest(at, bounds));
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                let node = self.data.nodes.get(over?)?;
                Some(canvas::Action::publish(Message::GraphPick(node.id.clone())).and_capture())
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) if over != state.over => {
                state.over = over;
                Some(canvas::Action::request_redraw())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let ink = ink_of(theme);
        let area = iced::Rectangle::new(iced::Point::ORIGIN, bounds.size());
        let (scale, _, _) = self.fit(area);
        let shade_of = |group: usize| {
            self.data
                .groups
                .get(group)
                .and_then(|group| group.shade)
                .and_then(|shade| GRAPH_SHADES.get(shade % GRAPH_SHADES.len()).copied())
                .unwrap_or(ink.muted)
        };
        self.draw_groups(&mut frame, area, scale, &ink, &shade_of);
        let mine = self
            .selected
            .as_ref()
            .and_then(|id| self.data.nodes.iter().position(|node| node.id == *id));
        // Lines over the circles. A link from one group to another is the one to see.
        for &(from, to) in &self.data.edges {
            let (Some(a), Some(b)) = (self.data.nodes.get(from), self.data.nodes.get(to)) else {
                continue;
            };
            let across = a.group != b.group;
            let touches = mine == Some(from)
                || mine == Some(to)
                || state.over == Some(from)
                || state.over == Some(to);
            let line = canvas::Path::line(self.spot(a.x, a.y, area), self.spot(b.x, b.y, area));
            let (width, alpha) = match (across, touches) {
                (_, true) => (2.2, 0.95),
                (true, false) => (1.6, 0.6),
                (false, false) => (1.0, 0.3),
            };
            frame.stroke(
                &line,
                canvas::Stroke::default()
                    .with_width(width)
                    .with_color(Color {
                        a: alpha,
                        ..ink.text
                    }),
            );
        }
        let few = self.data.nodes.len() <= 30;
        for (index, node) in self.data.nodes.iter().enumerate() {
            let spot = self.spot(node.x, node.y, area);
            let color = shade_of(node.group);
            let chosen = mine == Some(index);
            let radius = if chosen {
                7.0
            } else if state.over == Some(index) {
                6.0
            } else {
                5.0
            };
            frame.fill(&canvas::Path::circle(spot, radius), color);
            frame.stroke(
                &canvas::Path::circle(spot, radius),
                canvas::Stroke::default()
                    .with_width(1.0)
                    .with_color(Color { a: 0.7, ..ink.text }),
            );
            if chosen {
                frame.stroke(
                    &canvas::Path::circle(spot, radius + 3.0),
                    canvas::Stroke::default()
                        .with_width(1.5)
                        .with_color(ink.text),
                );
            }
            if chosen || state.over == Some(index) || few {
                // A long title breaks onto lines at this width instead of running on.
                frame.fill_text(canvas::Text {
                    content: node.title.clone(),
                    position: iced::Point::new(spot.x + radius + 5.0, spot.y),
                    max_width: 110.0,
                    color: ink.text,
                    size: iced::Pixels(12.0),
                    font: sans(),
                    align_y: iced::alignment::Vertical::Center,
                    ..canvas::Text::default()
                });
            }
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if state.over.is_some() {
            iced::mouse::Interaction::Pointer
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

/// The handle on a note in a group. Pressing it starts moving the note in that group.
fn grip(handle: iced_image::Handle, group: &str, id: &str) -> Element<'static, Message> {
    let face =
        mouse_area(container(iced_image::Image::new(handle).width(16).height(16)).padding(6))
            .on_press(Message::DragStart {
                group: group.to_owned(),
                id: id.to_owned(),
            })
            .interaction(iced::mouse::Interaction::Grab);
    hover_label(face, "Drag to move it in this group")
}

/// A line in the accent color, drawn where a note being moved would land.
fn drop_bar() -> Element<'static, Message> {
    container(column![])
        .height(3)
        .width(Fill)
        .style(|theme| {
            let ink = ink_of(theme);
            container::Style {
                background: Some(iced::Background::Color(ink.accent)),
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn is_tasks_note(note: &NoteSummary) -> bool {
    note.title == "Tasks" && !note.conflict
}

fn card_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 12.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
        ..Default::default()
    }
}

fn section_card(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 14.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn field_well(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.field)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 10.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn settings_card<'a>(title: &'static str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(
        column![text(title).font(serif()).size(22), body]
            .spacing(14)
            .width(Fill),
    )
    .padding(20)
    .width(Fill)
    .style(section_card)
    .into()
}

fn hover_label(
    face: impl Into<Element<'static, Message>>,
    label: &'static str,
) -> Element<'static, Message> {
    tooltip(
        face,
        container(text(label).size(12))
            .padding([4, 8])
            .style(card_style),
        tooltip::Position::Top,
    )
    .gap(6)
    .into()
}

/// One layer over the window, dimmed, with `content` on it: a card in the middle, or a sheet at
/// an edge. A press on the dim part sends `close`. A press on the content is the content's, and
/// is not taken as a press outside it. It is one layer and not two, so that nothing above the
/// dim part can take the press away from it.
fn dim_layer(
    content: Element<'_, Message>,
    close: Message,
    align: iced::alignment::Horizontal,
    pad: f32,
) -> Element<'_, Message> {
    mouse_area(
        container(mouse_area(content).on_press(Message::FieldDone))
            .width(Fill)
            .height(Fill)
            .align_x(align)
            .align_y(iced::alignment::Vertical::Center)
            .padding(pad)
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.38,
                ))),
                ..Default::default()
            }),
    )
    .on_press(close)
    .into()
}

fn bubble_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 10.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn mine_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.accent)),
        text_color: Some(ink.on_accent),
        border: iced::Border {
            radius: 14.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn theirs_style(theme: &Theme) -> container::Style {
    let mut style = bubble_style(theme);
    style.border.radius = 14.0.into();
    style
}

struct ChatBubble {
    text: String,
    time: String,
    day: String,
    mine: bool,
    sender: String,
    cite: Option<String>,
}

fn file_chip(
    name: &str,
    path: Option<PathBuf>,
    video: bool,
    mine: bool,
) -> Element<'static, Message> {
    let label = if video {
        format!("Video · {name}")
    } else {
        name.to_owned()
    };
    if let Some(path) = path {
        button(text(format!("Open {label}")).size(14))
            .padding([8.0, 14.0])
            .on_press(Message::OpenPath(path))
            .style(move |theme, status| file_chip_style(theme, status, mine))
            .into()
    } else {
        text(format!("{label} · not on this computer"))
            .size(13)
            .style(move |theme: &Theme| {
                let ink = ink_of(theme);
                iced::widget::text::Style {
                    color: Some(if mine { ink.on_accent } else { ink.text }),
                }
            })
            .into()
    }
}

fn file_chip_style(theme: &Theme, status: button::Status, mine: bool) -> button::Style {
    let ink = ink_of(theme);
    if !mine {
        return ghost_style(theme, status);
    }
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Some(iced::Background::Color(Color {
            a: 0.16,
            ..ink.on_accent
        })),
        _ => Some(iced::Background::Color(Color {
            a: 0.08,
            ..ink.on_accent
        })),
    };
    button::Style {
        background,
        text_color: ink.on_accent,
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: Color {
                a: 0.45,
                ..ink.on_accent
            },
        },
        ..Default::default()
    }
}

fn format_button(
    label: &'static str,
    hint_text: &'static str,
    kind: FormatKind,
) -> Element<'static, Message> {
    let face = button(text(label).size(13))
        .padding([6.0, 10.0])
        .on_press(Message::Format(kind))
        .style(ghost_style);
    hover_label(face, hint_text)
}

fn pill_button(
    label: &'static str,
    hint_text: &'static str,
    message: Message,
) -> Element<'static, Message> {
    let face = button(text(label).size(13))
        .padding([4.0, 8.0])
        .on_press(message)
        .style(ghost_style);
    tooltip(
        face,
        container(text(hint_text).size(12))
            .padding([4, 8])
            .style(card_style),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

fn pill_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        text_color: Some(ink.text),
        background: Some(iced::Background::Color(ink.surface)),
        border: iced::Border {
            radius: 16.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn clean_tag(raw: &str) -> Option<String> {
    let tag = raw.trim().trim_start_matches('#').trim();
    if tag.is_empty() || tag.len() > 60 || tag.contains(['%', ',', '\n', '|']) {
        return None;
    }
    // `Novel / Part 1` and `Novel/Part 1` are the same group path.
    let parts = tag_path(tag);
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

fn format_caption(label: &'static str) -> Element<'static, Message> {
    text(label).size(11).into()
}

fn chat_words(words: String) -> Element<'static, Message> {
    text(words)
        .size(15)
        .wrapping(iced::advanced::text::Wrapping::WordOrGlyph)
        .into()
}

fn push_tasks(rows: &mut Vec<TaskRow>, id: &str, title: &str, body: &str) {
    for task in markdown::task_lines(body) {
        rows.push(TaskRow {
            note_id: id.to_owned(),
            note_title: title.to_owned(),
            line: task.line,
            done: task.done,
            due: due::Due::of(&markdown::without_cites(&task.text)),
            text: task.text,
        });
    }
}

fn slim_bar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(4.0)
        .scroller_width(3.0)
        .margin(2.0)
}

/// Pixels a wheel notch moves a page of this height. Iced moves a fixed 60, which is slow in a
/// tall window. This is the step GTK takes, the page height to the power of two thirds, kept
/// between 60 and 150 so a small window is not left slower than before and a large one is not
/// jumpy. A touchpad sends pixels, already scaled by the system, and is left alone.
fn wheel_step(page: f32) -> f32 {
    page.max(1.0).powf(2.0 / 3.0).clamp(60.0, 150.0)
}

/// Stands over the whole window and scales a wheel notch before any scroll area sees it.
struct Wheel<'a> {
    child: Element<'a, Message>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for Wheel<'_> {
    fn size(&self) -> Size<iced::Length> {
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
        let scaled = match event {
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                delta: iced::mouse::ScrollDelta::Lines { x, y },
            }) => {
                // Iced multiplies a line by 60, so this is the factor that makes it the step.
                let by = wheel_step(layout.bounds().height) / 60.0;
                Some(iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                    delta: iced::mouse::ScrollDelta::Lines {
                        x: x * by,
                        y: y * by,
                    },
                }))
            }
            _ => None,
        };
        self.child.as_widget_mut().update(
            child,
            scaled.as_ref().unwrap_or(event),
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
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

    /// What floats over the window belongs to something inside it, and has to be passed up. It
    /// was not, and so no menu under a button and no hover label was ever drawn.
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

impl<'a> From<Wheel<'a>> for Element<'a, Message> {
    fn from(wheel: Wheel<'a>) -> Self {
        Element::new(wheel)
    }
}

/// Room between content and the bar. The bar is drawn over the content, so without this it sits
/// on the edge of a button or a line of text.
const BAR_ROOM: f32 = 10.0;

fn vscroll<'a>(content: impl Into<Element<'a, Message>>) -> scrollable::Scrollable<'a, Message> {
    vscroll_flush(container(content).width(Fill).padding(iced::Padding {
        top: 0.0,
        right: BAR_ROOM,
        bottom: 0.0,
        left: 0.0,
    }))
}

/// A vertical scroll with no room kept for the bar, for a page that has made its own.
fn vscroll_flush<'a>(
    content: impl Into<Element<'a, Message>>,
) -> scrollable::Scrollable<'a, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Vertical(slim_bar()))
        .style(scroll_style)
}

fn hscroll<'a>(content: impl Into<Element<'a, Message>>) -> scrollable::Scrollable<'a, Message> {
    scrollable(container(content).padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: BAR_ROOM,
        left: 0.0,
    }))
    .direction(scrollable::Direction::Horizontal(slim_bar()))
    .style(scroll_style)
}

/// How long a scrollbar stays visible after the last scroll.
const SCROLL_SHOWN: Duration = Duration::from_millis(700);

/// Milliseconds since the start of the process at the last scroll, or `u64::MAX` for never.
static LAST_SCROLL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

fn since_start() -> Duration {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

/// Whether a wake to hide the scrollbar is already waiting. A burst of wheel ticks asks for one.
static SCROLL_WAKE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A wheel tick. It costs one atomic store, so scrolling never goes through `update` or rebuilds
/// the view. Only the first tick of a burst sends a message, to start the wake that hides the bar.
fn scroll_seen() -> Option<Message> {
    note_scroll();
    if SCROLL_WAKE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        None
    } else {
        Some(Message::Scrolled)
    }
}

fn note_scroll() {
    let at = u64::try_from(since_start().as_millis()).unwrap_or(u64::MAX - 1);
    LAST_SCROLL.store(at, std::sync::atomic::Ordering::Relaxed);
}

fn scrolled_recently() -> bool {
    let last = LAST_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let now = u64::try_from(since_start().as_millis()).unwrap_or(u64::MAX - 1);
    last != u64::MAX
        && now.saturating_sub(last) < u64::try_from(SCROLL_SHOWN.as_millis()).unwrap_or(0)
}

/// A scrollbar is not drawn. It shows faintly for a moment after the pane under the pointer is
/// scrolled, and clearly while the pointer is on the bar itself, so it can still be dragged.
fn scroll_style(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    let ink = ink_of(theme);
    let thumb = Color {
        a: thumb_alpha(status, scrolled_recently()),
        ..ink.muted
    };
    let rail = scrollable::Rail {
        background: None,
        border: iced::Border {
            radius: 2.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        scroller: scrollable::Scroller {
            background: iced::Background::Color(thumb),
            border: iced::Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
    };
    style.vertical_rail = rail;
    style.horizontal_rail = rail;
    style.gap = None;
    style
}

/// Opacity of the thumb. `recent` is whether the last scroll was a moment ago.
fn thumb_alpha(status: scrollable::Status, recent: bool) -> f32 {
    if scroll_hot(status) {
        0.62
    } else if recent
        && matches!(
            status,
            scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
        )
    {
        0.32
    } else {
        0.0
    }
}

fn scroll_hot(status: scrollable::Status) -> bool {
    matches!(
        status,
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged: true,
            ..
        } | scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        }
    )
}

fn hairline() -> Element<'static, Message> {
    container(column![])
        .height(1)
        .width(Fill)
        .style(|theme| {
            let ink = ink_of(theme);
            container::Style {
                background: Some(iced::Background::Color(ink.hairline)),
                ..Default::default()
            }
        })
        .into()
}

fn hairline_v() -> Element<'static, Message> {
    container(column![])
        .width(1)
        .height(Fill)
        .style(|theme| {
            let ink = ink_of(theme);
            container::Style {
                background: Some(iced::Background::Color(ink.hairline)),
                ..Default::default()
            }
        })
        .into()
}

fn action_label(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14))
        .padding([10.0, 16.0])
        .on_press(message)
        .style(action_style)
        .into()
}

fn danger_label(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([10.0, 16.0])
        .on_press(message)
        .style(danger_style)
        .into()
}

fn danger_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let danger = Color::from_rgb8(184, 78, 62);
    let background = match status {
        button::Status::Hovered => mix(danger, Color::WHITE, 0.08),
        button::Status::Pressed => mix(danger, Color::BLACK, 0.16),
        button::Status::Disabled => mix(danger, ink.bg, 0.45),
        button::Status::Active => danger,
    };
    button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: Color::from_rgb8(0xED, 0xEA, 0xDE),
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn action_fill(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14).width(Fill))
        .width(Fill)
        .padding([8.0, 14.0])
        .on_press(message)
        .style(action_style)
        .into()
}

fn ghost_label(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14))
        .padding([8.0, 14.0])
        .on_press(message)
        .style(ghost_style)
        .into()
}

fn ghost_label_owned(label: String, message: Message) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([8.0, 14.0])
        .on_press(message)
        .style(ghost_style)
        .into()
}

fn segment(label: &str, active: bool, message: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(14).width(Fill))
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(message)
        .style(segment_style(active))
        .into()
}

fn chat_row(
    label: &str,
    who: &str,
    preview: &str,
    time: &str,
    id: &str,
    selected: bool,
    fresh: bool,
) -> Element<'static, Message> {
    let label = label.to_owned();
    let who = who.to_owned();
    let preview = preview.to_owned();
    let time = time.to_owned();
    let id = id.to_owned();
    let mut head = row![text(label).size(15).width(Fill)].align_y(iced::Alignment::Center);
    if fresh {
        head = head.push(text("New").size(12).style(fresh_style));
    }
    if !time.is_empty() {
        head = head.push(text(time).size(12).style(muted_style));
    }
    let mut lines = column![head].spacing(2).width(Fill);
    if !who.is_empty() {
        lines = lines.push(text(who).size(12).style(muted_style));
    }
    if !preview.is_empty() {
        lines = lines.push(text(preview).size(12).style(muted_style));
    }
    button(lines)
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(Message::SelectChat(id))
        .style(list_row(selected))
        .into()
}

fn clip_chars(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        text
    } else {
        text.chars().take(max).collect()
    }
}

/// An initial and a name for someone else in a group. The color comes from their key.
fn speaker(name: &str, key: &str) -> Element<'static, Message> {
    let letter = name
        .chars()
        .find(|glyph| glyph.is_alphanumeric())
        .map_or_else(|| "?".to_owned(), |glyph| glyph.to_uppercase().to_string());
    let color = face_color(key);
    let mark = container(text(letter).size(11).color(Color::WHITE))
        .width(22)
        .height(22)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(color)),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                radius: 11.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        });
    row![mark, text(name.to_owned()).size(12).style(muted_style)]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
}

fn face_color(key: &str) -> Color {
    const PALETTE: [[u8; 3]; 8] = [
        [184, 84, 72],
        [196, 132, 58],
        [58, 138, 96],
        [58, 116, 168],
        [124, 90, 176],
        [168, 72, 116],
        [62, 132, 146],
        [138, 106, 64],
    ];
    let n = key
        .bytes()
        .fold(0_usize, |acc, byte| acc.wrapping_add(usize::from(byte)));
    let [r, g, b] = PALETTE
        .get(n % PALETTE.len())
        .copied()
        .unwrap_or([184, 84, 72]);
    Color::from_rgb8(r, g, b)
}

fn guide_line(title: &'static str, body: &'static str) -> Element<'static, Message> {
    column![
        text(title).font(serif()).size(22),
        text(body).size(14).style(muted_style),
    ]
    .spacing(4)
    .into()
}

fn chat_preview(text: &str) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut out: String = line.chars().take(72).collect();
    if line.chars().count() > 72 {
        out.push_str("...");
    }
    out
}

fn edit_burst(action: &text_editor::Action) -> Option<Burst> {
    match action {
        text_editor::Action::Edit(text_editor::Edit::Insert(_)) => Some(Burst::Type),
        text_editor::Action::Edit(text_editor::Edit::Backspace | text_editor::Edit::Delete) => {
            Some(Burst::Erase)
        }
        text_editor::Action::Edit(
            text_editor::Edit::Paste(_)
            | text_editor::Edit::Enter
            | text_editor::Edit::Indent
            | text_editor::Edit::Unindent,
        ) => Some(Burst::Break),
        _ => None,
    }
}

fn fresh_style(theme: &Theme) -> iced::widget::text::Style {
    let ink = ink_of(theme);
    iced::widget::text::Style {
        color: Some(ink.accent),
    }
}

fn muted_style(theme: &Theme) -> iced::widget::text::Style {
    let ink = ink_of(theme);
    iced::widget::text::Style {
        color: Some(ink.muted),
    }
}

fn copy_style(theme: &Theme, status: button::Status, mine: bool) -> button::Style {
    let ink = ink_of(theme);
    let text_color = if mine {
        Color {
            a: 0.85,
            ..ink.on_accent
        }
    } else {
        ink.muted
    };
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Some(iced::Background::Color(Color {
            a: 0.12,
            ..if mine { ink.on_accent } else { ink.text }
        })),
        _ => None,
    };
    button::Style {
        background,
        text_color,
        border: iced::Border {
            radius: 6.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn icon_button(handle: iced_image::Handle, message: Message) -> Element<'static, Message> {
    button(iced_image::Image::new(handle).width(22).height(22))
        .padding(6)
        .on_press(message)
        .style(icon_style)
        .into()
}

/// The height of the formatting bar, for sliding it. Close enough that it does not jump.
const FORMAT_BAR: f32 = 32.0;

/// Room kept above the writing in focus, for the small title and the buttons beside it.
const FOCUS_TOP: f32 = 34.0;

/// How long a panel takes to slide in or out.
const SLIDE: Duration = Duration::from_millis(220);

/// Something that slides between shown and hidden. It only moves for a moment after it is set,
/// and the window only redraws on a timer while it does.
struct Slide {
    open: bool,
    since: Instant,
}

impl Slide {
    /// Already in place, with nothing moving.
    fn settled(open: bool) -> Self {
        Self {
            open,
            since: Instant::now()
                .checked_sub(SLIDE.saturating_mul(2))
                .unwrap_or_else(Instant::now),
        }
    }

    fn set(&mut self, open: bool) {
        if self.open != open {
            self.open = open;
            self.since = Instant::now();
        }
    }

    /// Put it in place at once, with no slide.
    fn jump(&mut self, open: bool) {
        *self = Self::settled(open);
    }

    fn moving(&self) -> bool {
        self.since.elapsed() < SLIDE
    }

    /// 0 when hidden and 1 when shown, easing out.
    fn shown(&self) -> f32 {
        let t = (self.since.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        if self.open { eased } else { 1.0 - eased }
    }
}

/// A size a person can read: `812 bytes`, `340 KB`, `3.4 MB`, `1.2 GB`.
fn size_label(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    let tenths = |unit: u64| bytes.saturating_mul(10) / unit;
    if bytes < KB {
        format!("{bytes} bytes")
    } else if bytes < MB {
        format!("{} KB", bytes / KB)
    } else if bytes < GB {
        format!("{}.{} MB", tenths(MB) / 10, tenths(MB) % 10)
    } else {
        format!("{}.{} GB", tenths(GB) / 10, tenths(GB) % 10)
    }
}

/// A key the editor did not take. It is put into the page here, so it is not lost.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Typed {
    Text(String),
    Enter,
    Backspace,
    Delete,
}

/// What a key that no widget handled should type, if anything.
///
/// The live editor is a widget whose focus lives in its place in the tree. When it moves, as it
/// does after Enter or when a space wraps a word onto another row, the new one starts without
/// focus, and the app asks for focus again a moment later. A key pressed in that moment is
/// ignored by the editor and is gone. The window sees every key an editor ignored, so it can
/// type them. A key the editor did take is captured, and is not seen here, so nothing is typed
/// twice. A key with Ctrl, Cmd, Alt, or the Super key is a shortcut and is left alone.
fn ignored_key(
    key: &Key,
    text: Option<&str>,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Typed> {
    use iced::keyboard::key::Named;
    if modifiers.control() || modifiers.command() || modifiers.alt() || modifiers.logo() {
        return None;
    }
    match key.as_ref() {
        Key::Named(Named::Enter) => Some(Typed::Enter),
        Key::Named(Named::Backspace) => Some(Typed::Backspace),
        Key::Named(Named::Delete) => Some(Typed::Delete),
        Key::Named(Named::Space) => Some(Typed::Text(" ".to_owned())),
        _ => {
            let typed: String = text?.chars().filter(|ch| !ch.is_control()).collect();
            (!typed.is_empty()).then_some(Typed::Text(typed))
        }
    }
}

fn focus_named(name: &'static str) -> Task<Message> {
    iced::widget::operation::focus(iced::widget::Id::new(name))
}

#[derive(Clone, Copy)]
enum Mini {
    Search,
    Pencil,
    Trash,
    Up,
    Down,
    Focus,
    Gear,
    Graph,
    Grip,
    Plus,
    Quote,
    Send,
}

fn mini_button(
    handle: iced_image::Handle,
    hint_text: &'static str,
    message: Message,
) -> Element<'static, Message> {
    let face = button(iced_image::Image::new(handle).width(16).height(16))
        .padding(6)
        .on_press(message)
        .style(icon_style);
    hover_label(face, hint_text)
}

fn mini_handle(kind: Mini, dark: bool) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 24]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        let ink = [0xED, 0xEA, 0xDE];
        let day = [0x1C, 0x19, 0x17];
        [
            iced_image::Handle::from_bytes(icon_png(ink, search_ink)),
            iced_image::Handle::from_bytes(icon_png(day, search_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, pencil_ink)),
            iced_image::Handle::from_bytes(icon_png(day, pencil_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, trash_ink)),
            iced_image::Handle::from_bytes(icon_png(day, trash_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, up_ink)),
            iced_image::Handle::from_bytes(icon_png(day, up_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, down_ink)),
            iced_image::Handle::from_bytes(icon_png(day, down_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, focus_ink)),
            iced_image::Handle::from_bytes(icon_png(day, focus_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, gear_ink)),
            iced_image::Handle::from_bytes(icon_png(day, gear_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, graph_ink)),
            iced_image::Handle::from_bytes(icon_png(day, graph_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, grip_ink)),
            iced_image::Handle::from_bytes(icon_png(day, grip_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, plus_ink)),
            iced_image::Handle::from_bytes(icon_png(day, plus_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, quote_ink)),
            iced_image::Handle::from_bytes(icon_png(day, quote_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, send_ink)),
            iced_image::Handle::from_bytes(icon_png(day, send_ink)),
        ]
    });
    let index = match kind {
        Mini::Search => 0,
        Mini::Pencil => 2,
        Mini::Trash => 4,
        Mini::Up => 6,
        Mini::Down => 8,
        Mini::Focus => 10,
        Mini::Gear => 12,
        Mini::Graph => 14,
        Mini::Grip => 16,
        Mini::Plus => 18,
        Mini::Quote => 20,
        Mini::Send => 22,
    } + usize::from(!dark);
    handles
        .get(index)
        .cloned()
        .unwrap_or_else(|| iced_image::Handle::from_bytes(Vec::new()))
}

fn icon_png(color: [u8; 3], ink: fn(f64, f64) -> bool) -> Vec<u8> {
    let size = 64_u32;
    let mut pixels = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = f64::from(x) - 31.5;
            let dy = f64::from(y) - 31.5;
            if !ink(dx, dy) {
                continue;
            }
            let index = ((y * size + x) * 4) as usize;
            if let Some(pixel) = pixels.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    encode_png(&pixels, size)
}

fn search_ink(dx: f64, dy: f64) -> bool {
    let ox = dx + 6.0;
    let oy = dy + 6.0;
    let radius = ox.hypot(oy);
    if (11.0..15.0).contains(&radius) {
        return true;
    }
    dx > 4.0 && dy > 4.0 && dx < 20.0 && dy < 20.0 && (dx - dy).abs() < 2.8 && radius > 14.0
}

fn pencil_ink(dx: f64, dy: f64) -> bool {
    let u = (dx + dy) * std::f64::consts::FRAC_1_SQRT_2;
    let v = (dy - dx) * std::f64::consts::FRAC_1_SQRT_2;
    if (-16.0..8.0).contains(&u) && v.abs() < 3.0 {
        return true;
    }
    (8.0..16.0).contains(&u) && v.abs() < (16.0 - u) * 0.55
}

fn trash_ink(dx: f64, dy: f64) -> bool {
    let x = dx.abs();
    let handle = (-21.0..-16.5).contains(&dy) && x < 5.0 && (dy > -18.2 || x > 3.2);
    let lid = (-16.5..-13.0).contains(&dy) && x < 16.0;
    let side = (-12.0..15.0).contains(&dy) && (x - 11.0).abs() < 1.7;
    let bottom = (13.0..16.5).contains(&dy) && x < 12.0;
    handle || lid || side || bottom
}

/// Four corner brackets around a dot: a viewfinder, for writing with nothing else in view.
fn focus_ink(dx: f64, dy: f64) -> bool {
    let x = dx.abs();
    let y = dy.abs();
    let upright = (16.0..20.5).contains(&x) && (8.0..=20.5).contains(&y);
    let across = (16.0..20.5).contains(&y) && (8.0..=20.5).contains(&x);
    upright || across || x.hypot(y) < 4.5
}

/// A cross: add.
fn plus_ink(dx: f64, dy: f64) -> bool {
    (dx.abs() < 3.2 && dy.abs() < 18.0) || (dy.abs() < 3.2 && dx.abs() < 18.0)
}

/// A pair of square brackets round a dot: a reference to another note.
fn quote_ink(dx: f64, dy: f64) -> bool {
    let x = dx.abs();
    let y = dy.abs();
    let stem = (12.0..16.5).contains(&x) && y < 19.0;
    let serif = (12.0..21.0).contains(&x) && (15.0..19.0).contains(&y);
    stem || serif || dx.hypot(dy) < 3.8
}

/// A paper plane pointing right: send.
fn send_ink(dx: f64, dy: f64) -> bool {
    // A dart: tip at the right, a notch in the middle of the back. Even-odd test on four points.
    let points = [(-19.0, -16.0), (21.0, 0.0), (-19.0, 16.0), (-9.0, 0.0)];
    let mut inside = false;
    // Each edge runs from the point before it, round to the first.
    for (edge, (px, py)) in points.iter().enumerate() {
        let (qx, qy) = points
            .get(edge.checked_sub(1).unwrap_or(points.len() - 1))
            .copied()
            .unwrap_or((*px, *py));
        if (*py > dy) != (qy > dy) && dx < (qx - px) * (dy - py) / (qy - py) + px {
            inside = !inside;
        }
    }
    inside
}

/// Two columns of three dots: something to take hold of.
fn grip_ink(dx: f64, dy: f64) -> bool {
    [-8.0, 8.0].iter().any(|x: &f64| {
        [-14.0, 0.0, 14.0]
            .iter()
            .any(|y: &f64| (dx - x).hypot(dy - y) < 4.6)
    })
}

/// Three dots joined by lines: notes and links.
fn graph_ink(dx: f64, dy: f64) -> bool {
    let dots = [(-14.0, 12.0), (14.0, 12.0), (0.0, -14.0)];
    if dots
        .iter()
        .any(|(x, y): &(f64, f64)| (dx - x).hypot(dy - y) < 6.5)
    {
        return true;
    }
    // A line between each pair: close to the segment.
    let near = |a: (f64, f64), b: (f64, f64)| {
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let along = (((dx - a.0) * vx + (dy - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
        (dx - (a.0 + vx * along)).hypot(dy - (a.1 + vy * along)) < 2.2
    };
    near(dots[0], dots[1]) || near(dots[1], dots[2]) || near(dots[0], dots[2])
}

/// A ring with eight teeth and a hole: settings.
fn gear_ink(dx: f64, dy: f64) -> bool {
    let radius = dx.hypot(dy);
    if !(6.5..=19.5).contains(&radius) {
        return false;
    }
    let tooth = (dy.atan2(dx) * 8.0).cos() > -0.1;
    radius <= 14.5 || (tooth && radius <= 19.5)
}

fn up_ink(dx: f64, dy: f64) -> bool {
    chevron_ink(true, dx, dy)
}

fn down_ink(dx: f64, dy: f64) -> bool {
    chevron_ink(false, dx, dy)
}

fn chevron_ink(up: bool, dx: f64, dy: f64) -> bool {
    let y = if up { dy + 2.0 } else { -dy + 2.0 };
    let x = dx.abs();
    let target = (y + 12.0) * (14.0 / 18.0);
    (-12.0..8.0).contains(&y) && (x - target).abs() < 2.4 && x < 16.0
}

fn gear_handle(dark: bool) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 2]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(gear_png([0x66, 0xA5, 0xE1])),
            iced_image::Handle::from_bytes(gear_png([0x16, 0x44, 0x6F])),
        ]
    });
    if dark {
        handles[0].clone()
    } else {
        handles[1].clone()
    }
}

fn gear_png(color: [u8; 3]) -> Vec<u8> {
    let Ok(icon) = image::load_from_memory(include_bytes!("../icons/settings.png")) else {
        return Vec::new();
    };
    let icon = icon.to_rgba8();
    let (width, height) = icon.dimensions();
    let mut pixels = icon.into_raw();
    for chunk in pixels.chunks_exact_mut(4) {
        let Ok(pixel) = <&mut [u8; 4]>::try_from(chunk) else {
            continue;
        };
        if pixel[3] == 0 {
            continue;
        }
        pixel[0] = color[0];
        pixel[1] = color[1];
        pixel[2] = color[2];
    }
    let mut buf = Vec::new();
    let encoder = ::image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    let _ = encoder.write_image(&pixels, width, height, ::image::ExtendedColorType::Rgba8);
    buf
}

fn mode_handle(dark: bool) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 2]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(mode_png(true, [0x66, 0xA5, 0xE1])),
            iced_image::Handle::from_bytes(mode_png(false, [0x16, 0x44, 0x6F])),
        ]
    });
    if dark {
        handles[0].clone()
    } else {
        handles[1].clone()
    }
}

fn mode_png(sun: bool, color: [u8; 3]) -> Vec<u8> {
    let size = 64_u32;
    let mut pixels = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = f64::from(x) - 31.5;
            let dy = f64::from(y) - 31.5;
            if !mode_ink(sun, dx, dy) {
                continue;
            }
            let index = ((y * size + x) * 4) as usize;
            if let Some(pixel) = pixels.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    encode_png(&pixels, size)
}

fn mode_ink(sun: bool, dx: f64, dy: f64) -> bool {
    let radius = dx.hypot(dy);
    if sun {
        if radius < 9.5 {
            return true;
        }
        if !(13.5..22.0).contains(&radius) {
            return false;
        }
        let angle = dy.atan2(dx);
        let step = std::f64::consts::FRAC_PI_4;
        let nearest = (angle / step).round() * step;
        (angle - nearest).abs() < 0.18
    } else {
        let bite = (dx - 9.0).hypot(dy - 1.0);
        radius < 16.0 && bite > 13.5
    }
}

fn encode_png(pixels: &[u8], size: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let encoder = ::image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    let _ = encoder.write_image(pixels, size, size, ::image::ExtendedColorType::Rgba8);
    buf
}

fn globe_handle(color: [u8; 3]) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 3]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(globe_png([140, 133, 122])),
            iced_image::Handle::from_bytes(globe_png([184, 78, 62])),
            iced_image::Handle::from_bytes(globe_png([46, 140, 96])),
        ]
    });
    if color == [184, 78, 62] {
        handles[1].clone()
    } else if color == [46, 140, 96] {
        handles[2].clone()
    } else {
        handles[0].clone()
    }
}

fn globe_png(color: [u8; 3]) -> Vec<u8> {
    let size = 64_u32;
    let mut pixels = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = f64::from(x) - 31.5;
            let dy = f64::from(y) - 31.5;
            let radius = dx.hypot(dy);
            if radius > 28.0 {
                continue;
            }
            let ring = radius > 25.5;
            let equator = dy.abs() < 1.3;
            let meridian = dx.abs() < 1.3;
            let ellipse = ((dx / 16.0).powi(2) + (dy / 28.0).powi(2) - 1.0).abs() < 0.12;
            let ink = ring || equator || meridian || ellipse;
            let alpha = if ink { 255 } else { 48 };
            let index = ((y * size + x) * 4) as usize;
            if let Some(pixel) = pixels.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&[color[0], color[1], color[2], alpha]);
            }
        }
    }
    let mut buf = Vec::new();
    let encoder = ::image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    let _ = encoder.write_image(&pixels, size, size, ::image::ExtendedColorType::Rgba8);
    buf
}

fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let name = dir
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or("notebook");
    match dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(format!("{name}{suffix}")),
        _ => PathBuf::from(format!("{name}{suffix}")),
    }
}

fn open_task(dir: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = (|| {
                    let _notes = notes_lock();
                    let _chat = chat_lock();
                    let book = Notebook::open(&dir).map_err(|err| err.to_string())?;
                    let files = lane::files_of(&book);
                    lane::park_side(lane::compute(&files, &lane::OpenSnap::default()));
                    lane::park_inbox(lane::inbox_of(&dir));
                    park_notebook(book);
                    Ok(())
                })();
                let _ = sender.send(result);
            });
            receiver
                .await
                .unwrap_or_else(|_| Err("could not open this folder".to_owned()))
        },
        Message::Opened,
    )
}

fn parked_notebook() -> &'static Mutex<Option<Notebook>> {
    static PARK: Mutex<Option<Notebook>> = Mutex::new(None);
    &PARK
}

fn park_notebook(book: Notebook) {
    *parked_notebook()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(book);
}

fn take_notebook() -> Option<Notebook> {
    parked_notebook()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

fn save_task(
    dir: PathBuf,
    id: Option<String>,
    title: String,
    body: String,
    token: u64,
) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = write_snapshot(&dir, id.as_deref(), &title, &body);
                let _ = sender.send((result, title, body));
            });
            let (result, title, body) = receiver.await.unwrap_or_else(|_| {
                (
                    Err("Save stopped before it finished.".to_owned()),
                    String::new(),
                    String::new(),
                )
            });
            (token, result, title, body)
        },
        |(token, result, title, body)| Message::Saved(token, result, title, body),
    )
}

fn write_snapshot(
    dir: &Path,
    id: Option<&str>,
    title: &str,
    body: &str,
) -> Result<(String, Vec<NoteSummary>), String> {
    let _notes = notes_lock();
    let book = Notebook::open(dir).map_err(|err| err.to_string())?;
    let saved = match id {
        Some(id) => {
            book.edit(id, title, body).map_err(|err| err.to_string())?;
            id.to_owned()
        }
        None => book.add(title, body).map_err(|err| err.to_string())?,
    };
    let notes = book.list().map_err(|err| err.to_string())?;
    lane::publish_saved(book);
    Ok((saved, notes))
}

fn write_chapter(
    dir: &Path,
    current: Option<&(Option<String>, String, String)>,
    title: &str,
) -> Result<MadeChapter, String> {
    let _notes = notes_lock();
    let book = Notebook::open(dir).map_err(|err| err.to_string())?;
    let saved = if let Some((id, note_title, note_body)) = current {
        let id = match id {
            Some(id) => {
                book.edit(id, note_title, note_body)
                    .map_err(|err| err.to_string())?;
                id.clone()
            }
            None => book
                .add(note_title, note_body)
                .map_err(|err| err.to_string())?,
        };
        Some(SavedDraft {
            id,
            title: note_title.clone(),
            body: note_body.clone(),
        })
    } else {
        None
    };
    let id = book.add(title, "").map_err(|err| err.to_string())?;
    let notes = book.list().map_err(|err| err.to_string())?;
    lane::publish_saved(book);
    Ok(MadeChapter { notes, id, saved })
}

fn choice_button(label: &str, active: bool, message: Message) -> Element<'_, Message> {
    button(
        text(label)
            .size(14)
            .width(Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Fill)
    .padding([10.0, 12.0])
    .on_press(message)
    .style(segment_style(active))
    .into()
}

fn binding(press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    let character = match press.key.as_ref() {
        Key::Character(key) => Some(key.to_owned()),
        _ => None,
    };
    let command = press.modifiers.control() || press.modifiers.command();
    let shift = press.modifiers.shift();
    if !command
        && !press.modifiers.alt()
        && matches!(
            press.key.as_ref(),
            Key::Named(iced::keyboard::key::Named::Tab)
        )
    {
        return Some(text_editor::Binding::Custom(Message::ShiftItems {
            back: shift,
        }));
    }
    match (character.as_deref(), command, shift) {
        (Some("z" | "Z"), true, true) | (Some("y" | "Y"), true, _) => {
            Some(text_editor::Binding::Custom(Message::Redo))
        }
        // Formatting, on the same marks the buttons use. Shift keeps the ones a browser or a
        // word processor already gives to something else out of the way.
        (Some("b" | "B"), true, false) => Some(format_binding(FormatKind::Bold)),
        (Some("i" | "I"), true, false) => Some(format_binding(FormatKind::Italic)),
        (Some("k" | "K"), true, false) => Some(format_binding(FormatKind::Link)),
        (Some("e" | "E"), true, false) => Some(format_binding(FormatKind::Code)),
        (Some("x" | "X"), true, true) => Some(format_binding(FormatKind::Strike)),
        (Some("h" | "H"), true, true) => Some(format_binding(FormatKind::Highlight)),
        (Some("d" | "D"), true, true) => Some(text_editor::Binding::Custom(Message::AddWord)),
        (Some("z" | "Z"), true, false) => Some(text_editor::Binding::Custom(Message::Undo)),
        (Some("f" | "F"), true, _) => Some(text_editor::Binding::Custom(Message::FindOpen)),
        (Some("s"), true, _) => Some(text_editor::Binding::Custom(Message::Save)),
        (Some("n"), true, _) => Some(text_editor::Binding::Custom(Message::NewNote)),
        (Some("c"), true, _) => Some(text_editor::Binding::Custom(Message::CopyNote)),
        (Some("x"), true, _) => Some(text_editor::Binding::Custom(Message::CutNote)),
        _ => text_editor::Binding::from_key_press(press),
    }
}

fn format_binding(kind: FormatKind) -> text_editor::Binding<Message> {
    text_editor::Binding::Custom(Message::Format(kind))
}

fn composer_binding(press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    let enter = matches!(
        press.key.as_ref(),
        Key::Named(iced::keyboard::key::Named::Enter)
    );
    if enter && press.modifiers.shift() {
        return Some(text_editor::Binding::Enter);
    }
    if enter && !press.modifiers.command() && !press.modifiers.control() && !press.modifiers.alt() {
        return Some(text_editor::Binding::Custom(Message::SendChat));
    }
    text_editor::Binding::from_key_press(press)
}

fn generate_qr(data: &str) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("no data for QR".to_owned());
    }
    let code = QrCode::new(data.as_bytes()).map_err(|_| "could not make QR")?;
    let img = code
        .render::<::image::Luma<u8>>()
        .min_dimensions(200, 200)
        .build();
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    let encoder = ::image::codecs::png::PngEncoder::new(&mut cursor);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ::image::ExtendedColorType::L8,
        )
        .map_err(|_| "could not encode QR")?;
    Ok(buf)
}

/// `card` floating just under `anchor`, over everything else and pushing nothing aside. It keeps
/// clear of the edges of the window, goes above the anchor when there is no room below, and a
/// press anywhere else sends `dismiss`.
fn anchored<'a>(
    anchor: impl Into<Element<'a, Message>>,
    card: Option<Element<'a, Message>>,
    dismiss: Message,
) -> Element<'a, Message> {
    Anchored {
        anchor: anchor.into(),
        card,
        dismiss,
    }
    .into()
}

struct Anchored<'a> {
    anchor: Element<'a, Message>,
    card: Option<Element<'a, Message>>,
    dismiss: Message,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for Anchored<'_> {
    fn size(&self) -> iced::Size<iced::Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        self.anchor.as_widget().size_hint()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        let mut children = vec![iced::advanced::widget::Tree::new(&self.anchor)];
        if let Some(card) = &self.card {
            children.push(iced::advanced::widget::Tree::new(card));
        }
        children
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        match &self.card {
            Some(card) => tree.diff_children(&[self.anchor.as_widget(), card.as_widget()]),
            None => tree.diff_children(&[self.anchor.as_widget()]),
        }
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(iced::Size::ZERO);
        };
        self.anchor.as_widget_mut().layout(child, renderer, limits)
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
        self.anchor.as_widget_mut().update(
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
        self.anchor
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
        self.anchor
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let (anchor_tree, card_trees) = tree.children.split_first_mut()?;
        let anchor_overlay = self.anchor.as_widget_mut().overlay(
            anchor_tree,
            layout,
            renderer,
            viewport,
            translation,
        );
        let Some(card) = self.card.as_mut() else {
            return anchor_overlay;
        };
        let Some(card_tree) = card_trees.first_mut() else {
            return anchor_overlay;
        };
        let mut anchor = layout.bounds();
        anchor.x += translation.x;
        anchor.y += translation.y;
        let card_overlay = iced::advanced::overlay::Element::new(Box::new(CardOverlay {
            anchor,
            viewport: *viewport,
            card,
            tree: card_tree,
            dismiss: self.dismiss.clone(),
        }));
        let mut layers = Vec::new();
        if let Some(layer) = anchor_overlay {
            layers.push(layer);
        }
        layers.push(card_overlay);
        Some(iced::advanced::overlay::Group::with_children(layers).overlay())
    }
}

impl<'a> From<Anchored<'a>> for Element<'a, Message> {
    fn from(anchored: Anchored<'a>) -> Self {
        Element::new(anchored)
    }
}

struct CardOverlay<'a, 'b> {
    anchor: iced::Rectangle,
    viewport: iced::Rectangle,
    card: &'b mut Element<'a, Message>,
    tree: &'b mut iced::advanced::widget::Tree,
    dismiss: Message,
}

impl iced::advanced::Overlay<Message, Theme, iced::Renderer> for CardOverlay<'_, '_> {
    fn layout(
        &mut self,
        renderer: &iced::Renderer,
        bounds: iced::Size,
    ) -> iced::advanced::layout::Node {
        let node = self.card.as_widget_mut().layout(
            self.tree,
            renderer,
            // A margin of eight pixels each side, so a wide card never touches the window.
            &iced::advanced::layout::Limits::new(
                iced::Size::ZERO,
                iced::Size::new(
                    (bounds.width - 16.0).max(0.0),
                    (bounds.height - 16.0).max(0.0),
                ),
            ),
        );
        let size = node.size();
        let mut x = self.anchor.x + self.anchor.width - size.width;
        let mut y = self.anchor.y + self.anchor.height + 4.0;
        if x < 8.0 {
            x = 8.0;
        }
        let right = bounds.width - 8.0;
        if x + size.width > right {
            x = (right - size.width).max(8.0);
        }
        let bottom = bounds.height - 8.0;
        if y + size.height > bottom {
            let above = self.anchor.y - size.height - 4.0;
            y = if above >= 8.0 { above } else { 8.0 };
        }
        node.move_to(iced::Point::new(x, y))
    }

    fn update(
        &mut self,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
    ) {
        let press = matches!(
            event,
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
        );
        let on_card = cursor.is_over(layout.bounds());
        let on_anchor = cursor.is_over(self.anchor);
        if press && !on_card && !on_anchor {
            shell.publish(self.dismiss.clone());
            shell.capture_event();
            return;
        }
        self.card.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );
        if press && on_card {
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
    ) {
        self.card.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &self.viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let interaction = self.card.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &self.viewport,
            renderer,
        );
        if cursor.is_over(layout.bounds())
            && interaction == iced::advanced::mouse::Interaction::None
        {
            iced::advanced::mouse::Interaction::Idle
        } else {
            interaction
        }
    }
}

struct Breeze {
    phase: f32,
    bg: Color,
    ink: Color,
    second: Color,
    word: Color,
}

impl canvas::Program<Message> for Breeze {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.bg);
        stroke_current(&mut frame, self.phase, 0.50, self.ink, 1.0);
        stroke_current(&mut frame, self.phase + 0.8, 0.58, self.second, 0.85);
        paint_word(&mut frame, self.phase, self.word);
        vec![frame.into_geometry()]
    }
}

fn paint_word(frame: &mut canvas::Frame, phase: f32, color: Color) {
    let size = (frame.width() * 0.072).clamp(42.0, 84.0);
    let advance = size * 0.58;
    let count = 8.0;
    let total = advance * count;
    let mut x = (frame.width() - total) / 2.0 + advance / 2.0;
    let y = frame.height() * 0.34;
    let mut start = 0.0;
    for letter in "Aerodash".chars() {
        let t = ((phase - start) / 0.35).clamp(0.0, 1.0);
        let alpha = t * t * (3.0 - 2.0 * t);
        if alpha > 0.01 {
            frame.fill_text(canvas::Text {
                content: letter.to_string(),
                position: iced::Point::new(x, y),
                color: Color { a: alpha, ..color },
                size: iced::Pixels(size),
                font: serif(),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        }
        start += 0.28;
        x += advance;
    }
}

fn stroke_current(
    frame: &mut canvas::Frame,
    phase: f32,
    y_frac: f32,
    color: Color,
    width_mul: f32,
) {
    let width = frame.width();
    let height = frame.height();
    let span = width * 3.5;
    let amp = 7.0 + 20.0 * (phase * 0.85).sin().abs();
    let drift = phase * width * 0.7;
    let freq = std::f32::consts::TAU / (width * 1.25);
    let base_y = height * y_frac + (phase * 1.15).sin() * height * 0.035;
    let mut x = -span;
    let end = width + span * 0.35;
    let step = (width / 90.0).max(1.0);
    let mut first = true;
    let path = canvas::Path::new(|builder| {
        while x <= end {
            let along = x - drift;
            let y = base_y
                + (along * freq).sin() * amp
                + (along * freq * 0.43 + 0.6).sin() * amp * 0.32;
            let point = iced::Point::new(x, y);
            if first {
                builder.move_to(point);
                first = false;
            } else {
                builder.line_to(point);
            }
            x += step;
        }
    });
    let thickness = (1.15 + 0.7 * (phase * 1.6 + y_frac).sin().abs()) * width_mul;
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(Color { a: 0.82, ..color })
            .with_width(thickness)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round),
    );
}

#[cfg(test)]
mod history_tests {
    use std::time::{Duration, Instant};

    use iced::widget::text_editor;

    use super::{Burst, EditSnap, NoteHistory, chat_preview, edit_burst};

    fn snap(body: &str) -> EditSnap {
        EditSnap {
            title: String::new(),
            body: body.to_owned(),
            cursor: text_editor::Cursor {
                position: text_editor::Position { line: 0, column: 0 },
                selection: None,
            },
        }
    }

    #[test]
    fn typing_gathers_into_one_step() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        let empty = snap("");
        history.begin("", "", empty.cursor, Burst::Type, start);
        history.begin(
            "",
            "a",
            snap("a").cursor,
            Burst::Type,
            start + Duration::from_millis(120),
        );
        let back = history.undo(snap("ab")).expect("step");
        assert_eq!(back.body, "");
        let forward = history.redo(back).expect("forward");
        assert_eq!(forward.body, "ab");
    }

    #[test]
    fn a_pause_starts_another_step() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        history.begin("", "", snap("").cursor, Burst::Type, start);
        history.begin(
            "",
            "a",
            snap("a").cursor,
            Burst::Type,
            start + Duration::from_millis(900),
        );
        let back = history.undo(snap("ab")).expect("step");
        assert_eq!(back.body, "a");
        let earlier = history.undo(back).expect("earlier");
        assert_eq!(earlier.body, "");
    }

    #[test]
    fn a_format_does_not_merge_with_typing() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        history.begin("", "", snap("").cursor, Burst::Type, start);
        history.begin(
            "",
            "hi",
            snap("hi").cursor,
            Burst::Break,
            start + Duration::from_millis(40),
        );
        let back = history.undo(snap("**hi**")).expect("step");
        assert_eq!(back.body, "hi");
    }

    #[test]
    fn a_failed_edit_puts_the_step_back() {
        let mut history = NoteHistory::new();
        let gate = history.begin("", "a", snap("a").cursor, Burst::Erase, Instant::now());
        history.abort(gate);
        assert!(history.undo(snap("a")).is_none());
    }

    #[test]
    fn redo_is_dropped_when_typing_starts_again() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        history.begin("", "", snap("").cursor, Burst::Type, start);
        let back = history.undo(snap("a")).expect("step");
        assert_eq!(back.body, "");
        history.begin(
            "",
            "",
            snap("").cursor,
            Burst::Type,
            start + Duration::from_millis(40),
        );
        assert!(history.redo(snap("b")).is_none());
    }

    #[test]
    fn the_stack_keeps_eighty_steps() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        for index in 0..90 {
            let body = "x".repeat(index);
            let cursor = snap(&body).cursor;
            let at = start + Duration::from_millis(u64::try_from(index).unwrap_or(0));
            history.begin("", &body, cursor, Burst::Break, at);
        }
        assert_eq!(history.undo.len(), 80);
    }

    #[test]
    fn a_chat_preview_is_one_short_line() {
        assert_eq!(chat_preview("\nhello\nthere"), "hello");
        let long = "a".repeat(80);
        let preview = chat_preview(&long);
        assert!(preview.ends_with("..."));
        assert_eq!(preview.chars().count(), 75);
    }

    #[test]
    fn paste_is_its_own_step() {
        let pasted = text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new(
            "x".to_owned(),
        )));
        assert_eq!(edit_burst(&pasted), Some(Burst::Break));
        let typed = text_editor::Action::Edit(text_editor::Edit::Insert('a'));
        assert_eq!(edit_burst(&typed), Some(Burst::Type));
    }
}

#[cfg(test)]
mod scrollbar_tests {
    use iced::widget::scrollable::Status;

    use super::{SCROLL_WAKE, scroll_hot, scroll_seen, scrolled_recently, thumb_alpha};

    fn resting() -> Status {
        Status::Active {
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    fn over_pane() -> Status {
        Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: false,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    fn over_bar() -> Status {
        Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    fn dragging() -> Status {
        Status::Dragged {
            is_horizontal_scrollbar_dragged: false,
            is_vertical_scrollbar_dragged: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    #[test]
    fn a_scrollbar_is_not_drawn_at_rest() {
        assert!(thumb_alpha(resting(), false) < f32::EPSILON);
        // Hovering a pane is not scrolling it.
        assert!(thumb_alpha(over_pane(), false) < f32::EPSILON);
        // A pane the pointer is not over stays bare even right after another one scrolled.
        assert!(thumb_alpha(resting(), true) < f32::EPSILON);
    }

    #[test]
    fn a_scrollbar_shows_for_a_moment_after_the_pane_under_the_pointer_scrolls() {
        let shown = thumb_alpha(over_pane(), true);
        assert!(
            shown > 0.0 && shown < 0.62,
            "faint, not the full thumb: {shown}"
        );
    }

    #[test]
    fn the_bar_can_still_be_found_and_dragged() {
        assert!(scroll_hot(over_bar()) && scroll_hot(dragging()));
        assert!(!scroll_hot(over_pane()) && !scroll_hot(resting()));
        assert!(thumb_alpha(over_bar(), false) > thumb_alpha(over_pane(), true));
        assert!(thumb_alpha(dragging(), false) > 0.5);
    }

    #[test]
    fn a_burst_of_wheel_ticks_sends_one_message() {
        // Scrolling must not go through `update`, which rebuilds the whole view. A tick only
        // records the time. The first tick of a burst asks for the wake that hides the bar.
        SCROLL_WAKE.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(scroll_seen().is_some(), "the first tick starts the wake");
        for _ in 0..500 {
            assert!(scroll_seen().is_none(), "a later tick sent a message");
        }
        assert!(scrolled_recently());
        // Once the wake has run, the next burst starts a new one.
        SCROLL_WAKE.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(scroll_seen().is_some());
    }
}

/// What a test needs from the app's background writes. They are recorded so a test can do them
/// itself, the way the worker thread would, and hand the result back.
#[cfg(test)]
mod tests_support {
    use std::cell::RefCell;

    type Write = (Option<String>, String, String);

    thread_local! {
        static WRITES: RefCell<Vec<Write>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record_write(id: Option<&str>, title: &str, body: &str) {
        WRITES.with(|writes| {
            writes
                .borrow_mut()
                .push((id.map(str::to_owned), title.to_owned(), body.to_owned()));
        });
    }

    pub(super) fn take_writes() -> Vec<Write> {
        WRITES.with(|writes| std::mem::take(&mut *writes.borrow_mut()))
    }

    /// The published notebook and the personal dictionary are each one per process, so the
    /// tests that touch them take turns.
    pub(crate) fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        TURN.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Tests that drive a real `App` on a real folder, through the same `update` the window uses.
#[cfg(test)]
mod app_tests {
    use super::tests_support::one_at_a_time;
    use super::{App, Message, Notebook, tests_support, write_snapshot};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aerodash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn open_app(dir: &Path) -> App {
        Notebook::create(dir).expect("create");
        let (mut app, _) = App::at(dir.to_path_buf());
        app.loaded = true;
        // The splash is over, so the window takes keys.
        app.boot_at = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now);
        app.adopt_open_book(Notebook::open(dir).expect("open"));
        let _ = tests_support::take_writes();
        app
    }

    /// Do the writes the app asked for, as its worker thread would, and hand each result back.
    fn settle(app: &mut App) {
        for (id, title, body) in tests_support::take_writes() {
            let result =
                write_snapshot(&app.dir, id.as_deref(), &title, &body).map(|(_id, notes)| notes);
            let _ = app.update(Message::Applied(result));
        }
    }

    fn listed(app: &App) -> Vec<String> {
        app.tasks.iter().map(|task| task.text.clone()).collect()
    }

    #[test]
    fn the_tasks_tab_updates_when_a_task_is_added_removed_or_checked() {
        let _turn = one_at_a_time();
        let dir = folder("tasks");
        let mut app = open_app(&dir);
        let _ = app.update(Message::ShowTasks);
        assert!(listed(&app).is_empty());

        // The first task creates the Tasks note.
        let _ = app.update(Message::TaskDraft("buy milk".to_owned()));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        assert_eq!(listed(&app), ["buy milk"], "the first task did not show");

        // The second goes into the note that is there now.
        let _ = app.update(Message::TaskDraft("call the clinic".to_owned()));
        let _ = app.update(Message::AddTask);
        assert!(
            app.task_draft.is_empty(),
            "the field is empty for the next task"
        );
        settle(&mut app);
        let _ = app.update(Message::TaskDraft("third".to_owned()));
        let _ = app.update(Message::AddTask);
        assert!(
            app.task_draft.is_empty(),
            "and again, with a message already up"
        );
        settle(&mut app);
        let _ = app.update(Message::TaskDraft("fourth".to_owned()));
        let _ = app.update(Message::AddTask);
        assert!(app.task_draft.is_empty(), "every time");
        settle(&mut app);
        // Take the two extra tasks out so the checks below are as they were.
        let extra: Vec<(String, usize)> = app
            .tasks
            .iter()
            .filter(|task| task.text == "third" || task.text == "fourth")
            .map(|task| (task.note_id.clone(), task.line))
            .collect();
        assert_eq!(extra.len(), 2);
        for (id, line) in extra.into_iter().rev() {
            let _ = app.update(Message::RemoveTask { id, line });
            settle(&mut app);
        }
        assert_eq!(listed(&app), ["buy milk", "call the clinic"]);

        // Checking one moves it to done, without leaving the tab.
        let milk = app
            .tasks
            .iter()
            .find(|task| task.text == "buy milk")
            .expect("task");
        let (id, line) = (milk.note_id.clone(), milk.line);
        let _ = app.update(Message::SetTask {
            id: id.clone(),
            line,
            done: true,
        });
        settle(&mut app);
        let milk = app
            .tasks
            .iter()
            .find(|task| task.text == "buy milk")
            .expect("task");
        assert!(milk.done, "the check did not show");

        // Removing one takes it off the list.
        let _ = app.update(Message::RemoveTask { id, line });
        settle(&mut app);
        assert_eq!(
            listed(&app),
            ["call the clinic"],
            "the removal did not show"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_conversation_name_is_text_until_the_pencil_is_pressed() {
        let _turn = one_at_a_time();
        let dir = folder("alias");
        let mut app = open_app(&dir);
        app.chats = vec![ambient_cli::ConversationSummary {
            id: "abc".to_owned(),
            label: "Key 12ab".to_owned(),
            other: String::new(),
            members: 2,
            started: false,
            others: Vec::new(),
        }];
        app.chat_id = Some("abc".to_owned());
        assert!(app.alias_editing.is_none(), "the name starts as plain text");

        let _ = app.update(Message::EditAlias);
        assert_eq!(app.alias_editing.as_deref(), Some("abc"));
        let _ = app.update(Message::AliasDraft("abc".to_owned(), "Sam".to_owned()));
        let chat = app.chats[0].clone();
        assert_eq!(app.chat_name(&chat), "Sam", "typing names them at once");

        // Enter, or Done, puts the plain name back.
        let _ = app.update(Message::FinishAlias);
        assert!(app.alias_editing.is_none());
        assert_eq!(app.chat_name(&chat), "Sam");

        // The pencil again opens it, and pressing it once more closes it.
        let _ = app.update(Message::EditAlias);
        assert!(app.alias_editing.is_some());
        let _ = app.update(Message::EditAlias);
        assert!(app.alias_editing.is_none());

        // Leaving and coming back does not leave the field open.
        let _ = app.update(Message::EditAlias);
        let _ = app.update(Message::ShowNotes);
        let _ = app.update(Message::ShowChat);
        assert!(app.alias_editing.is_none());

        // Clearing the name falls back to the label the conversation came with.
        let _ = app.update(Message::AliasDraft("abc".to_owned(), String::new()));
        assert_eq!(app.chat_name(&chat), "Key 12ab");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn wide(app: &mut App) {
        let _ = app.update(Message::Resized(iced::Size::new(1100.0, 720.0)));
    }

    #[test]
    fn focus_hides_the_panels_and_sends_the_bar_away() {
        let _turn = one_at_a_time();
        let dir = folder("focus");
        let mut app = open_app(&dir);
        wide(&mut app);
        assert!(!app.focused(), "focus starts off");
        assert!(app.slide_bar.open);

        let _ = app.update(Message::ToggleFocus);
        assert!(app.focused());
        assert!(!app.slide_bar.open, "the bar slides away");
        assert!(app.slide_bar.moving(), "and it moves, not jumps");

        // The Aa button brings it back, and sends it away again.
        let _ = app.update(Message::FocusBar);
        assert!(app.slide_bar.open);
        let _ = app.update(Message::FocusBar);
        assert!(!app.slide_bar.open);

        // Leaving puts everything back at once, so nothing is left hidden.
        let _ = app.update(Message::ToggleFocus);
        assert!(!app.focused());
        assert!(app.slide_bar.open);
        assert!((app.slide_bar.shown() - 1.0).abs() < f32::EPSILON);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn focus_only_applies_to_writing_a_note_in_the_wide_layout() {
        let _turn = one_at_a_time();
        let dir = folder("focus-where");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::ToggleFocus);
        assert!(app.focused());
        // Another screen is not changed by it.
        let _ = app.update(Message::ShowTasks);
        assert!(!app.focused());
        let _ = app.update(Message::ShowNotes);
        assert!(app.focused(), "it is still on when the notes are back");
        // A narrow window has its own layout.
        let _ = app.update(Message::Resized(iced::Size::new(600.0, 700.0)));
        assert!(!app.focused());
        let _ = app.update(Message::Resized(iced::Size::new(1100.0, 720.0)));
        assert!(app.focused());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_window_draws_in_every_state_of_focus() {
        let _turn = one_at_a_time();
        let dir = folder("focus-draw");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        // Normal, going into focus (mid-slide), in focus with each thing pulled in, and out again.
        drop(app.view());
        let _ = app.update(Message::ToggleFocus);
        drop(app.view());
        let _ = app.update(Message::FocusBar);
        drop(app.view());
        let _ = app.update(Message::FocusBar);
        drop(app.view());
        // With the delete question open, which is drawn in the corner where the buttons were.
        let _ = app.update(Message::AskDelete);
        drop(app.view());
        let _ = app.update(Message::CancelDelete);
        let _ = app.update(Message::ToggleFocus);
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_tasks_note_is_not_in_the_notes_list_and_opens_over_the_tasks_tab() {
        let _turn = one_at_a_time();
        let dir = folder("tasks-note-hidden");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::TaskDraft("buy milk".to_owned()));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        assert!(app.catalog.iter().any(|note| note.title == "Tasks"));
        assert!(
            app.notes.iter().all(|note| note.title != "Tasks"),
            "the notes list does not show it"
        );
        let _ = app.update(Message::ToggleTasksNote);
        assert!(
            app.tasks_note_text()
                .is_some_and(|text| text.contains("buy milk"))
        );
        drop(app.view());
        let _ = app.update(Message::ShowNotes);
        let _ = app.update(Message::ShowTasks);
        assert!(!app.tasks_note_open, "it is shut on the way back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_selection_goes_to_a_chat_with_where_it_is_from() {
        assert_eq!(
            super::selection_message("Plan", (3, 3), "", "one idea"),
            "> one idea\n\n— Plan, line 3"
        );
        assert_eq!(
            super::selection_message("", (2, 4), "Laptop", "a\nb\n"),
            "> a\n> b\n\n— Untitled, lines 2–4, written on Laptop"
        );
    }

    #[test]
    fn tags_used_before_are_offered_and_narrow_as_you_type() {
        let _turn = one_at_a_time();
        let dir = folder("tag-suggest");
        let mut app = open_app(&dir);
        let note = |id: &str, tags: &[&str]| super::NoteSummary {
            id: id.to_owned(),
            title: id.to_owned(),
            writer: String::new(),
            conflict: false,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            version: String::new(),
        };
        app.catalog = vec![
            note("a", &["project", "reading"]),
            note("b", &["project", "recipes"]),
            note("c", &["Research"]),
        ];
        assert!(
            app.tag_suggestions().is_empty(),
            "nothing typed, nothing offered"
        );
        let _ = app.update(Message::TagDraft("r".to_owned()));
        assert_eq!(
            app.tag_suggestions(),
            ["reading", "recipes", "Research", "project"],
            "those that start with it first, then those that only contain it"
        );
        let _ = app.update(Message::TagDraft("re".to_owned()));
        assert_eq!(app.tag_suggestions(), ["reading", "recipes", "Research"]);
        let _ = app.update(Message::TagDraft("rec".to_owned()));
        assert_eq!(app.tag_suggestions(), ["recipes"]);
        drop(app.view());
        let _ = app.update(Message::PickSuggestion("recipes".to_owned()));
        assert_eq!(app.tags, ["recipes"]);
        assert!(app.tag_draft.is_empty());
        let _ = app.update(Message::TagDraft("rec".to_owned()));
        assert!(
            app.tag_suggestions().is_empty(),
            "one already on the note is not offered"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wheel_notch_is_a_page_step_not_a_fixed_sixty() {
        assert!(
            (super::wheel_step(100.0) - 60.0).abs() < f32::EPSILON,
            "a small window keeps 60"
        );
        let tall = super::wheel_step(720.0);
        assert!((79.0..81.0).contains(&tall), "{tall}");
        assert!(
            (super::wheel_step(100_000.0) - 150.0).abs() < f32::EPSILON,
            "and a huge one is capped"
        );
    }

    #[test]
    fn the_side_panels_ease_out_and_back_with_focus() {
        let _turn = one_at_a_time();
        let dir = folder("panel-slide");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        assert!((app.panel_shown() - 1.0).abs() < f32::EPSILON);
        let _ = app.update(Message::ToggleFocus);
        assert!(app.slide_panels.moving(), "they move, not jump");
        assert!(app.panel_shown() > 0.5, "they have only just started to go");
        drop(app.view());
        app.slide_panels.jump(false);
        assert!(
            app.panel_shown() < f32::EPSILON,
            "and in the end they are away"
        );
        drop(app.view());
        let _ = app.update(Message::ToggleFocus);
        assert!(app.slide_panels.moving(), "they come back by sliding");
        app.slide_panels.jump(true);
        assert!((app.panel_shown() - 1.0).abs() < f32::EPSILON);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn summary(id: &str, tags: &[&str]) -> super::NoteSummary {
        super::NoteSummary {
            id: id.to_owned(),
            title: id.to_owned(),
            writer: String::new(),
            conflict: false,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            version: String::new(),
        }
    }

    #[test]
    fn a_group_is_a_tag_and_a_slash_makes_a_group_inside_a_group() {
        use super::ListLine::Group;
        let note = |index, depth, group: &str| super::ListLine::Note {
            index,
            depth,
            group: group.to_owned(),
        };
        let notes = [
            summary("ch1", &["Novel/Part 1"]),
            summary("ch2", &["novel / part 1", "Research"]),
            summary("map", &["Novel"]),
            summary("loose", &[]),
        ];
        let open = std::collections::BTreeSet::new();
        let lines = super::group_outline(&notes, &open, &std::collections::BTreeMap::new());
        let group = |path: &str, name: &str, depth, count| Group {
            path: path.to_owned(),
            name: name.to_owned(),
            depth,
            count,
            open: true,
        };
        assert_eq!(
            lines,
            [
                group("novel", "Novel", 0, 3),
                group("novel/part 1", "Part 1", 1, 2),
                note(0, 2, "novel/part 1"),
                note(1, 2, "novel/part 1"),
                note(2, 1, "novel"),
                group("research", "Research", 0, 1),
                note(1, 1, "research"),
                note(3, 0, ""),
            ],
            "case and spacing do not make a second group, a note in two groups shows in both, \
             and an untagged note comes after the groups"
        );
        // Folding a group hides what is in it, not the groups beside it.
        let mut folded = std::collections::BTreeSet::new();
        folded.insert("novel".to_owned());
        let shut = super::group_outline(&notes, &folded, &std::collections::BTreeMap::new());
        assert_eq!(shut.len(), 4, "{shut:?}");
        assert!(matches!(
            shut.first(),
            Some(Group {
                open: false,
                count: 3,
                ..
            })
        ));
    }

    #[test]
    fn a_tag_is_in_a_group_when_it_is_that_tag_or_inside_it() {
        let tags = ["Novel/Part 1".to_owned()];
        assert!(super::in_group(&tags, "novel"));
        assert!(super::in_group(&tags, "Novel / Part 1"));
        assert!(
            !super::in_group(&tags, "Nov"),
            "a prefix of a name is not a group"
        );
        assert!(!super::in_group(&tags, "Novel/Part 2"));
    }

    #[test]
    fn a_group_path_is_tidied_when_it_is_added() {
        assert_eq!(
            super::clean_tag(" #Novel / Part 1 ").as_deref(),
            Some("Novel/Part 1")
        );
        assert_eq!(super::clean_tag("/ / "), None);
        assert_eq!(super::clean_tag("a|b"), None);
    }

    #[test]
    fn the_open_note_lists_what_it_links_to_and_the_graph_has_every_link() {
        use super::lane::{NoteFile, OpenSnap, compute};
        let file = |id: &str, body: &str| NoteFile {
            id: id.to_owned(),
            title: id.to_owned(),
            body: body.to_owned(),
        };
        let files = [
            file("a", "to [[b]] and [[c]] and [[a]]"),
            file("b", "back to [[a]]"),
            file("c", "nothing"),
            file("d", "to [[missing]]"),
        ];
        let open = OpenSnap {
            selected: Some("a".to_owned()),
            title: "a".to_owned(),
            stored: files[0].body.clone(),
            unsaved: Vec::new(),
        };
        let done = compute(&files, &open);
        assert_eq!(
            done.links_to,
            [
                ("b".to_owned(), "b".to_owned()),
                ("c".to_owned(), "c".to_owned())
            ],
            "a link to itself and a link to nothing are not listed"
        );
        assert_eq!(done.backlinks, [("b".to_owned(), "b".to_owned())]);
        assert_eq!(done.edges.len(), 3, "a-b, a-c, b-a: {:?}", done.edges);
    }

    #[test]
    fn a_group_is_one_cluster_in_a_circle_and_the_circles_do_not_touch() {
        let mut catalog = Vec::new();
        for index in 0..7 {
            catalog.push(summary(&format!("n{index}"), &["Novel/Part 1"]));
        }
        for index in 0..4 {
            catalog.push(summary(&format!("r{index}"), &["research"]));
        }
        catalog.push(summary("solo", &[]));
        let links = vec![("n0".to_owned(), "r0".to_owned())];
        let graph = super::build_graph(&catalog, &links);
        assert_eq!(graph.groups.len(), 3, "novel, research, and no group");
        let names: Vec<_> = graph
            .groups
            .iter()
            .map(|group| group.name.as_str())
            .collect();
        assert_eq!(names, ["Novel", "research", "No group"]);
        assert!(graph.groups[2].shade.is_none(), "no group has no color");
        // Every dot is inside its own circle.
        for node in &graph.nodes {
            let group = &graph.groups[node.group];
            assert!(
                (node.x - group.x).hypot(node.y - group.y) <= group.r,
                "{node:?}"
            );
        }
        // No two circles overlap, with room between.
        for (a, first) in graph.groups.iter().enumerate() {
            for second in &graph.groups[a + 1..] {
                let far = (first.x - second.x).hypot(first.y - second.y);
                assert!(far >= first.r + second.r, "{first:?} {second:?}");
            }
        }
        // The box holds every circle, and the picture is the same each time.
        let (left, top, right, bottom) = graph.bounds;
        for group in &graph.groups {
            assert!(group.x - group.r >= left - 0.01 && group.x + group.r <= right + 0.01);
            assert!(group.y - group.r >= top - 0.01 && group.y + group.r <= bottom + 0.01);
        }
        let again = super::build_graph(&catalog, &links);
        assert_eq!(format!("{graph:?}"), format!("{again:?}"));
        assert_eq!(super::build_graph(&[], &[]).nodes.len(), 0);
    }

    #[test]
    fn groups_that_link_sit_nearer_than_groups_that_do_not() {
        let mut catalog = Vec::new();
        for (group, count) in [("a", 3), ("b", 3), ("c", 3)] {
            for index in 0..count {
                catalog.push(summary(&format!("{group}{index}"), &[group]));
            }
        }
        let links = vec![
            ("a0".to_owned(), "b0".to_owned()),
            ("a1".to_owned(), "b1".to_owned()),
            ("a2".to_owned(), "b2".to_owned()),
        ];
        let graph = super::build_graph(&catalog, &links);
        let far = |x: usize, y: usize| {
            let (first, second) = (&graph.groups[x], &graph.groups[y]);
            (first.x - second.x).hypot(first.y - second.y)
        };
        assert!(far(0, 1) < far(0, 2), "a and b link, a and c do not");
        assert!(far(0, 1) < far(1, 2));
    }

    #[test]
    fn the_graph_opens_in_place_of_the_note_and_a_dot_opens_a_note() {
        let _turn = one_at_a_time();
        let dir = folder("graph");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.catalog = vec![summary("a", &["Novel"]), summary("b", &[])];
        app.graph_edges = vec![("a".to_owned(), "b".to_owned())];
        let _ = app.update(Message::ToggleGraph);
        let data = app.graph.clone().expect("built when opened");
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges, [(0, 1)]);
        assert!(data.groups[data.nodes[0].group].shade.is_some());
        assert!(data.groups[data.nodes[1].group].shade.is_none());
        drop(app.view());
        let _ = app.update(Message::ToggleGraph);
        assert!(app.graph.is_none(), "dropped when closed");
        let _ = app.update(Message::ToggleGraph);
        let _ = app.update(Message::GraphPick("a".to_owned()));
        assert!(!app.graph_open, "picking a dot goes back to the page");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_links_no_longer_make_a_group_and_a_group_is_read_by_its_members() {
        let _turn = one_at_a_time();
        let dir = folder("read-group");
        let mut app = open_app(&dir);
        wide(&mut app);
        // A note that links two others is an ordinary note. Nothing turns on.
        let _ = app.update(Message::TitleChanged("Hub".to_owned()));
        let _ = app.update(Message::BodyEdited(
            iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Paste(
                std::sync::Arc::new("[[One]] and [[Two]]".to_owned()),
            )),
        ));
        assert!(!app.reading && app.cached_chapters.is_empty());
        let _ = app.update(Message::ToggleRead);
        assert!(!app.reading, "no group in view, nothing to read");

        // With a group in view, Read draws its notes in title order, and goes back.
        app.catalog = vec![
            summary("b", &["Novel/Part 1"]),
            summary("a", &["Novel"]),
            summary("x", &[]),
        ];
        app.tag = Some("Novel".to_owned());
        let _ = app.update(Message::ToggleRead);
        // The notes are not in a folder here, so the bodies are empty, but the order is real.
        assert!(app.reading);
        let names: Vec<_> = app
            .cached_chapters
            .iter()
            .map(|chapter| chapter.name.as_str())
            .collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(app.read_title, "Novel");
        drop(app.view());
        let _ = app.update(Message::ToggleRead);
        assert!(!app.reading);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn titles_sort_the_way_a_person_counts() {
        let mut titles = vec![
            "Chapter 10",
            "chapter 2",
            "Chapter 1",
            "Epilogue",
            "Appendix",
            "Chapter 02b",
        ];
        titles.sort_by(|a, b| super::natural_cmp(a, b));
        assert_eq!(
            titles,
            [
                "Appendix",
                "Chapter 1",
                "chapter 2",
                "Chapter 02b",
                "Chapter 10",
                "Epilogue"
            ]
        );
    }

    #[test]
    fn notes_in_a_group_are_alphabetical_until_placed_by_hand() {
        let item = |id: &str, title: &str| (id.to_owned(), title.to_owned());
        let mut items = vec![
            item("c10", "Chapter 10"),
            item("pre", "Prequel"),
            item("c2", "Chapter 2"),
            item("c1", "Chapter 1"),
        ];
        super::order_ids(&mut items, &[]);
        let ids: Vec<_> = items.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            ["c1", "c2", "c10", "pre"],
            "alphabetical, Chapter 2 before Chapter 10"
        );
        // The prequel pulled to the top, and the rest left as they were.
        let saved = super::move_in_order(
            &ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>(),
            "pre",
            None,
        );
        assert_eq!(saved, ["pre", "c1", "c2", "c10"]);
        super::order_ids(&mut items, &saved);
        assert_eq!(items[0].0, "pre");
        // A note added later is not in the saved order, so it comes after, alphabetically.
        items.push(item("c3", "Chapter 3"));
        items.push(item("a", "A new one"));
        super::order_ids(&mut items, &saved);
        let ids: Vec<_> = items.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["pre", "c1", "c2", "c10", "a", "c3"]);
    }

    #[test]
    fn a_dropped_note_lands_where_the_pointer_was() {
        let ids: Vec<String> = ["a", "b", "c", "d"].map(str::to_owned).into();
        let moved = |id: &str, before: Option<&str>| super::move_in_order(&ids, id, before);
        assert_eq!(
            moved("a", Some("c")),
            ["b", "c", "a", "d"],
            "down: after the one it is dropped on"
        );
        assert_eq!(moved("d", Some("b")), ["a", "d", "b", "c"], "up: before it");
        assert_eq!(
            moved("c", None),
            ["c", "a", "b", "d"],
            "on the group's name: first"
        );
        assert_eq!(moved("b", Some("b")), ids, "on itself: nothing");
        assert_eq!(moved("zz", Some("a")), ids, "a stranger: nothing");
        assert_eq!(moved("a", Some("zz")), ids, "onto a stranger: nothing");
    }

    #[test]
    fn the_outline_follows_the_order_set_by_hand() {
        let notes = [
            summary("ch1", &["Novel"]),
            summary("ch2", &["Novel"]),
            summary("pre", &["Novel"]),
        ];
        let mut orders = std::collections::BTreeMap::new();
        orders.insert("novel".to_owned(), vec!["pre".to_owned()]);
        let lines = super::group_outline(&notes, &std::collections::BTreeSet::new(), &orders);
        let order: Vec<_> = lines
            .iter()
            .filter_map(|line| match line {
                super::ListLine::Note { index, .. } => Some(notes[*index].id.as_str()),
                super::ListLine::Group { .. } => None,
            })
            .collect();
        assert_eq!(order, ["pre", "ch1", "ch2"]);
    }

    #[test]
    fn dragging_a_note_to_the_top_is_kept_and_survives_a_restart() {
        let _turn = one_at_a_time();
        let dir = folder("drag-order");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.catalog = vec![
            summary("c1", &["Novel"]),
            summary("c2", &["Novel"]),
            summary("pre", &["Novel"]),
        ];
        app.catalog[0].title = "Chapter 1".to_owned();
        app.catalog[1].title = "Chapter 2".to_owned();
        app.catalog[2].title = "Prequel".to_owned();
        app.refresh_shown();
        let shown = |app: &App| -> Vec<String> {
            app.exact_members("novel")
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        };
        assert_eq!(shown(&app), ["c1", "c2", "pre"]);
        // Press the grip on the prequel, move onto the group's name, let go.
        let _ = app.update(Message::DragStart {
            group: "novel".to_owned(),
            id: "pre".to_owned(),
        });
        drop(app.view());
        let _ = app.update(Message::RowHover(super::top_of("novel"), true));
        drop(app.view());
        let _ = app.update(Message::SelectStop);
        assert!(app.list_drag.is_none());
        assert_eq!(shown(&app), ["pre", "c1", "c2"]);
        // Letting go over nothing changes nothing.
        let _ = app.update(Message::DragStart {
            group: "novel".to_owned(),
            id: "c2".to_owned(),
        });
        let _ = app.update(Message::SelectStop);
        assert_eq!(shown(&app), ["pre", "c1", "c2"]);
        // Another note's row: c2 goes before c1.
        let _ = app.update(Message::DragStart {
            group: "novel".to_owned(),
            id: "c2".to_owned(),
        });
        let _ = app.update(Message::RowHover("c1".to_owned(), true));
        let _ = app.update(Message::SelectStop);
        assert_eq!(shown(&app), ["pre", "c2", "c1"]);
        assert_eq!(
            super::profile::load_order(&dir).get("novel").map(Vec::len),
            Some(3)
        );
        let (mut again, _) = App::at(dir.clone());
        again.orders = super::profile::load_order(&dir);
        again.catalog = app.catalog.clone();
        assert_eq!(shown(&again), ["pre", "c2", "c1"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn typing_a_tag_does_not_change_the_shape_of_the_tag_row() {
        let _turn = one_at_a_time();
        let dir = folder("tag-shape");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.catalog = vec![summary("a", &["recipes"])];
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        // Nothing offered, then something offered, then nothing again: the row is built the
        // same way each time, so the field keeps its place and the caret.
        for draft in ["", "r", "re", "rez", ""] {
            let _ = app.update(Message::TagDraft(draft.to_owned()));
            let row = app.tag_chips();
            let tree = iced::advanced::widget::Tree::new(&row);
            assert_eq!(tree.children.len(), 2, "two rows for {draft:?}");
            assert_eq!(
                tree.children.first().map(|first| first.children.len()),
                Some(1),
                "the field is alone in the first row, for {draft:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fix the clock at 2026-11-20 in Los Angeles, at this minute of the day.
    fn pinned(app: &mut App, minutes: u32) -> super::dates::Date {
        let today = super::dates::Date::new(2026, 11, 20).expect("a day");
        let clock = super::clock::Clock {
            today,
            minutes,
            offset: -8 * 3_600,
            zone: "America/Los_Angeles".to_owned(),
        };
        app.clock = clock.clone();
        app.pinned_clock = Some(clock);
        today
    }

    fn task_row(id: &str, line: usize, done: bool, text: &str) -> super::TaskRow {
        super::TaskRow {
            note_id: id.to_owned(),
            note_title: id.to_owned(),
            line,
            done,
            due: super::due::Due::of(&super::markdown::without_cites(text)),
            text: text.to_owned(),
        }
    }

    #[test]
    fn the_task_views_sort_by_date_group_and_note() {
        use super::TaskView;
        let _turn = one_at_a_time();
        let dir = folder("task-views");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        app.catalog = vec![summary("plan", &["Novel/Part 1"]), summary("home", &[])];
        app.tasks = vec![
            task_row("plan", 0, false, "late 2026-11-01"),
            task_row("plan", 1, false, "now 2026-11-20"),
            task_row("home", 0, false, "soon 2026-12-24"),
            task_row("home", 1, false, "someday"),
            task_row("home", 2, true, "finished 2026-11-01"),
            task_row("home", 3, false, "impossible 2026-02-30"),
            task_row("plan", 2, false, "cited 2026-11-20 [[abc|Plan]]"),
        ];
        let ids = |app: &App, view: &TaskView| -> Vec<String> {
            app.tasks
                .iter()
                .filter(|task| app.in_view(task, view))
                .map(|task| format!("{}{}", task.note_id, task.line))
                .collect()
        };
        assert_eq!(
            ids(&app, &TaskView::Today),
            ["plan1", "plan2"],
            "a cite after the date still counts"
        );
        assert_eq!(
            ids(&app, &TaskView::Overdue),
            ["plan0"],
            "done ones are not overdue"
        );
        assert_eq!(ids(&app, &TaskView::Upcoming), ["home0"]);
        assert_eq!(
            ids(&app, &TaskView::NoDate),
            ["home1", "home3"],
            "a date that does not exist is no date"
        );
        assert_eq!(ids(&app, &TaskView::Done), ["home2"]);
        assert_eq!(ids(&app, &TaskView::Note("home".to_owned())).len(), 4);
        assert_eq!(
            ids(&app, &TaskView::Group("novel".to_owned())),
            ["plan0", "plan1", "plan2"],
            "a group has the tasks of its notes, and so does the group above it"
        );
        assert_eq!(
            ids(&app, &TaskView::Group("novel/part 1".to_owned())).len(),
            3
        );
        assert!(ids(&app, &TaskView::Group("other".to_owned())).is_empty());
        for view in [
            TaskView::Open,
            TaskView::Today,
            TaskView::Overdue,
            TaskView::Upcoming,
            TaskView::NoDate,
            TaskView::Done,
            TaskView::Group("novel".to_owned()),
            TaskView::Note("home".to_owned()),
        ] {
            let _ = app.update(Message::ShowTaskView(view));
            let _ = app.update(Message::ShowTasks);
            drop(app.view());
        }
        let _ = app.update(Message::ShowTaskView(TaskView::Note("gone".to_owned())));
        assert_eq!(
            app.task_view_name(),
            "that note",
            "a note not in the list has no name"
        );
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_with_a_time_turns_late_in_its_minute_and_moves_views_with_the_clock() {
        use super::TaskView;
        let _turn = one_at_a_time();
        let dir = folder("task-late");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        app.tasks = vec![
            task_row("n", 0, false, "early 2026-11-20 09:00"),
            task_row("n", 1, false, "later 2026-11-20 11:00"),
            task_row("n", 2, false, "all day 2026-11-20"),
        ];
        let now = |app: &App, view: &TaskView| -> Vec<usize> {
            app.tasks
                .iter()
                .filter(|task| app.in_view(task, view))
                .map(|task| task.line)
                .collect()
        };
        assert_eq!(now(&app, &TaskView::Overdue), [0], "nine is past at ten");
        assert_eq!(now(&app, &TaskView::Today), [1, 2]);
        // Two hours on: the eleven has passed, the day with no time has not.
        pinned(&mut app, 12 * 60);
        assert_eq!(now(&app, &TaskView::Overdue), [0, 1]);
        assert_eq!(now(&app, &TaskView::Today), [2]);
        // At the end of the day it is still today, and a minute into the next it is late.
        pinned(&mut app, 23 * 60 + 59);
        assert_eq!(now(&app, &TaskView::Today), [2]);
        app.clock.today = super::dates::Date::new(2026, 11, 21).expect("a day");
        app.clock.minutes = 1;
        assert_eq!(now(&app, &TaskView::Overdue), [0, 1, 2]);
        assert!(now(&app, &TaskView::Today).is_empty());
        // The panel says what day and time it is, and where, and the lines say what is late.
        let _ = app.update(Message::ShowTasks);
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_calendar_writes_a_real_date_in_the_one_form() {
        use super::CalFor;
        let _turn = one_at_a_time();
        let dir = folder("calendar");
        let mut app = open_app(&dir);
        wide(&mut app);
        pinned(&mut app, 10 * 60);
        let _ = app.update(Message::ShowTasks);
        drop(app.view());
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        assert_eq!(app.cal, Some(CalFor::New));
        assert_eq!(app.cal_month, (2026, 11));
        drop(app.view());
        let _ = app.update(Message::CalMonth(2));
        assert_eq!(app.cal_month, (2027, 1));
        drop(app.view());
        let _ = app.update(Message::PickDay("2027-01-09".to_owned()));
        assert_eq!(app.new_due.date, "2027-01-09");
        assert!(app.cal.is_none(), "choosing a day shuts it");
        // Something that is not a day is not taken.
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::PickDay("2027-02-30".to_owned()));
        assert_eq!(app.new_due.date, "2027-01-09");
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::PickDay("tomorrow".to_owned()));
        assert_eq!(app.new_due.date, "2027-01-09");
        // Opening it again shows the month of the day already picked, and the button shuts it.
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        assert_eq!(app.cal_month, (2027, 1));
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        assert!(app.cal.is_none());
        // The quick ones count from the real today, worked out when they are pressed.
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::PickQuick(1));
        assert_eq!(app.new_due.date, "2026-11-21");
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::PickQuick(7));
        assert_eq!(app.new_due.date, "2026-11-27");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_calendar_sets_a_time_and_a_repeat_and_keeps_them_in_step_with_the_day() {
        use super::CalFor;
        use super::due::{Repeat, Time, Unit};
        let _turn = one_at_a_time();
        let dir = folder("calendar-time");
        let mut app = open_app(&dir);
        wide(&mut app);
        pinned(&mut app, 10 * 60);
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        // Stepping a time with no day starts at nine and sets today.
        let _ = app.update(Message::StepTime {
            hours: 0,
            minutes: 0,
        });
        assert_eq!(
            (app.new_due.date.as_str(), app.new_due.time.as_str()),
            ("2026-11-20", "09:00")
        );
        let _ = app.update(Message::StepTime {
            hours: 1,
            minutes: 5,
        });
        assert_eq!(app.new_due.time, "10:05");
        // Past midnight wraps, both ways, and never makes a time that does not exist.
        let _ = app.update(Message::StepTime {
            hours: 14,
            minutes: 0,
        });
        assert_eq!(app.new_due.time, "00:05");
        let _ = app.update(Message::StepTime {
            hours: 0,
            minutes: -10,
        });
        assert_eq!(app.new_due.time, "23:55");
        let _ = app.update(Message::SetTime("17:00".to_owned()));
        assert_eq!(app.new_due.time, "17:00");
        let _ = app.update(Message::SetTime("25:99".to_owned()));
        assert_eq!(
            app.new_due.time, "17:00",
            "a time that is not one is not taken"
        );
        drop(app.view());
        // A repeat keeps the day, picks one when there is none, and keeps how many.
        let _ = app.update(Message::SetRepeat(Some(Repeat {
            every: 1,
            unit: Unit::Week,
        })));
        let _ = app.update(Message::RepeatEvery(1));
        let _ = app.update(Message::RepeatEvery(1));
        assert_eq!(app.new_due.repeat.map(|repeat| repeat.every), Some(3));
        let _ = app.update(Message::RepeatEvery(500));
        assert_eq!(
            app.new_due.repeat.map(|repeat| repeat.every),
            Some(99),
            "at most 99"
        );
        let _ = app.update(Message::RepeatEvery(-500));
        assert_eq!(
            app.new_due.repeat.map(|repeat| repeat.every),
            Some(1),
            "at least one"
        );
        let _ = app.update(Message::RepeatEvery(2));
        drop(app.view());
        assert_eq!(
            app.new_due.due().map(|due| due.time),
            Some(Time::new(17, 0))
        );
        // Weekdays is only ever every one.
        let _ = app.update(Message::SetRepeat(Some(Repeat {
            every: 1,
            unit: Unit::Weekday,
        })));
        let _ = app.update(Message::RepeatEvery(4));
        assert_eq!(app.new_due.repeat.map(|repeat| repeat.every), Some(1));
        drop(app.view());
        // No date takes the time and the repeat with it.
        let _ = app.update(Message::ClearDue);
        assert_eq!(app.new_due, super::DueDraft::default());
        // A repeat with no day starts today.
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::SetRepeat(Some(Repeat {
            every: 1,
            unit: Unit::Day,
        })));
        assert_eq!(app.new_due.date, "2026-11-20");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_added_with_a_moment_ends_in_it_and_a_repeating_one_is_moved_not_checked() {
        use super::CalFor;
        use super::due::{Repeat, Unit};
        let _turn = one_at_a_time();
        let dir = folder("task-date");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::TaskDraft("buy milk".to_owned()));
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::PickDay("2026-11-21".to_owned()));
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::SetTime("08:30".to_owned()));
        let _ = app.update(Message::SetRepeat(Some(Repeat {
            every: 1,
            unit: Unit::Week,
        })));
        let _ = app.update(Message::AddTask);
        assert_eq!(
            app.new_due,
            super::DueDraft::default(),
            "the next task starts with nothing"
        );
        assert!(app.task_draft.is_empty());
        settle(&mut app);
        let task = app
            .tasks
            .iter()
            .find(|task| task.text.starts_with("buy milk"));
        assert_eq!(
            task.map(|task| task.text.as_str()),
            Some("buy milk 2026-11-21 08:30 repeat:weekly")
        );
        assert!(
            task.is_some_and(|task| task.due.is_some()),
            "and it reads back"
        );
        // A plain one has none.
        let _ = app.update(Message::TaskDraft("call".to_owned()));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        assert!(
            app.tasks
                .iter()
                .any(|task| task.text == "call" && task.due.is_none())
        );
        // Checking the repeating one moves it a week on, and leaves it open.
        let (id, line) = app
            .tasks
            .iter()
            .find(|task| task.text.starts_with("buy milk"))
            .map(|task| (task.note_id.clone(), task.line))
            .expect("task");
        let _ = app.update(Message::SetTask {
            id: id.clone(),
            line,
            done: true,
        });
        settle(&mut app);
        let milk = app
            .tasks
            .iter()
            .find(|task| task.text.starts_with("buy milk"))
            .expect("still here");
        assert!(!milk.done, "it is not checked off");
        assert_eq!(milk.text, "buy milk 2026-11-28 08:30 repeat:weekly");
        assert!(app.message.contains("next"), "{}", app.message);
        // One that does not repeat is checked as before.
        let (id, line) = app
            .tasks
            .iter()
            .find(|task| task.text == "call")
            .map(|task| (task.note_id.clone(), task.line))
            .expect("task");
        let _ = app.update(Message::SetTask {
            id,
            line,
            done: true,
        });
        settle(&mut app);
        assert!(
            app.tasks
                .iter()
                .any(|task| task.text == "call" && task.done)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_moment_of_a_task_already_there_is_changed_through_the_same_calendar() {
        use super::CalFor;
        let _turn = one_at_a_time();
        let dir = folder("task-edit-date");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::TaskDraft(
            "buy milk 2026-11-28 08:30 repeat:weekly".to_owned(),
        ));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        let (id, line) = app
            .tasks
            .iter()
            .find(|task| task.text.starts_with("buy milk"))
            .map(|task| (task.note_id.clone(), task.line))
            .expect("task");
        let _ = app.update(Message::BeginTaskEdit { id, line });
        assert_eq!(app.edit_due.date, "2026-11-28");
        assert_eq!(app.edit_due.time, "08:30");
        let _ = app.update(Message::OpenCalendar(CalFor::Edit));
        let _ = app.update(Message::PickDay("2026-12-01".to_owned()));
        let _ = app.update(Message::OpenCalendar(CalFor::Edit));
        let _ = app.update(Message::SetRepeat(None));
        drop(app.view());
        let _ = app.update(Message::CommitTaskRename);
        settle(&mut app);
        assert!(
            app.tasks
                .iter()
                .any(|task| task.text == "buy milk 2026-12-01 08:30")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn times_read_on_a_twelve_hour_clock_until_the_person_says_otherwise_and_it_is_kept() {
        use super::{CalFor, due};
        let _turn = one_at_a_time();
        let dir = folder("time-settings");
        let mut app = open_app(&dir);
        wide(&mut app);
        pinned(&mut app, 15 * 60 + 42);
        assert!(app.hour12, "twelve hour is the default");
        assert_eq!(app.clock.time_label(), "3:42 PM");
        let moment = due::Due {
            date: super::dates::Date::new(2026, 11, 21).expect("a day"),
            time: due::Time::new(14, 30),
            repeat: None,
        };
        assert_eq!(app.due_words(&moment, true), "Tomorrow 2:30 PM");
        // The settings page draws with the new section, and a choice takes effect and is kept.
        let _ = app.update(Message::ShowSettings);
        drop(app.view());
        let _ = app.update(Message::PickHour12(false));
        assert_eq!(app.clock.time_label(), "15:42");
        assert_eq!(app.due_words(&moment, true), "Tomorrow 14:30");
        assert!(
            !super::profile::load_profile(&dir).hour12,
            "kept for next time"
        );
        drop(app.view());
        let _ = app.update(Message::PickWeekMonday(true));
        assert!(super::profile::load_profile(&dir).week_monday);
        // What is written on a task does not depend on how it is read.
        assert_eq!(due::Due::write("a", Some(&moment)), "a 2026-11-21 14:30");
        let _ = app.update(Message::PickHour12(true));
        assert_eq!(due::Due::write("a", Some(&moment)), "a 2026-11-21 14:30");
        // Back on the twelve hour clock, the calendar steps the hour 1 to 12 and flips AM and PM.
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        let _ = app.update(Message::SetTime("12:00".to_owned()));
        drop(app.view());
        let _ = app.update(Message::StepTime {
            hours: 12,
            minutes: 0,
        });
        assert_eq!(
            app.new_due.time, "00:00",
            "PM to AM keeps the minute and the hour on the clock face"
        );
        let _ = app.update(Message::StepTime {
            hours: 12,
            minutes: 0,
        });
        assert_eq!(app.new_due.time, "12:00");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_calendar_draws_in_every_setting_and_at_every_width() {
        use super::CalFor;
        let _turn = one_at_a_time();
        let dir = folder("calendar-draw");
        let mut app = open_app(&dir);
        pinned(&mut app, 9 * 60);
        let _ = app.update(Message::ShowTasks);
        for monday in [false, true] {
            for twelve in [false, true] {
                for width in [1100.0, 700.0, 420.0] {
                    let _ = app.update(Message::Resized(iced::Size::new(width, 760.0)));
                    let _ = app.update(Message::PickWeekMonday(monday));
                    let _ = app.update(Message::PickHour12(twelve));
                    for whose in [CalFor::New, CalFor::Edit] {
                        let _ = app.update(Message::OpenCalendar(whose));
                        drop(app.view());
                        let _ = app.update(Message::SetTime("08:05".to_owned()));
                        let _ = app.update(Message::SetRepeat(Some(super::due::Repeat {
                            every: 2,
                            unit: super::due::Unit::Week,
                        })));
                        drop(app.view());
                        let _ = app.update(Message::ClearDue);
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_trip_is_noticed_said_once_and_leaves_the_tasks_alone() {
        let _turn = one_at_a_time();
        let dir = folder("trip");
        let mut app = open_app(&dir);
        wide(&mut app);
        pinned(&mut app, 10 * 60);
        let _ = app.update(Message::ShowTasks);
        app.tasks = vec![task_row("n", 0, false, "call 2026-11-20 09:00")];
        // At home: no notice.
        app.home_zone = "America/Los_Angeles".to_owned();
        app.zone_name = Some("America/Los_Angeles".to_owned());
        assert!(app.trip_banner().is_none());
        // Somewhere else: a notice, and it names both places and what the clock at home says.
        app.zone_name = Some("America/New_York".to_owned());
        app.clock.offset = -5 * 3_600;
        app.trip = Some(super::clock::Trip {
            home_name: "America/Los_Angeles".to_owned(),
            home: Some(super::clock::Clock {
                today: app.clock.today,
                minutes: 7 * 60,
                offset: -8 * 3_600,
                zone: "America/Los_Angeles".to_owned(),
            }),
        });
        let words = app.trip_words(app.trip.as_ref().expect("trip"));
        assert_eq!(
            words,
            "Home is America/Los_Angeles, where it is 7:00 AM now, 3 hours behind here."
        );
        assert!(app.trip_banner().is_some());
        drop(app.view());
        // The task is as it was written, whatever zone this is.
        assert_eq!(app.tasks[0].text, "call 2026-11-20 09:00");
        // Put away, it stays away for this zone, and comes back for another one.
        let _ = app.update(Message::DismissTrip);
        assert!(app.trip_banner().is_none());
        drop(app.view());
        app.zone_name = Some("Asia/Tokyo".to_owned());
        assert!(
            app.trip_banner().is_some(),
            "a different place is a different notice"
        );
        // Making it home ends the trip and is kept.
        let _ = app.update(Message::SetHomeHere);
        assert_eq!(app.home_zone, "Asia/Tokyo");
        assert!(app.trip.is_none() && app.trip_banner().is_none());
        assert_eq!(super::profile::load_profile(&dir).home_zone, "Asia/Tokyo");
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_first_named_zone_is_home_without_asking() {
        let _turn = one_at_a_time();
        let dir = folder("home-zone");
        let mut app = open_app(&dir);
        assert!(app.home_zone.is_empty());
        app.zone_name = Some("Europe/London".to_owned());
        app.check_trip();
        assert_eq!(app.home_zone, "Europe/London");
        assert!(app.trip.is_none());
        assert_eq!(
            super::profile::load_profile(&dir).home_zone,
            "Europe/London"
        );
        // A computer that names no zone is not home anywhere, and nothing is made up.
        let mut blank = open_app(&folder("home-zone-blank"));
        blank.zone_name = None;
        blank.check_trip();
        assert!(blank.home_zone.is_empty() && blank.trip.is_none());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(folder("home-zone-blank"));
    }

    #[test]
    fn the_right_panel_sits_beside_the_page_only_when_there_is_room_and_is_a_sheet_otherwise() {
        let _turn = one_at_a_time();
        let dir = folder("panel-modes");
        let mut app = open_app(&dir);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        let _ = app.update(Message::Resized(iced::Size::new(1300.0, 800.0)));
        let _ = app.update(Message::ToggleContext);
        assert!(app.show_context && app.side_by_side_context());
        assert!(
            app.context_sheet().is_none(),
            "beside the page, not over it"
        );
        let wide_page = app.pane_inner();
        drop(app.view());
        // Narrowing past the point where it fits closes it, and does not put it over the writing.
        let _ = app.update(Message::Resized(iced::Size::new(900.0, 800.0)));
        assert!(!app.side_by_side_context() && !app.show_context);
        assert!(app.context_sheet().is_none());
        drop(app.view());
        // Opened on purpose at this width it is a sheet, and the page keeps all its width.
        let _ = app.update(Message::ToggleContext);
        assert!(app.context_sheet().is_some());
        assert!(
            app.pane_inner() > 400.0,
            "the page was not squeezed: {}",
            app.pane_inner()
        );
        assert!(wide_page < 1300.0);
        drop(app.view());
        let _ = app.update(Message::ToggleContext);
        assert!(app.context_sheet().is_none());
        // A narrow window has it as a sheet too, but only over a note, not over the list.
        let _ = app.update(Message::Resized(iced::Size::new(420.0, 760.0)));
        let _ = app.update(Message::ToggleContext);
        drop(app.view());
        assert_eq!(app.context_sheet().is_some(), app.detail);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_question_is_a_centered_card_at_every_width_and_the_calendar_is_one_on_a_narrow_screen() {
        use super::CalFor;
        let _turn = one_at_a_time();
        let dir = folder("modals");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        let _ = app.update(Message::ShowTasks);
        // The calendar is under its button on a wide window, so it is not a card over the page.
        let _ = app.update(Message::Resized(iced::Size::new(1100.0, 760.0)));
        let _ = app.update(Message::OpenCalendar(CalFor::New));
        assert!(app.modal().is_none(), "anchored under the Due button");
        drop(app.view());
        // On a narrow one it is the standard card, with the page dimmed behind it.
        let _ = app.update(Message::Resized(iced::Size::new(420.0, 760.0)));
        assert!(app.modal().is_some());
        drop(app.view());
        let _ = app.update(Message::CloseCalendar);
        assert!(app.modal().is_none());
        // Clearing done tasks.
        app.tasks = vec![task_row("n", 0, true, "finished")];
        let _ = app.update(Message::AskClearDone);
        assert!(app.modal().is_some());
        drop(app.view());
        let _ = app.update(Message::CancelClearDone);
        // Deleting and sending are cards over the page, not buttons in its corner.
        let _ = app.update(Message::ShowNotes);
        let _ = app.update(Message::AskDelete);
        assert!(app.modal().is_some());
        drop(app.view());
        let _ = app.update(Message::CancelDelete);
        assert!(app.modal().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_tab_is_a_list_then_the_thing_chosen_with_a_way_back_on_a_narrow_window() {
        use super::{Screen, TaskView};
        let _turn = one_at_a_time();
        let dir = folder("two-steps");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        app.catalog = vec![summary("a", &[]), summary("b", &[])];
        let _ = app.update(Message::Resized(iced::Size::new(420.0, 760.0)));
        // Tasks: the list is the views. Choosing one opens its tasks, and Back is the list again.
        let _ = app.update(Message::ShowTasks);
        assert!(app.screen == Screen::Tasks && !app.detail);
        drop(app.view());
        let _ = app.update(Message::ShowTaskView(TaskView::Today));
        assert!(app.detail, "its tasks");
        drop(app.view());
        let _ = app.update(Message::BackToList);
        assert!(!app.detail);
        drop(app.view());
        // The Tasks tab itself always lands on the list.
        let _ = app.update(Message::ShowTaskView(TaskView::Open));
        let _ = app.update(Message::ShowTasks);
        assert!(!app.detail);
        // Chat: a conversation, and Back to the list of them.
        let _ = app.update(Message::ShowChat);
        assert!(app.screen == Screen::Chat && !app.detail);
        app.chat_id = Some("c".to_owned());
        app.detail = true;
        drop(app.view());
        let _ = app.update(Message::BackToList);
        assert!(!app.detail);
        drop(app.view());
        // Notes: the graph is opened from the list, covers it, and Back closes it.
        let _ = app.update(Message::ShowNotes);
        assert!(!app.detail);
        let _ = app.update(Message::ToggleGraph);
        assert!(app.graph_open && app.detail, "the list is out of the way");
        drop(app.view());
        let _ = app.update(Message::BackToList);
        assert!(!app.graph_open && !app.detail);
        // Choosing a note from the graph opens it, and the graph is put away.
        let _ = app.update(Message::ToggleGraph);
        let _ = app.update(Message::GraphPick("a".to_owned()));
        assert!(!app.graph_open && app.detail);
        drop(app.view());
        // On a wide window none of this changes what is shown.
        let _ = app.update(Message::Resized(iced::Size::new(1300.0, 800.0)));
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_screen_draws_at_every_width_with_every_popup_open() {
        let _turn = one_at_a_time();
        let dir = folder("sweep");
        let mut app = open_app(&dir);
        pinned(&mut app, 10 * 60);
        app.catalog = vec![summary("a", &["Novel"]), summary("b", &[])];
        app.trip = Some(super::clock::Trip {
            home_name: "America/Los_Angeles".to_owned(),
            home: None,
        });
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        for width in [
            1400.0, 1100.0, 1019.0, 900.0, 760.0, 719.0, 600.0, 420.0, 340.0,
        ] {
            let _ = app.update(Message::Resized(iced::Size::new(width, 760.0)));
            for screen in [
                Message::ShowNotes,
                Message::ShowTasks,
                Message::ShowChat,
                Message::ShowSettings,
                Message::ShowInvite,
                Message::ShowNotes,
            ] {
                let _ = app.update(screen);
                drop(app.view());
                let _ = app.update(Message::ToggleContext);
                drop(app.view());
                let _ = app.update(Message::ToggleNoteSettings);
                drop(app.view());
                let _ = app.update(Message::ToggleTaskCite);
                let _ = app.update(Message::ToggleChatCite);
                drop(app.view());
                let _ = app.update(Message::ToggleTaskCite);
                let _ = app.update(Message::ToggleChatCite);
                let _ = app.update(Message::ToggleGraph);
                drop(app.view());
                let _ = app.update(Message::ToggleGraph);
                let _ = app.update(Message::ToggleFocus);
                drop(app.view());
                let _ = app.update(Message::ToggleFocus);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_new_icons_are_drawn_where_they_should_be() {
        use super::{plus_ink, quote_ink, send_ink};
        assert!(plus_ink(0.0, 10.0) && plus_ink(-10.0, 0.0) && !plus_ink(10.0, 10.0));
        assert!(quote_ink(0.0, 0.0) && quote_ink(14.0, 0.0) && quote_ink(-14.0, 5.0));
        assert!(!quote_ink(0.0, 10.0) && !quote_ink(30.0, 0.0));
        assert!(
            send_ink(0.0, 0.0) && send_ink(18.0, 0.0),
            "the body and the tip"
        );
        assert!(!send_ink(-14.0, 0.0), "the notch in the back");
        assert!(!send_ink(25.0, 0.0) && !send_ink(0.0, 25.0));
        for kind in [super::Mini::Plus, super::Mini::Quote, super::Mini::Send] {
            for dark in [true, false] {
                drop(super::mini_handle(kind, dark));
            }
        }
    }

    #[test]
    fn the_settings_icon_switches_the_right_panel_and_back() {
        let _turn = one_at_a_time();
        let dir = folder("note-settings");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        assert!(!app.note_panel, "links and backlinks are the default");
        drop(app.view());
        let _ = app.update(Message::ToggleNoteSettings);
        assert!(
            app.note_panel && app.show_context,
            "the panel opens on settings"
        );
        drop(app.view());
        let _ = app.update(Message::ToggleNoteSettings);
        assert!(!app.note_panel);
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_row_shows_delete_under_the_pointer_and_asks_before_deleting() {
        let _turn = one_at_a_time();
        let dir = folder("row-delete");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("First".to_owned()));
        let _ = app.update(Message::Save);
        let id = app.selected.clone();
        let Some(id) = id else {
            // The save is a task in the window; nothing to delete in this harness.
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        let _ = app.update(Message::RowHover(id.clone(), true));
        assert_eq!(app.hover_row.as_deref(), Some(id.as_str()));
        drop(app.view());
        let _ = app.update(Message::RowHover("other".to_owned(), false));
        assert_eq!(
            app.hover_row.as_deref(),
            Some(id.as_str()),
            "another row leaving changes nothing"
        );
        let _ = app.update(Message::RowHover(id.clone(), false));
        assert!(app.hover_row.is_none());
        let _ = app.update(Message::AskDeleteNote(id));
        assert!(app.confirm_delete, "it asks first");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slide_eases_between_hidden_and_shown() {
        use super::Slide;
        assert!(Slide::settled(false).shown() < f32::EPSILON);
        assert!((Slide::settled(true).shown() - 1.0).abs() < f32::EPSILON);
        assert!(!Slide::settled(true).moving());
        let mut slide = Slide::settled(false);
        slide.set(true);
        assert!(slide.moving());
        let now = slide.shown();
        assert!((0.0..=1.0).contains(&now), "{now}");
        // Setting it to what it already is does not restart it.
        let before = slide.since;
        slide.set(true);
        assert_eq!(slide.since, before);
        slide.jump(false);
        assert!(slide.shown() < f32::EPSILON && !slide.moving());
    }

    #[test]
    fn a_word_is_added_to_the_dictionary_from_the_caret() {
        let _turn = one_at_a_time();
        let dir = folder("dictionary");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.body.load("a zzquuxword here");
        app.body.place_byte(5);
        let _ = app.update(Message::AddWord);
        assert!(app.my_words.contains("zzquuxword"), "{}", app.message);
        assert!(app.message.contains("zzquuxword"));
        assert_eq!(super::profile::load_words(&dir), ["zzquuxword"]);

        // Again says so, and does not write twice.
        let _ = app.update(Message::AddWord);
        assert!(app.message.contains("already"), "{}", app.message);
        assert_eq!(app.my_words.len(), 1);

        // Between words there is nothing to add.
        app.body.place_byte(1);
        let _ = app.update(Message::AddWord);
        assert!(app.message.contains("cursor"), "{}", app.message);
        assert_eq!(app.my_words.len(), 1);

        // A new session reads the file back.
        let (again, _) = App::at(dir.clone());
        assert!(again.my_words.contains("zzquuxword"));
        super::markdown::set_personal_words(&[]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_dictionary_file_ignores_what_is_not_a_word() {
        let dir = folder("dictionary-file");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join("dictionary"),
            "good\nNo\nab\nhas space\n123\nfine'ish\n\n",
        )
        .expect("write");
        assert_eq!(super::profile::load_words(&dir), ["good", "fine'ish"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn erasing_the_notebook_erases_the_dictionary_too() {
        let _turn = one_at_a_time();
        let dir = folder("wipe-dictionary");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.body.load("a zzquuxwipe here");
        app.body.place_byte(5);
        let _ = app.update(Message::AddWord);
        assert!(dir.join("dictionary").is_file());
        assert!(app.my_words.contains("zzquuxwipe"));

        let _ = app.update(Message::WipeDraft("delete".to_owned()));
        let _ = app.update(Message::ConfirmWipe);
        assert!(!dir.join("dictionary").exists(), "the file is gone");
        assert!(app.my_words.is_empty(), "and so are the words in memory");
        // A word added to the next notebook does not bring the old one back.
        assert!(super::profile::load_words(&dir).is_empty());
        super::markdown::set_personal_words(&[]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_backup_copies_the_folder_remembers_when_and_can_be_opened() {
        let _turn = one_at_a_time();
        let dir = folder("backup-flow");
        let mut app = open_app(&dir);
        let id = Notebook::open(&dir)
            .expect("open")
            .add("Plan", "buy oats")
            .expect("add");
        let place = folder("backup-flow-usb");
        let _ = app.update(Message::BackupTo(place.display().to_string()));

        let _ = app.update(Message::BackUp);
        assert!(app.backup_busy, "it is working");
        assert_eq!(app.message, "Backing up.");
        // A second press while it works does nothing.
        let _ = app.update(Message::BackUp);
        assert!(app.backup_busy);

        // What the worker thread does, and its answer.
        let report =
            super::profile::backup_folder(&app.dir, &place, "aerodash-test").expect("the copy");
        let _ = app.update(Message::BackedUp(Ok(report.clone())));
        assert!(!app.backup_busy);
        assert!(app.backup_at > 0, "the time is remembered");
        assert!(app.message.contains("Backed up") && app.message.contains("aerodash-test"));
        let saved = super::profile::load_profile(&dir);
        assert_eq!(saved.backup_at, app.backup_at, "and kept for next time");
        assert_eq!(saved.backup_to, place.display().to_string());

        // The copy is a whole notebook.
        let copy = Notebook::open(&report.path).expect("the copy opens");
        assert_eq!(copy.show(&id).expect("the note").heads[0].body, "buy oats");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&place);
    }

    #[test]
    fn a_failed_backup_says_why_and_changes_nothing_else() {
        let _turn = one_at_a_time();
        let dir = folder("backup-fail");
        let mut app = open_app(&dir);
        let _ = app.update(Message::BackupTo(dir.join("inside").display().to_string()));
        let _ = app.update(Message::BackUp);
        assert!(app.backup_busy);
        let refused = super::profile::backup_folder(&app.dir, &dir.join("inside"), "copy");
        let _ = app.update(Message::BackedUp(refused));
        assert!(!app.backup_busy, "it can be tried again");
        assert!(app.message.contains("outside"), "{}", app.message);
        assert_eq!(app.backup_at, 0, "no backup is claimed");
        assert!(
            !dir.join("inside").exists(),
            "nothing was left in the notebook"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_typed_a_moment_ago_is_saved_before_the_copy_is_made() {
        let _turn = one_at_a_time();
        let dir = folder("backup-dirty");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("Not saved yet".to_owned()));
        assert!(app.editor_dirty());
        let _ = app.update(Message::BackUp);
        assert!(!app.backup_busy, "the copy waits for the save");
        assert!(app.message.contains("Saving"), "{}", app.message);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_backup_card_draws_with_and_without_a_passphrase() {
        let _turn = one_at_a_time();
        let dir = folder("backup-card");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::ShowSettings);
        assert!(!ambient_cli::folder_key_is_wrapped(&dir));
        drop(app.view());
        ambient_cli::set_folder_unlock(&dir, "correct horse").expect("lock");
        assert!(ambient_cli::folder_key_is_wrapped(&dir));
        drop(app.view());
        // And after a copy, and while one is running.
        let _ = app.update(Message::BackedUp(Err("no".to_owned())));
        app.backup_at = 1_700_000_000_000;
        drop(app.view());
        app.backup_busy = true;
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sizes_read_as_people_say_them() {
        assert_eq!(super::size_label(0), "0 bytes");
        assert_eq!(super::size_label(812), "812 bytes");
        assert_eq!(super::size_label(1024), "1 KB");
        assert_eq!(super::size_label(340 * 1024), "340 KB");
        assert_eq!(super::size_label(3 * 1024 * 1024 + 450_000), "3.4 MB");
        assert_eq!(super::size_label(1_300_000_000), "1.2 GB");
    }

    #[test]
    fn a_typed_place_can_start_with_the_home_folder() {
        use std::path::PathBuf;
        let home = Some(PathBuf::from("/Users/me"));
        let expand = |typed: &str| super::profile::expand_home_in(typed, home.clone());
        assert_eq!(expand("/media/usb"), PathBuf::from("/media/usb"));
        assert_eq!(expand("backups"), PathBuf::from("backups"));
        assert_eq!(expand("~/Backups"), PathBuf::from("/Users/me/Backups"));
        assert_eq!(expand("~"), PathBuf::from("/Users/me"));
        assert_eq!(expand("~other/x"), PathBuf::from("~other/x"));
        // With no home folder a `~` is left as it was typed.
        assert_eq!(
            super::profile::expand_home_in("~/x", None),
            PathBuf::from("~/x")
        );
        assert_eq!(
            super::profile::backup_dir_in(home),
            PathBuf::from("/Users/me/Aerodash backups")
        );
        assert_eq!(
            super::profile::backup_dir_in(None),
            PathBuf::from("Aerodash backups")
        );
    }

    /// Type `text` the way a person does: one key at a time through `update`, with the pause that
    /// follows a key allowed to fire now and then. After every key the page must hold exactly what
    /// was typed and the caret must sit just after it.
    fn type_through(app: &mut App, text: &str, wake_every: usize) {
        let mut typed = String::new();
        for (index, ch) in text.chars().enumerate() {
            let action = if ch == '\n' {
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Enter)
            } else {
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(ch))
            };
            let _ = app.update(Message::BodyEdited(action));
            typed.push(ch);
            if wake_every > 0 && index % wake_every == wake_every - 1 {
                let _ = app.update(Message::Woke(super::Wake::Pause(app.pause_token)));
                settle(app);
            }
            let shown = app.body.text();
            assert_eq!(
                shown, typed,
                "after {index} keys, the page lost or moved a character"
            );
            assert_eq!(
                app.body.caret_byte(),
                typed.len(),
                "after {index} keys ({:?}), the caret is not just after what was typed",
                &typed[typed.len().saturating_sub(12)..]
            );
        }
    }

    #[test]
    fn fast_typing_keeps_every_character_and_the_caret_at_the_end() {
        let _turn = one_at_a_time();
        let dir = folder("typing");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        let passage = "The **quick** brown fox jumpd over ==the== lazy dog, and ~~then~~ it did not.\n\
A second paragraph with a [link](https://example.com/a) in it, and a # that is not a heading.\n\
Some `code` and *an em* and __strong__ words. Done.\n- last, a list item with no Enter after it";
        type_through(&mut app, passage, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn typing_while_saves_come_back_keeps_every_character() {
        let _turn = one_at_a_time();
        let dir = folder("typing-saves");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        let passage = "A line of words typed with a pause that fires every few keys, so a save is \
always on its way back while the next letters arrive. **Bold ones** too, and a misspeling.";
        type_through(&mut app, passage, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_nothing_took_is_typed_and_a_shortcut_is_not() {
        use iced::keyboard::key::Named;
        use iced::keyboard::{Key, Modifiers};
        let plain = Modifiers::empty();
        let letter =
            |text: &str| super::ignored_key(&Key::Character(text.into()), Some(text), plain);
        assert_eq!(letter("a"), Some(super::Typed::Text("a".to_owned())));
        assert_eq!(letter("é"), Some(super::Typed::Text("é".to_owned())));
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Space), None, plain),
            Some(super::Typed::Text(" ".to_owned()))
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Enter), Some("\r"), plain),
            Some(super::Typed::Enter)
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Backspace), None, plain),
            Some(super::Typed::Backspace)
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Delete), None, plain),
            Some(super::Typed::Delete)
        );
        // A key that types nothing, and a key with a shortcut modifier, are left alone.
        assert_eq!(
            super::ignored_key(&Key::Named(Named::ArrowLeft), None, plain),
            None
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::F5), None, plain),
            None
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Shift), None, plain),
            None
        );
        for held in [Modifiers::CTRL, Modifiers::LOGO, Modifiers::ALT] {
            assert_eq!(
                super::ignored_key(&Key::Character("b".into()), Some("b"), held),
                None
            );
        }
        // A control character in the text is not typed.
        assert_eq!(
            super::ignored_key(&Key::Character("x".into()), Some("\u{7}"), plain),
            None
        );
    }

    #[test]
    fn a_key_the_editor_dropped_is_typed_into_the_page_in_order() {
        let _turn = one_at_a_time();
        let dir = folder("dropped-keys");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        for typed in [
            super::Typed::Text("hello".to_owned()),
            super::Typed::Text(" ".to_owned()),
            super::Typed::Text("wor".to_owned()),
            super::Typed::Backspace,
            super::Typed::Text("rld".to_owned()),
            super::Typed::Enter,
            super::Typed::Text("next".to_owned()),
        ] {
            let _ = app.update(Message::TypedIgnored(typed));
        }
        assert_eq!(app.body.text(), "hello world\nnext");
        assert_eq!(app.body.caret_byte(), app.body.text().len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dropped_key_is_not_typed_when_something_else_is_up() {
        let _turn = one_at_a_time();
        let dir = folder("dropped-keys-guard");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        let typed = || super::Typed::Text("x".to_owned());
        // The settings page, another tab, and the delete question each have their own keys.
        let _ = app.update(Message::ShowSettings);
        let _ = app.update(Message::TypedIgnored(typed()));
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::TypedIgnored(typed()));
        let _ = app.update(Message::ShowNotes);
        app.confirm_delete = true;
        let _ = app.update(Message::TypedIgnored(typed()));
        app.confirm_delete = false;
        assert_eq!(
            app.body.text(),
            "",
            "a key was typed into a page that was not up"
        );
        // And with the page up, it is.
        let _ = app.update(Message::TypedIgnored(typed()));
        assert_eq!(app.body.text(), "x");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
