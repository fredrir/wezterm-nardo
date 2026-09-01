use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{
    self as term, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, KeyCode, KeyEventKind,
};
use crossterm::execute;
use ratatui::backend::{Backend, TestBackend};
use ratatui::layout::Rect;
use ratatui::{DefaultTerminal, Terminal};
use serde::{Deserialize, Serialize};

use crate::app::{App, Cx, Flow, Outcome};
use crate::context::Context;
use crate::event::Event;
use crate::keys::{FORWARD_PREFIX, ScriptToken, printable};
use crate::log::Logger;
use crate::ui::fx::Effects;
use crate::ui::theme::Theme;
use crate::wezterm::{Action, DEFAULT_USERVAR, ROLE, Wezterm, action_payload, user_var};

pub struct RunOptions {
    pub context: Arc<Context>,
    pub wezterm: Arc<dyn Wezterm>,
    pub headless: Option<Headless>,
}

pub struct Headless {
    pub size: (u16, u16),
    pub script: Vec<ScriptToken>,
    pub dump: bool,
}

/// Printed as json in headless mode, see docs/protocol.md "Outcome json".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    #[serde(flatten)]
    pub outcome: Outcome,
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub snapshot: serde_json::Value,
}

/// Everything the loop waits on: terminal input and job results share one channel.
pub(crate) enum Inbox<M> {
    Input(term::Event),
    Msg(M),
}

/// Interactive: raw mode + alternate screen + mouse capture, restored on exit and on panic.
/// Headless: `TestBackend`, feeds the script (settling jobs between tokens), never touches the tty.
/// Both: `init` → loop { event → update → view } → on exit kill own pane (interactive, not headless).
pub fn run<A: App>(app: A, opts: RunOptions) -> anyhow::Result<Report> {
    let RunOptions { context, wezterm, headless } = opts;
    match headless {
        Some(headless) => run_headless(app, context, wezterm, headless),
        None => run_interactive(app, context, wezterm),
    }
}

/// Max wait for `settle`; a fake wezterm answers in ms, a real one in tens of ms.
pub const SETTLE_TIMEOUT_MS: u64 = 5_000;

const IDLE_TIMEOUT: Duration = Duration::from_millis(250);
const POLL: Duration = Duration::from_millis(2);
const CLOSE_CAP: Duration = Duration::from_millis(150);
const ACTION_GRACE: Duration = Duration::from_millis(20);

struct Runner<A: App> {
    app: A,
    cx: Cx<A::Msg>,
    rx: Receiver<Inbox<A::Msg>>,
    forward_pending: bool,
    fx_clock: Option<Instant>,
    exit: Option<Outcome>,
}

impl<A: App> Runner<A> {
    fn new(app: A, context: Arc<Context>, wezterm: Arc<dyn Wezterm>, headless: bool, area: Rect) -> Self {
        let (jobs, rx) = mpsc::channel();
        let presentation = context.presentation.clone();
        let cx = Cx {
            theme: Theme::from_spec(&context.theme),
            fx: Effects::new(presentation.animations && !headless),
            presentation,
            context,
            wezterm,
            area,
            headless,
            own_pane: Context::own_pane_id(),
            jobs,
            in_flight: Arc::new(AtomicUsize::new(0)),
            actions: Vec::new(),
            redraw: false,
            log: Logger::from_env(),
        };
        Self { app, cx, rx, forward_pending: false, fx_clock: None, exit: None }
    }

    fn init(&mut self) {
        let Rect { width, height, .. } = self.cx.area;
        self.cx.log(format!("{} start {width}x{height} headless={}", self.app.name(), self.cx.headless));
        self.app.init(&mut self.cx);
    }

    fn dispatch(&mut self, event: Event<A::Msg>) {
        if self.exit.is_some() {
            return;
        }
        if let Flow::Exit(outcome) = self.app.update(event, &mut self.cx) {
            self.cx.log(format!("exit {outcome:?}"));
            self.exit = Some(outcome);
        }
    }

    fn input(&mut self, event: term::Event) {
        match event {
            term::Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return;
                }
                if std::mem::take(&mut self.forward_pending)
                    && let KeyCode::Char(c) = key.code
                {
                    self.dispatch(Event::Forwarded(c));
                } else if key.code == KeyCode::Char(FORWARD_PREFIX) {
                    self.forward_pending = true;
                } else {
                    self.dispatch(Event::Key(key));
                }
            }
            term::Event::Mouse(mouse) => self.dispatch(Event::Mouse(mouse)),
            term::Event::Paste(text) => self.dispatch(Event::Paste(text)),
            term::Event::Resize(cols, rows) => {
                self.cx.area = Rect::new(0, 0, cols, rows);
                self.dispatch(Event::Resize(cols, rows));
            }
            term::Event::FocusGained => self.dispatch(Event::Focus(true)),
            term::Event::FocusLost => self.dispatch(Event::Focus(false)),
        }
    }

    fn inbox(&mut self, item: Inbox<A::Msg>) {
        match item {
            Inbox::Input(event) => self.input(event),
            Inbox::Msg(msg) => self.dispatch(Event::Msg(msg)),
        }
    }

    /// One frame: `Tick` (only while effects run) → `view` → effects on `cx.area`.
    fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        let now = Instant::now();
        let elapsed = self.fx_clock.map_or(Duration::ZERO, |t| now.duration_since(t));
        if self.cx.fx.running() {
            self.dispatch(Event::Tick(elapsed));
        }
        let Self { app, cx, .. } = self;
        terminal.draw(|frame| {
            cx.area = frame.area();
            app.view(frame, cx);
            let area = cx.area;
            cx.fx.render(frame, area, elapsed);
        })?;
        self.fx_clock = self.cx.fx.running().then_some(now);
        self.cx.redraw = false;
        Ok(())
    }

    /// Feeds job results to `update` until nothing is in flight (or `SETTLE_TIMEOUT_MS`).
    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_millis(SETTLE_TIMEOUT_MS);
        loop {
            while let Ok(item) = self.rx.try_recv() {
                self.inbox(item);
            }
            if self.exit.is_some() {
                return;
            }
            if self.cx.jobs_in_flight() == 0 {
                match self.rx.try_recv() {
                    Ok(item) => self.inbox(item),
                    Err(_) => return,
                }
                continue;
            }
            if Instant::now() >= deadline {
                self.cx.log(format!("settle: timeout with {} job(s) in flight", self.cx.jobs_in_flight()));
                return;
            }
            if let Ok(item) = self.rx.recv_timeout(POLL) {
                self.inbox(item);
            }
        }
    }

    /// Waits for running jobs after exit without delivering their results.
    fn drain_jobs(&mut self) {
        let deadline = Instant::now() + Duration::from_millis(SETTLE_TIMEOUT_MS);
        while self.cx.jobs_in_flight() > 0 && Instant::now() < deadline {
            let _ = self.rx.recv_timeout(POLL);
        }
    }
}

fn run_headless<A: App>(
    app: A,
    context: Arc<Context>,
    wezterm: Arc<dyn Wezterm>,
    headless: Headless,
) -> anyhow::Result<Report> {
    let (cols, rows) = headless.size;
    let mut terminal = Terminal::new(TestBackend::new(cols, rows))?;
    let mut runner = Runner::new(app, context, wezterm, true, Rect::new(0, 0, cols, rows));
    runner.init();
    runner.settle();
    runner.draw(&mut terminal)?;

    for token in headless.script {
        if runner.exit.is_some() {
            break;
        }
        match token {
            ScriptToken::Settle => runner.settle(),
            ScriptToken::Key(key) => {
                let plain_char = printable(&key).is_some();
                runner.input(term::Event::Key(key));
                runner.draw(&mut terminal)?;
                if !plain_char {
                    runner.settle();
                }
            }
            ScriptToken::Mouse(mouse) => {
                runner.input(term::Event::Mouse(mouse));
                runner.draw(&mut terminal)?;
                runner.settle();
            }
        }
    }
    if runner.exit.is_none() {
        runner.settle();
        runner.draw(&mut terminal)?;
    }
    runner.drain_jobs();

    let outcome = runner.exit.take().unwrap_or_else(Outcome::open);
    let snapshot = if headless.dump { runner.app.snapshot() } else { serde_json::Value::Null };
    Ok(Report { outcome, actions: std::mem::take(&mut runner.cx.actions), snapshot })
}

fn run_interactive<A: App>(app: A, context: Arc<Context>, wezterm: Arc<dyn Wezterm>) -> anyhow::Result<Report> {
    let (mut terminal, tty) = Tty::enter()?;
    let size = terminal.size()?;
    let mut runner = Runner::new(app, context, wezterm, false, Rect::new(0, 0, size.width, size.height));
    let mut emitter = Emitter::from_env();
    emitter.write_raw(&user_var(&format!("{}_role", emitter.name), ROLE))?;
    spawn_input_thread(runner.cx.jobs.clone());

    runner.init();
    runner.draw(&mut terminal)?;
    emitter.flush(&mut runner.cx)?;

    let outcome = loop {
        if let Some(outcome) = runner.exit.take() {
            break outcome;
        }
        let awake = runner.cx.fx.running() || runner.cx.redraw;
        let timeout = if awake { runner.cx.tick_rate() } else { IDLE_TIMEOUT };
        match runner.rx.recv_timeout(timeout) {
            Ok(item) => runner.inbox(item),
            Err(RecvTimeoutError::Timeout) if !awake => continue,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => anyhow::bail!("event channel closed"),
        }
        while let Ok(item) = runner.rx.try_recv() {
            runner.inbox(item);
        }
        runner.draw(&mut terminal)?;
        emitter.flush(&mut runner.cx)?;
    };

    if runner.cx.animations() {
        let theme = runner.cx.theme;
        runner.cx.fx.close(&theme);
        let deadline = Instant::now() + CLOSE_CAP;
        while runner.cx.fx.closing() && Instant::now() < deadline {
            runner.draw(&mut terminal)?;
            if let Err(RecvTimeoutError::Disconnected) = runner.rx.recv_timeout(runner.cx.tick_rate()) {
                break;
            }
        }
    }
    runner.drain_jobs();
    drop(tty);
    emitter.flush(&mut runner.cx)?;

    if let Some(own) = runner.cx.own_pane {
        if !emitter.sent.is_empty() {
            thread::sleep(ACTION_GRACE);
        }
        if let Err(err) = runner.cx.wezterm.kill_pane(own) {
            runner.cx.log(format!("kill-pane {own}: {err}"));
        }
    }
    Ok(Report { outcome, actions: emitter.sent, snapshot: serde_json::Value::Null })
}

fn spawn_input_thread<M: Send + 'static>(tx: mpsc::Sender<Inbox<M>>) {
    thread::spawn(move || {
        while let Ok(event) = term::read() {
            if tx.send(Inbox::Input(event)).is_err() {
                break;
            }
        }
    });
}

/// Writes queued actions as user-var OSCs to stdout, numbered so equal payloads still fire.
struct Emitter {
    name: String,
    n: u64,
    sent: Vec<Action>,
}

impl Emitter {
    fn from_env() -> Self {
        let name = std::env::var("WEZPLUG_USERVAR")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_USERVAR.to_string());
        Self { name, n: 0, sent: Vec::new() }
    }

    fn write_raw(&self, osc: &str) -> io::Result<()> {
        let mut out = io::stdout().lock();
        out.write_all(osc.as_bytes())?;
        out.flush()
    }

    fn flush<M>(&mut self, cx: &mut Cx<M>) -> io::Result<()> {
        if cx.actions.is_empty() {
            return Ok(());
        }
        let mut out = io::stdout().lock();
        for action in std::mem::take(&mut cx.actions) {
            self.n += 1;
            out.write_all(user_var(&self.name, &action_payload(&action, self.n)).as_bytes())?;
            self.sent.push(action);
        }
        out.flush()
    }
}

/// Alternate screen + raw mode + mouse/paste/focus reporting; undone on drop and on panic.
struct Tty;

impl Tty {
    fn enter() -> anyhow::Result<(DefaultTerminal, Tty)> {
        let terminal = ratatui::try_init()?;
        let guard = Tty;
        execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste, EnableFocusChange)?;
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            Tty::disable_reporting();
            previous(info);
        }));
        Ok((terminal, guard))
    }

    fn disable_reporting() {
        let _ = execute!(io::stdout(), DisableFocusChange, DisableBracketedPaste, DisableMouseCapture);
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        Tty::disable_reporting();
        ratatui::restore();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use ratatui::Frame;
    use ratatui::widgets::Paragraph;
    use serde_json::json;

    use super::*;
    use crate::context::{PaneId, TabId, WindowId};
    use crate::keys::parse_script;
    use crate::wezterm::{NewTabTarget, PaneRecord, Result as WeztermResult, SpawnSpec, SplitDirection};

    const LIST: &str = r#"[
      {"window_id":10,"tab_id":30,"pane_id":46,"workspace":"default","title":"zsh","is_active":true},
      {"window_id":10,"tab_id":30,"pane_id":47,"workspace":"default","title":"vim"}
    ]"#;

    #[derive(Default)]
    struct FakeWezterm {
        calls: Mutex<Vec<String>>,
    }

    impl FakeWezterm {
        fn record(&self, call: impl Into<String>) -> WeztermResult<()> {
            self.calls.lock().expect("calls lock").push(call.into());
            Ok(())
        }
    }

    impl Wezterm for FakeWezterm {
        fn list(&self) -> WeztermResult<Vec<PaneRecord>> {
            self.record("list")?;
            Ok(serde_json::from_str(LIST).expect("fixture parses"))
        }
        fn get_text(&self, pane: PaneId, lines: Option<u32>, escapes: bool) -> WeztermResult<String> {
            self.record(format!("get-text {pane} {lines:?} {escapes}"))?;
            Ok("$ ls\n".into())
        }
        fn activate_pane(&self, pane: PaneId) -> WeztermResult<()> {
            self.record(format!("activate-pane {pane}"))
        }
        fn activate_tab(&self, tab: TabId) -> WeztermResult<()> {
            self.record(format!("activate-tab {tab}"))
        }
        fn kill_pane(&self, pane: PaneId) -> WeztermResult<()> {
            self.record(format!("kill-pane {pane}"))
        }
        fn move_pane_to_new_tab(&self, pane: PaneId, _: NewTabTarget) -> WeztermResult<()> {
            self.record(format!("move-pane-to-new-tab {pane}"))
        }
        fn move_pane_into_split(&self, pane: PaneId, _: PaneId, _: SplitDirection) -> WeztermResult<()> {
            self.record(format!("split-pane --move-pane-id {pane}"))
        }
        fn split_pane(&self, pane: PaneId, _: SplitDirection, _: Option<&str>) -> WeztermResult<PaneId> {
            self.record(format!("split-pane {pane}")).map(|()| 100)
        }
        fn spawn(&self, _: &SpawnSpec) -> WeztermResult<PaneId> {
            self.record("spawn").map(|()| 101)
        }
        fn set_tab_title(&self, tab: TabId, title: &str) -> WeztermResult<()> {
            self.record(format!("set-tab-title {tab} {title}"))
        }
        fn set_window_title(&self, window: WindowId, title: &str) -> WeztermResult<()> {
            self.record(format!("set-window-title {window} {title}"))
        }
        fn rename_workspace(&self, workspace: &str, new_name: &str) -> WeztermResult<()> {
            self.record(format!("rename-workspace {workspace} {new_name}"))
        }
        fn zoom_pane(&self, pane: PaneId, _: Option<bool>) -> WeztermResult<()> {
            self.record(format!("zoom-pane {pane}"))
        }
    }

    enum Msg {
        Listed(usize),
        Text(String),
    }

    /// Lists on init, then fetches text for the first pane (a chained job), so `settle` must
    /// follow the second hop before the first frame.
    #[derive(Default)]
    struct Probe {
        panes: usize,
        text: Option<String>,
        typed: String,
        forwarded: Vec<char>,
        mouse: usize,
        ticks: usize,
    }

    impl App for Probe {
        type Msg = Msg;

        fn name(&self) -> &'static str {
            "probe"
        }

        fn init(&mut self, cx: &mut Cx<Msg>) {
            let wezterm = Arc::clone(&cx.wezterm);
            cx.spawn(move || Msg::Listed(wezterm.list().map(|l| l.len()).unwrap_or(0)));
        }

        fn update(&mut self, event: Event<Msg>, cx: &mut Cx<Msg>) -> Flow {
            match event {
                Event::Msg(Msg::Listed(n)) => {
                    self.panes = n;
                    let wezterm = Arc::clone(&cx.wezterm);
                    cx.spawn(move || Msg::Text(wezterm.get_text(46, Some(10), true).unwrap_or_default()));
                }
                Event::Msg(Msg::Text(text)) => self.text = Some(text),
                Event::Key(key) if key.code == KeyCode::Enter => {
                    cx.emit(Action::Focus { pane_id: 46 });
                    return Flow::Exit(Outcome::activated(46));
                }
                Event::Key(key) if key.code == KeyCode::Esc => {
                    return Flow::Exit(Outcome::cancelled());
                }
                Event::Key(key) => self.typed.extend(printable(&key)),
                Event::Forwarded(c) => self.forwarded.push(c),
                Event::Mouse(_) => self.mouse += 1,
                Event::Tick(_) => self.ticks += 1,
                _ => {}
            }
            Flow::Continue
        }

        fn view(&mut self, frame: &mut Frame, _: &mut Cx<Msg>) {
            frame.render_widget(Paragraph::new(self.typed.clone()), frame.area());
        }

        fn snapshot(&self) -> serde_json::Value {
            json!({ "panes": self.panes, "text": self.text, "typed": self.typed, "forwarded": self.forwarded, "mouse": self.mouse })
        }
    }

    fn headless(script: &str, dump: bool) -> (Report, Arc<FakeWezterm>) {
        let wezterm = Arc::new(FakeWezterm::default());
        let opts = RunOptions {
            context: Arc::new(Context::default()),
            wezterm: wezterm.clone(),
            headless: Some(Headless { size: (40, 10), script: parse_script(script).expect("script"), dump }),
        };
        (run(Probe::default(), opts).expect("run"), wezterm)
    }

    #[test]
    fn headless_settles_chained_jobs_before_the_first_frame() {
        let (report, wezterm) = headless("", true);
        assert_eq!(report.outcome, Outcome::open());
        assert_eq!(report.snapshot["panes"], 2);
        assert_eq!(report.snapshot["text"], "$ ls\n");
        let calls = wezterm.calls.lock().expect("calls lock").clone();
        assert_eq!(calls, ["list", "get-text 46 Some(10) true"]);
        assert!(calls.iter().all(|c| !c.starts_with("kill-pane")), "headless never kills its own pane");
    }

    #[test]
    fn headless_types_chars_exits_on_enter_and_reports_actions() {
        let (report, _) = headless("v i enter x", true);
        assert_eq!(report.outcome, Outcome::activated(46));
        assert_eq!(report.actions, [Action::Focus { pane_id: 46 }]);
        assert_eq!(report.snapshot["typed"], "vi", "tokens after the exit are not delivered");

        let json = serde_json::to_value(&report).expect("report serializes");
        assert_eq!(json["exit"], "activated");
        assert_eq!(json["pane_id"], 46);
        assert_eq!(json["actions"][0]["t"], "focus");
        assert_eq!(json["snapshot"]["typed"], "vi");
    }

    #[test]
    fn headless_report_omits_snapshot_without_dump() {
        let (report, _) = headless("esc", false);
        assert_eq!(report.outcome, Outcome::cancelled());
        let json = serde_json::to_value(&report).expect("report serializes");
        assert_eq!(json, json!({ "exit": "cancelled", "actions": [] }));
    }

    #[test]
    fn forward_prefix_turns_the_next_char_into_forwarded() {
        let script = format!("\"{FORWARD_PREFIX}D\" a mouse:click:1,1");
        let (report, _) = headless(&script, true);
        assert_eq!(report.snapshot["forwarded"], json!(["D"]));
        assert_eq!(report.snapshot["typed"], "a");
        assert_eq!(report.snapshot["mouse"], 2, "click is a Down then an Up");
    }

    #[test]
    fn cancelled_job_is_dropped_but_still_settles() {
        let mut runner = Runner::new(
            Probe::default(),
            Arc::new(Context::default()),
            Arc::new(FakeWezterm::default()),
            true,
            Rect::new(0, 0, 40, 10),
        );
        let token = runner.cx.spawn_cancellable(|| {
            thread::sleep(Duration::from_millis(20));
            Msg::Text("late".into())
        });
        token.cancel();
        assert_eq!(runner.cx.jobs_in_flight(), 1);
        runner.settle();
        assert_eq!(runner.cx.jobs_in_flight(), 0);
        assert_eq!(runner.app.text, None);

        runner.cx.spawn(|| Msg::Text("kept".into()));
        runner.settle();
        assert_eq!(runner.app.text.as_deref(), Some("kept"));
    }
}
