use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use slint::{ComponentFactory, ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{
    Base64Tool, CronTool, EnvironmentTool, FormTool, HeaderFormTool, HistoryTool, ListTool,
    MainWindow, MarkdownTool, PageState, SnippetListTool, TabInfo, TaskListTool, TextDiffTool,
    TextTool, ToolInfo,
};

#[derive(Clone, Default)]
struct PageSnapshot {
    draft: SharedString,
    selection_index: i32,
    outer_scroll_y: f32,
    inner_scroll_y: f32,
}

struct ToolView {
    factory: ComponentFactory,
    snapshot: Box<dyn Fn()>,
    focus_probe: Box<dyn Fn()>,
    set_probe: Box<dyn Fn()>,
    probe_matches: Box<dyn Fn() -> bool>,
    #[cfg(test)]
    state: Rc<RefCell<PageSnapshot>>,
}

macro_rules! tool_builder {
    ($function:ident, $component:ty, $draft:literal) => {
        fn $function() -> Result<ToolView> {
            let current = Rc::new(RefCell::new(None));
            let state = Rc::new(RefCell::new(PageSnapshot {
                draft: $draft.into(),
                ..Default::default()
            }));
            let factory_current = current.clone();
            let factory_state = state.clone();
            let snapshot_current = current.clone();
            let snapshot_state = state.clone();
            let focus_current = current.clone();
            let probe_current = current.clone();
            let verify_current = current.clone();
            Ok(ToolView {
                factory: ComponentFactory::new(move |_| {
                    let view = <$component>::new().ok()?;
                    let state = factory_state.borrow().clone();
                    let page = view.global::<PageState>();
                    page.set_draft(state.draft);
                    page.set_selection_index(state.selection_index);
                    page.set_outer_scroll_y(state.outer_scroll_y);
                    page.set_inner_scroll_y(state.inner_scroll_y);
                    *factory_current.borrow_mut() = Some(view.as_weak());
                    Some(view)
                }),
                snapshot: Box::new(move || {
                    let Some(view) = snapshot_current
                        .borrow()
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                    else {
                        return;
                    };
                    let page = view.global::<PageState>();
                    *snapshot_state.borrow_mut() = PageSnapshot {
                        draft: page.get_draft(),
                        selection_index: page.get_selection_index(),
                        outer_scroll_y: page.get_outer_scroll_y(),
                        inner_scroll_y: page.get_inner_scroll_y(),
                    };
                }),
                focus_probe: Box::new(move || {
                    if let Some(view) = focus_current
                        .borrow()
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                    {
                        let page = view.global::<PageState>();
                        page.set_focus_request(page.get_focus_request() + 1);
                    }
                }),
                set_probe: Box::new(move || {
                    if let Some(view) = probe_current
                        .borrow()
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                    {
                        let page = view.global::<PageState>();
                        page.set_draft("probe draft".into());
                        page.set_selection_index(2);
                        page.set_outer_scroll_y(120.0);
                        page.set_inner_scroll_y(60.0);
                    }
                }),
                probe_matches: Box::new(move || {
                    let Some(view) = verify_current
                        .borrow()
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                    else {
                        return false;
                    };
                    let page = view.global::<PageState>();
                    page.get_draft() == "probe draft"
                        && page.get_selection_index() == 2
                        && page.get_outer_scroll_y() == 120.0
                        && page.get_inner_scroll_y() == 60.0
                }),
                #[cfg(test)]
                state,
            })
        }
    };
}

tool_builder!(
    build_json,
    TextTool,
    "Edit this long text, select a range, and scroll both regions."
);
tool_builder!(build_request, FormTool, "localhost");
tool_builder!(build_logs, ListTool, "Filter rows");
tool_builder!(build_diff, TextDiffTool, "Left and right text draft");
tool_builder!(build_cron, CronTool, "0 0 * * *");
tool_builder!(build_tasks, TaskListTool, "Filter tasks");
tool_builder!(build_base64, Base64Tool, "SGVsbG8gTGF6eUNhdA==");
tool_builder!(build_headers, HeaderFormTool, "Accept: application/json");
tool_builder!(build_snippets, SnippetListTool, "Filter snippets");
tool_builder!(build_markdown, MarkdownTool, "# Prototype note");
tool_builder!(build_environment, EnvironmentTool, "development");
tool_builder!(build_history, HistoryTool, "Filter history");

struct ToolDefinition {
    title: &'static str,
    kind: &'static str,
    build: fn() -> Result<ToolView>,
}

const TOOL_DEFINITIONS: [ToolDefinition; 12] = [
    ToolDefinition {
        title: "JSON workbench",
        kind: "text",
        build: build_json,
    },
    ToolDefinition {
        title: "Request form",
        kind: "form",
        build: build_request,
    },
    ToolDefinition {
        title: "Log viewer",
        kind: "list",
        build: build_logs,
    },
    ToolDefinition {
        title: "Text diff",
        kind: "text",
        build: build_diff,
    },
    ToolDefinition {
        title: "Cron builder",
        kind: "form",
        build: build_cron,
    },
    ToolDefinition {
        title: "Task list",
        kind: "list",
        build: build_tasks,
    },
    ToolDefinition {
        title: "Base64 editor",
        kind: "text",
        build: build_base64,
    },
    ToolDefinition {
        title: "HTTP headers",
        kind: "form",
        build: build_headers,
    },
    ToolDefinition {
        title: "Snippet browser",
        kind: "list",
        build: build_snippets,
    },
    ToolDefinition {
        title: "Markdown notes",
        kind: "text",
        build: build_markdown,
    },
    ToolDefinition {
        title: "Environment form",
        kind: "form",
        build: build_environment,
    },
    ToolDefinition {
        title: "History list",
        kind: "list",
        build: build_history,
    },
];

struct ToolSession {
    tool_index: usize,
    title: SharedString,
    view: ToolView,
}

struct WorkspaceState {
    sessions: Vec<ToolSession>,
    active: Option<usize>,
}

impl WorkspaceState {
    fn new() -> Self {
        Self {
            sessions: Vec::new(),
            active: None,
        }
    }

    fn open(&mut self, tool_index: usize) -> Result<()> {
        if let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.tool_index == tool_index)
        {
            self.snapshot_active();
            self.active = Some(index);
            return Ok(());
        }

        self.snapshot_active();
        let definition = TOOL_DEFINITIONS
            .get(tool_index)
            .context("tool index outside prototype catalog")?;
        let view = (definition.build)()?;
        self.sessions.push(ToolSession {
            tool_index,
            title: definition.title.into(),
            view,
        });
        self.active = Some(self.sessions.len() - 1);
        Ok(())
    }

    fn select(&mut self, index: usize) {
        if index < self.sessions.len() && self.active != Some(index) {
            self.snapshot_active();
            self.active = Some(index);
        }
    }

    fn close(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }

        let old_active = self.active;
        self.sessions.remove(index);
        self.active = match self.sessions.len() {
            0 => None,
            len if old_active == Some(index) => Some(index.min(len - 1)),
            _ if old_active.is_some_and(|active| active > index) => {
                old_active.map(|active| active - 1)
            }
            _ => old_active,
        };
    }

    fn snapshot_active(&self) {
        if let Some(active) = self.active {
            (self.sessions[active].view.snapshot)();
        }
    }
}

pub struct Workspace;

impl Workspace {
    pub fn attach(window: &MainWindow) -> Result<()> {
        let state = Rc::new(RefCell::new(WorkspaceState::new()));
        set_tool_catalog(window);

        {
            let state = state.clone();
            let window = window.as_weak();
            window
                .upgrade()
                .context("main window disappeared before callback setup")?
                .on_open_tool(move |index| {
                    let Some(window) = window.upgrade() else {
                        return;
                    };
                    if let Err(error) = state.borrow_mut().open(index.max(0) as usize) {
                        window.set_status(format!("Open failed: {error:#}").into());
                        return;
                    }
                    sync_window(&window, &state.borrow());
                });
        }

        {
            let state = state.clone();
            window.on_focus_active_probe(move || {
                let state = state.borrow();
                if let Some(active) = state.active {
                    (state.sessions[active].view.focus_probe)();
                }
            });
        }

        {
            let state = state.clone();
            window.on_set_active_probe(move || {
                let state = state.borrow();
                if let Some(active) = state.active {
                    (state.sessions[active].view.set_probe)();
                }
            });
        }

        {
            let state = state.clone();
            window.on_active_probe_matches(move || {
                let state = state.borrow();
                state
                    .active
                    .is_some_and(|active| (state.sessions[active].view.probe_matches)())
            });
        }

        {
            let state = state.clone();
            let window = window.as_weak();
            window
                .upgrade()
                .context("main window disappeared before callback setup")?
                .on_select_tab(move |index| {
                    let Some(window) = window.upgrade() else {
                        return;
                    };
                    state.borrow_mut().select(index.max(0) as usize);
                    sync_window(&window, &state.borrow());
                });
        }

        {
            let state = state.clone();
            let window = window.as_weak();
            window
                .upgrade()
                .context("main window disappeared before callback setup")?
                .on_close_tab(move |index| {
                    let Some(window) = window.upgrade() else {
                        return;
                    };
                    state.borrow_mut().close(index.max(0) as usize);
                    sync_window(&window, &state.borrow());
                });
        }

        state.borrow_mut().open(0)?;
        state.borrow_mut().open(1)?;
        state.borrow_mut().open(2)?;
        sync_window(window, &state.borrow());
        Ok(())
    }
}

fn set_tool_catalog(window: &MainWindow) {
    let tools = TOOL_DEFINITIONS
        .iter()
        .enumerate()
        .map(|(index, definition)| ToolInfo {
            index: index as i32,
            title: definition.title.into(),
            kind: definition.kind.into(),
        })
        .collect::<Vec<_>>();
    window.set_tools(ModelRc::new(VecModel::from(tools)));
}

fn sync_window(window: &MainWindow, state: &WorkspaceState) {
    let tabs = state
        .sessions
        .iter()
        .enumerate()
        .map(|(index, session)| TabInfo {
            index: index as i32,
            title: session.title.clone(),
            active: state.active == Some(index),
        })
        .collect::<Vec<_>>();
    window.set_tabs(ModelRc::new(VecModel::from(tabs)));
    window.set_active_tab(state.active.map_or(-1, |index| index as i32));
    if let Some(active) = state.active {
        window.set_active_tool(state.sessions[active].view.factory.clone());
        window.set_status(
            "Switch tabs to verify draft, selection, both scroll positions, and local context."
                .into(),
        );
    } else {
        window.set_active_tool(ComponentFactory::default());
        window.set_status("Open a tool from the left sidebar.".into());
    }
}

#[cfg(test)]
mod tests {
    use super::{TOOL_DEFINITIONS, WorkspaceState, sync_window};
    use crate::MainWindow;

    #[test]
    fn workspace_preserves_snapshot_until_close() {
        let window = MainWindow::new().unwrap();
        let mut state = WorkspaceState::new();
        for index in 0..TOOL_DEFINITIONS.len() {
            state.open(index).unwrap();
            sync_window(&window, &state);
        }
        assert_eq!(state.sessions.len(), TOOL_DEFINITIONS.len());

        *state.sessions[0].view.state.borrow_mut() = super::PageSnapshot {
            draft: "probe draft".into(),
            selection_index: 2,
            outer_scroll_y: 120.0,
            inner_scroll_y: 60.0,
        };
        state.select(1);
        sync_window(&window, &state);
        state.select(0);
        sync_window(&window, &state);
        let snapshot = state.sessions[0].view.state.borrow();
        assert_eq!(snapshot.draft, "probe draft");
        assert_eq!(snapshot.selection_index, 2);
        assert_eq!(snapshot.outer_scroll_y, 120.0);
        assert_eq!(snapshot.inner_scroll_y, 60.0);
        drop(snapshot);

        state.open(0).unwrap();
        assert_eq!(state.sessions.len(), TOOL_DEFINITIONS.len());
        assert_eq!(state.active, Some(0));

        state.close(0);
        assert_eq!(state.sessions.len(), TOOL_DEFINITIONS.len() - 1);
        assert_eq!(state.active, Some(0));

        state.open(0).unwrap();
        assert_eq!(state.sessions.len(), TOOL_DEFINITIONS.len());
        let reopened = state
            .sessions
            .iter()
            .find(|session| session.tool_index == 0)
            .unwrap();
        let snapshot = reopened.view.state.borrow();
        assert_ne!(snapshot.draft, "probe draft");
        assert_eq!(snapshot.selection_index, 0);
        assert_eq!(snapshot.outer_scroll_y, 0.0);
        assert_eq!(snapshot.inner_scroll_y, 0.0);
    }
}
