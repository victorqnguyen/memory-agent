mod views;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use memory_core::{
    types::EventLogEntry, types::Memory, types::MemoryMetric, types::SearchResult, SearchParams,
    Store,
};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::{ListState, TableState};
use tui_textarea::TextArea;

use crate::config_loader::HooksConfig;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Search,
    Detail(i64),
    Live,
    Metrics,
    ScopeTree,
    HookConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

/// A row in the scope tree: either a scope header or a memory under it.
#[derive(Debug, Clone)]
pub enum TreeRow {
    Scope {
        path: String,
        count: usize,
    },
    Memory {
        id: i64,
        key: String,
        preview: String,
    },
}

pub struct App {
    pub store: Store,
    pub view: View,
    pub input_mode: InputMode,
    pub search_input: String,
    pub search_results: Vec<SearchResult>,
    pub list_state: ListState,
    pub table_state: TableState,
    pub detail_memory: Option<Memory>,
    pub detail_scroll: u16,
    pub metrics: Vec<MemoryMetric>,
    // Scope tree
    pub tree_rows: Vec<TreeRow>,
    pub expanded_scopes: HashSet<String>,
    pub tree_state: ListState,
    // Delete confirmation
    pub confirm_delete: Option<(i64, String)>, // (id, key) pending deletion
    pub should_quit: bool,
    // Hook config
    pub data_dir: PathBuf,
    pub hooks_config: HooksConfig,
    pub hook_selected: usize,
    pub hook_status: Option<String>,
    pub pending_edit: Option<(usize, Option<String>)>,
    pub template_editor: Option<(usize, TextArea<'static>)>,
    // Live feed
    pub live_events: Vec<EventLogEntry>,
    pub live_state: ListState,
    /// (saves_today, injections_today, searches_today, tokens_today)
    pub live_summary: (i64, i64, i64, i64),
    /// Update notice fetched at startup; shown as a banner across all tabs.
    pub update_notice: Option<String>,
}

impl App {
    pub fn new(
        store: Store,
        data_dir: PathBuf,
        hooks_config: HooksConfig,
        update_notice: Option<String>,
    ) -> anyhow::Result<Self> {
        let mut app = Self {
            store,
            view: View::Search,
            input_mode: InputMode::Normal,
            search_input: String::new(),
            search_results: Vec::new(),
            list_state: ListState::default(),
            table_state: TableState::default(),
            detail_memory: None,
            detail_scroll: 0,
            metrics: Vec::new(),
            tree_rows: Vec::new(),
            expanded_scopes: HashSet::new(),
            tree_state: ListState::default(),
            confirm_delete: None,
            should_quit: false,
            data_dir,
            hooks_config,
            hook_selected: 0,
            hook_status: None,
            pending_edit: None,
            template_editor: None,
            live_events: Vec::new(),
            live_state: ListState::default(),
            live_summary: (0, 0, 0, 0),
            update_notice,
        };
        app.load_initial_data()?;
        Ok(app)
    }

    fn load_initial_data(&mut self) -> anyhow::Result<()> {
        self.do_search()?;
        Ok(())
    }

    pub fn do_search(&mut self) -> anyhow::Result<()> {
        if self.search_input.is_empty() {
            let all = self.store.list(None, None, Some(100))?;
            self.search_results = all
                .into_iter()
                .map(|m| SearchResult {
                    id: m.id,
                    key: m.key.clone(),
                    value_preview: memory_core::make_preview(&m.value, 80),
                    scope: m.scope,
                    source_type: m.source_type,
                    confidence: m.confidence,
                    rank: 0.0,
                })
                .collect();
        } else {
            self.search_results = self.store.search(SearchParams {
                query: self.search_input.clone(),
                scope: None,
                source_type: None,
                limit: Some(50),
            })?;
        }
        self.list_state = ListState::default();
        if !self.search_results.is_empty() {
            self.list_state.select(Some(0));
        }
        Ok(())
    }

    pub fn load_metrics(&mut self) -> anyhow::Result<()> {
        self.metrics = self.store.get_metrics()?;
        self.metrics.sort_by(|a, b| b.injections.cmp(&a.injections));
        self.table_state = TableState::default();
        if !self.metrics.is_empty() {
            self.table_state.select(Some(0));
        }
        Ok(())
    }

    pub fn load_scope_tree(&mut self) -> anyhow::Result<()> {
        let all = self.store.list(None, None, Some(10000))?;

        // Group memories by scope
        let mut scope_memories: std::collections::BTreeMap<String, Vec<Memory>> =
            std::collections::BTreeMap::new();
        for m in all {
            scope_memories.entry(m.scope.clone()).or_default().push(m);
        }

        // Build tree rows: scope headers + expanded memories
        let mut rows = Vec::new();
        for (scope, memories) in &scope_memories {
            rows.push(TreeRow::Scope {
                path: scope.clone(),
                count: memories.len(),
            });
            if self.expanded_scopes.contains(scope) {
                for m in memories {
                    rows.push(TreeRow::Memory {
                        id: m.id,
                        key: m.key.clone(),
                        preview: memory_core::make_preview(&m.value, 60),
                    });
                }
            }
        }

        self.tree_rows = rows;
        // Preserve selection if valid, otherwise reset
        if let Some(idx) = self.tree_state.selected() {
            if idx >= self.tree_rows.len() {
                self.tree_state.select(if self.tree_rows.is_empty() {
                    None
                } else {
                    Some(0)
                });
            }
        } else if !self.tree_rows.is_empty() {
            self.tree_state.select(Some(0));
        }
        Ok(())
    }

    pub fn load_live(&mut self) -> anyhow::Result<()> {
        self.live_events = self.store.recent_events(100)?;
        let summary = self.store.events_today_summary()?;
        self.live_summary = summary;
        if !self.live_events.is_empty() && self.live_state.selected().is_none() {
            self.live_state.select(Some(0));
        }
        Ok(())
    }

    fn toggle_scope(&mut self) -> anyhow::Result<()> {
        if let Some(idx) = self.tree_state.selected() {
            if let Some(TreeRow::Scope { path, .. }) = self.tree_rows.get(idx) {
                let path = path.clone();
                if self.expanded_scopes.contains(&path) {
                    self.expanded_scopes.remove(&path);
                } else {
                    self.expanded_scopes.insert(path);
                }
                self.load_scope_tree()?;
            }
        }
        Ok(())
    }

    fn selected_memory_id(&self) -> Option<(i64, String)> {
        match &self.view {
            View::Search => self
                .list_state
                .selected()
                .and_then(|idx| self.search_results.get(idx))
                .map(|r| (r.id, r.key.clone())),
            View::Detail(id) => Some((
                *id,
                self.detail_memory
                    .as_ref()
                    .map(|m| m.key.clone())
                    .unwrap_or_default(),
            )),
            View::ScopeTree => self
                .tree_state
                .selected()
                .and_then(|idx| self.tree_rows.get(idx))
                .and_then(|row| match row {
                    TreeRow::Memory { id, key, .. } => Some((*id, key.clone())),
                    _ => None,
                }),
            _ => None,
        }
    }

    fn delete_confirmed(&mut self, hard: bool) -> anyhow::Result<()> {
        if let Some((id, _)) = self.confirm_delete.take() {
            self.store.delete_by_id(id, hard)?;
            // If we're in detail view for this memory, go back
            if matches!(self.view, View::Detail(did) if did == id) {
                self.view = View::Search;
            }
            self.refresh_current_view()?;
        }
        Ok(())
    }

    fn scroll_down(&mut self) {
        match &self.view.clone() {
            View::Detail(_) => {
                self.detail_scroll = self.detail_scroll.saturating_add(3);
            }
            View::Metrics => {
                let len = self.metrics.len();
                if len > 0 {
                    let current = self.table_state.selected().unwrap_or(0);
                    self.table_state.select(Some((current + 1).min(len - 1)));
                }
            }
            View::Search => {
                let len = self.search_results.len();
                if len > 0 {
                    let current = self.list_state.selected().unwrap_or(0);
                    self.list_state.select(Some((current + 1).min(len - 1)));
                }
            }
            View::ScopeTree => {
                let len = self.tree_rows.len();
                if len > 0 {
                    let current = self.tree_state.selected().unwrap_or(0);
                    self.tree_state.select(Some((current + 1).min(len - 1)));
                }
            }
            View::Live => {
                let len = self.live_events.len();
                if len > 0 {
                    let cur = self.live_state.selected().unwrap_or(0);
                    self.live_state.select(Some((cur + 1).min(len - 1)));
                }
            }
            View::HookConfig => {}
        }
    }

    fn scroll_up(&mut self) {
        match &self.view.clone() {
            View::Detail(_) => {
                self.detail_scroll = self.detail_scroll.saturating_sub(3);
            }
            View::Metrics => {
                let current = self.table_state.selected().unwrap_or(0);
                if current > 0 {
                    self.table_state.select(Some(current - 1));
                }
            }
            View::ScopeTree => {
                let current = self.tree_state.selected().unwrap_or(0);
                if current > 0 {
                    self.tree_state.select(Some(current - 1));
                }
            }
            View::Live => {
                let cur = self.live_state.selected().unwrap_or(0);
                if cur > 0 {
                    self.live_state.select(Some(cur - 1));
                }
            }
            View::HookConfig => {
                self.hook_selected = self.hook_selected.saturating_sub(1);
            }
            _ => {
                let current = self.list_state.selected().unwrap_or(0);
                if current > 0 {
                    self.list_state.select(Some(current - 1));
                }
            }
        }
    }

    fn open_detail(&mut self) -> anyhow::Result<()> {
        let id = match &self.view {
            View::Search => self
                .list_state
                .selected()
                .and_then(|idx| self.search_results.get(idx).map(|r| r.id)),
            View::ScopeTree => self
                .tree_state
                .selected()
                .and_then(|idx| self.tree_rows.get(idx))
                .and_then(|row| match row {
                    TreeRow::Memory { id, .. } => Some(*id),
                    _ => None,
                }),
            _ => None,
        };
        if let Some(id) = id {
            match self.store.get(id) {
                Ok(m) => {
                    self.detail_memory = Some(m);
                    self.view = View::Detail(id);
                    self.detail_scroll = 0;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn refresh_current_view(&mut self) -> anyhow::Result<()> {
        match &self.view {
            View::Search => self.do_search()?,
            View::Detail(id) => {
                let id = *id;
                match self.store.get(id) {
                    Ok(m) => self.detail_memory = Some(m),
                    Err(_) => {
                        self.detail_memory = None;
                        self.view = View::Search;
                        self.do_search()?;
                    }
                }
            }
            View::Live => self.load_live()?,
            View::Metrics => self.load_metrics()?,
            View::ScopeTree => self.load_scope_tree()?,
            View::HookConfig => {}
        }
        Ok(())
    }

    fn switch_view(&mut self, next: View) -> anyhow::Result<()> {
        self.view = next.clone();
        self.hook_status = None;
        if !matches!(self.view, View::HookConfig) {
            self.pending_edit = None;
            self.template_editor = None;
        }
        match next {
            View::Live => self.load_live()?,
            View::Metrics => self.load_metrics()?,
            View::ScopeTree => self.load_scope_tree()?,
            View::Search => {
                self.list_state = ListState::default();
                self.do_search()?;
            }
            View::Detail(_) | View::HookConfig => {}
        }
        Ok(())
    }

    fn next_view(&mut self) -> anyhow::Result<()> {
        let next = match &self.view {
            View::Search | View::Detail(_) => View::Live,
            View::Live => View::Metrics,
            View::Metrics => View::ScopeTree,
            View::ScopeTree => View::HookConfig,
            View::HookConfig => View::Search,
        };
        self.switch_view(next)
    }

    fn prev_view(&mut self) -> anyhow::Result<()> {
        let prev = match &self.view {
            View::Search | View::Detail(_) => View::HookConfig,
            View::Live => View::Search,
            View::Metrics => View::Live,
            View::ScopeTree => View::Metrics,
            View::HookConfig => View::ScopeTree,
        };
        self.switch_view(prev)
    }

    fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        let code = key.code;

        // Inline template editor intercepts all keys when active.
        if let Some((_field_idx, ref mut ta)) = self.template_editor {
            match (code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.template_editor = None;
                    self.hook_status = Some("Discarded.".to_string());
                }
                (KeyCode::Char('s'), KeyModifiers::CONTROL) | (KeyCode::F(2), _) => {
                    let content = ta.lines().join("\n");
                    let new_value: Option<String> = if content.trim().is_empty() {
                        None
                    } else {
                        Some(content.trim().to_string())
                    };
                    let original = self.hooks_config.injection_prompt.as_deref();
                    self.template_editor = None;
                    if new_value.as_deref() == original {
                        self.hook_status = Some("No changes.".to_string());
                        return Ok(());
                    }
                    self.hooks_config.injection_prompt = new_value;
                    match crate::config_loader::save_hooks_config(
                        &self.data_dir,
                        &self.hooks_config,
                    ) {
                        Ok(()) => self.hook_status = Some("Saved.".to_string()),
                        Err(e) => self.hook_status = Some(format!("Error: {e}")),
                    }
                }
                _ => {
                    ta.input(key);
                }
            }
            return Ok(());
        }

        // Handle delete confirmation
        if self.confirm_delete.is_some() {
            match code {
                KeyCode::Char('y') => self.delete_confirmed(false)?,
                KeyCode::Char('Y') => self.delete_confirmed(true)?,
                _ => {
                    self.confirm_delete = None;
                }
            }
            return Ok(());
        }

        match self.input_mode {
            InputMode::Search => match code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Enter => {
                    self.input_mode = InputMode::Normal;
                    self.do_search()?;
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                }
                _ => {}
            },
            InputMode::Normal => match code {
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                KeyCode::Tab => {
                    self.next_view()?;
                }
                KeyCode::BackTab => {
                    self.prev_view()?;
                }
                // HookConfig-specific keys
                KeyCode::Char('e') if matches!(self.view, View::HookConfig) => {
                    self.hook_status = None;
                    let current = self.hooks_config.injection_prompt.as_deref().unwrap_or("");
                    let lines: Vec<String> = current.lines().map(|l: &str| l.to_string()).collect();
                    let mut ta = TextArea::new(lines);
                    ta.set_block(
                        ratatui::widgets::Block::default()
                            .borders(ratatui::widgets::Borders::ALL)
                            .title(" Edit Injection Prompt — Ctrl+S=save  Esc=cancel ")
                            .border_style(
                                ratatui::style::Style::default().fg(ratatui::style::Color::Yellow),
                            ),
                    );
                    self.template_editor = Some((0, ta));
                }
                KeyCode::Char('s') if matches!(self.view, View::HookConfig) => {
                    if let Some((_field_idx, new_value)) = self.pending_edit.take() {
                        self.hooks_config.injection_prompt = new_value;
                        match crate::config_loader::save_hooks_config(
                            &self.data_dir,
                            &self.hooks_config,
                        ) {
                            Ok(()) => self.hook_status = Some("Saved.".to_string()),
                            Err(e) => self.hook_status = Some(format!("Error: {e}")),
                        }
                    }
                }
                KeyCode::Esc
                    if matches!(self.view, View::HookConfig) && self.pending_edit.is_some() =>
                {
                    self.pending_edit = None;
                    self.hook_status = Some("Discarded.".to_string());
                }
                KeyCode::Char('x') if matches!(self.view, View::HookConfig) => {
                    self.hook_status = None;
                    self.hooks_config.injection_prompt = None;
                    match crate::config_loader::save_hooks_config(
                        &self.data_dir,
                        &self.hooks_config,
                    ) {
                        Ok(()) => self.hook_status = Some("Reset to default.".to_string()),
                        Err(e) => self.hook_status = Some(format!("Error: {e}")),
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll_up();
                }
                KeyCode::Char('/') => {
                    if matches!(self.view, View::Search) {
                        self.input_mode = InputMode::Search;
                    }
                }
                KeyCode::Enter => {
                    if matches!(self.view, View::ScopeTree) {
                        // Enter on a scope toggles it, on a memory opens detail
                        if let Some(idx) = self.tree_state.selected() {
                            match self.tree_rows.get(idx) {
                                Some(TreeRow::Scope { .. }) => self.toggle_scope()?,
                                Some(TreeRow::Memory { .. }) => self.open_detail()?,
                                None => {}
                            }
                        }
                    } else {
                        self.open_detail()?;
                    }
                }
                KeyCode::Char('d') => {
                    if let Some((id, key)) = self.selected_memory_id() {
                        self.confirm_delete = Some((id, key));
                    }
                }
                KeyCode::Char('r') => {
                    self.refresh_current_view()?;
                }
                KeyCode::Esc => {
                    if matches!(self.view, View::Detail(_)) {
                        self.switch_view(View::Search)?;
                    }
                }
                _ => {}
            },
        }
        Ok(())
    }
}

const LIVE_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const DATA_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

pub fn run(
    store: Store,
    data_dir: PathBuf,
    hooks_config: HooksConfig,
    update_notice: Option<String>,
) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new(store, data_dir, hooks_config, update_notice)?;
    let mut last_live_refresh = Instant::now();
    let mut last_data_refresh = Instant::now();

    loop {
        terminal.draw(|f| views::render(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                app.handle_key(key)?;
                last_data_refresh = Instant::now();
            }
        }

        // Auto-refresh Live tab every 2s
        if matches!(app.view, View::Live) && last_live_refresh.elapsed() >= LIVE_REFRESH_INTERVAL {
            app.load_live()?;
            last_live_refresh = Instant::now();
        }

        // Auto-refresh data tabs (Metrics, ScopeTree, Search) every 30s
        if matches!(app.view, View::Metrics | View::ScopeTree | View::Search)
            && last_data_refresh.elapsed() >= DATA_REFRESH_INTERVAL
        {
            app.refresh_current_view()?;
            last_data_refresh = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::Store;

    fn make_app() -> App {
        let store = Store::open_in_memory().expect("in-memory store");
        App::new(store, PathBuf::from("/tmp"), HooksConfig::default(), None).expect("App::new")
    }

    /// Live tab loads event log entries.
    #[test]
    fn live_tab_loads_events() {
        let mut app = make_app();
        app.switch_view(View::Live).unwrap();
        // No events yet — just verify it doesn't panic
        assert_eq!(app.live_events.len(), 0);
        assert_eq!(app.live_summary, (0, 0, 0, 0));
    }

    /// LIVE_REFRESH_INTERVAL constant is the expected 2 seconds.
    #[test]
    fn live_refresh_interval_is_2s() {
        assert_eq!(LIVE_REFRESH_INTERVAL, Duration::from_secs(2));
    }

    /// DATA_REFRESH_INTERVAL constant is the expected 30 seconds.
    #[test]
    fn data_refresh_interval_is_30s() {
        assert_eq!(DATA_REFRESH_INTERVAL, Duration::from_secs(30));
    }
}
