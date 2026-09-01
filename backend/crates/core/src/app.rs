use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::context::{Context, PaneId, Presentation};
use crate::event::Event;
use crate::runtime::Inbox;
use crate::ui::fx::Effects;
use crate::ui::theme::Theme;
use crate::wezterm::{Action, Wezterm};

/// A launcher screen. The runtime owns the terminal and the loop; the app owns state and drawing.
pub trait App: Send + 'static {
    type Msg: Send + 'static;

    fn name(&self) -> &'static str;
    /// Kick off initial jobs (`cx.spawn`). Called once before the first frame.
    fn init(&mut self, cx: &mut Cx<Self::Msg>);
    fn update(&mut self, event: Event<Self::Msg>, cx: &mut Cx<Self::Msg>) -> Flow;
    fn view(&mut self, frame: &mut Frame, cx: &mut Cx<Self::Msg>);
    /// View model for `--dump`; behaviour tests read this, so keep it stable and small.
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Flow {
    Continue,
    Exit(Outcome),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exit {
    Activated,
    Cancelled,
    HandedOff,
    /// Headless script ended while the launcher was still open.
    Open,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub exit: Exit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<PaneId>,
}

impl Outcome {
    pub fn activated(pane_id: PaneId) -> Self {
        Self { exit: Exit::Activated, pane_id: Some(pane_id) }
    }
    pub fn cancelled() -> Self {
        Self { exit: Exit::Cancelled, pane_id: None }
    }
    pub fn handed_off() -> Self {
        Self { exit: Exit::HandedOff, pane_id: None }
    }
    pub fn open() -> Self {
        Self { exit: Exit::Open, pane_id: None }
    }
}

/// Per-app handle into the runtime: theme, context, wezterm, jobs, actions, effects.
pub struct Cx<M> {
    pub theme: Theme,
    pub presentation: Presentation,
    pub context: Arc<Context>,
    pub wezterm: Arc<dyn Wezterm>,
    pub fx: Effects,
    /// Frame area at the start of `view`; a view may narrow it to its modal so effects and
    /// hit-tests use that instead.
    pub area: Rect,
    pub headless: bool,
    pub own_pane: Option<PaneId>,
    pub(crate) jobs: Sender<Inbox<M>>,
    pub(crate) in_flight: Arc<AtomicUsize>,
    pub(crate) actions: Vec<Action>,
    pub(crate) redraw: bool,
    pub(crate) log: crate::log::Logger,
}

impl<M: Send + 'static> Cx<M> {
    /// Runs `job` on a worker thread; its result arrives as `Event::Msg`.
    pub fn spawn<F>(&self, job: F)
    where
        F: FnOnce() -> M + Send + 'static,
    {
        self.spawn_job(job, None);
    }

    /// Like `spawn` but returns a cancellation token: a stale job's message is dropped when
    /// `token.cancel()` was called before it finished (debounced previews).
    pub fn spawn_cancellable<F>(&self, job: F) -> JobToken
    where
        F: FnOnce() -> M + Send + 'static,
    {
        let token = JobToken::default();
        self.spawn_job(job, Some(token.clone()));
        token
    }

    fn spawn_job<F>(&self, job: F, token: Option<JobToken>)
    where
        F: FnOnce() -> M + Send + 'static,
    {
        let guard = InFlight::enter(&self.in_flight);
        let tx = self.jobs.clone();
        let spawned = std::thread::Builder::new().name("nardo-job".into()).spawn(move || {
            let msg = job();
            if token.is_none_or(|t| !t.cancelled()) {
                let _ = tx.send(Inbox::Msg(msg));
            }
            drop(guard);
        });
        if let Err(err) = spawned {
            self.log.log(format!("spawn: thread failed: {err}"));
        }
    }

    /// Queues a user-var action. Headless keeps it for the report; interactive writes the OSC
    /// after the next frame flush.
    pub fn emit(&mut self, action: Action) {
        self.log.log(format!("emit {action:?}"));
        self.actions.push(action);
    }

    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }

    pub fn log(&mut self, msg: impl AsRef<str>) {
        self.log.log(msg);
    }

    pub fn jobs_in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    pub fn animations(&self) -> bool {
        self.presentation.animations && !self.headless
    }

    pub fn tick_rate(&self) -> Duration {
        Duration::from_millis(16)
    }
}

/// Counts a job until its thread finishes, message sent or not, so `settle` never hangs.
struct InFlight(Arc<AtomicUsize>);

impl InFlight {
    fn enter(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(Arc::clone(counter))
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobToken(Arc<AtomicBool>);

impl JobToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
