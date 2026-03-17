use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{backend::Backend, Terminal};

use crate::{db, models::{ConnectionForm, QueryResult}, storage};

pub enum Screen {
    ConnectionList {
        connections: Vec<String>,
        selected: usize,
    },
    CreateConnection {
        form: ConnectionForm,
        status: Option<String>,
    },
    QueryList {
        connection: String,
        queries: Vec<String>,
        selected: usize,
    },
    CreateQueryName {
        connection: String,
        input: String,
    },
    QueryView {
        connection: String,
        query: String,
        content: String,
    },
    Results {
        connection: String,
        query: String,
        result: QueryResult,
    },
}

pub struct App {
    pub screen: Screen,
    pub status: Option<String>,
}

impl App {
    pub fn new(connection: Option<&str>, query: Option<&str>) -> Result<Self> {
        if let Some(conn) = connection
            && let Ok(cfg) = storage::load_connection(conn) {
                let conn = cfg.name.clone();
                if let Some(qname) = query
                    && let Ok(content) = storage::load_query(&conn, qname) {
                        return Ok(Self {
                            screen: Screen::QueryView { connection: conn, query: qname.to_string(), content },
                            status: None,
                        });
                    }
                let queries = storage::list_queries(&conn)?;
                return Ok(Self {
                    screen: Screen::QueryList { connection: conn, queries, selected: 0 },
                    status: None,
                });
            }

        let connections = storage::list_connections()?;
        Ok(Self {
            screen: Screen::ConnectionList { connections, selected: 0 },
            status: None,
        })
    }

    /// Returns false if the app should quit.
    pub async fn handle_key<B: Backend>(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<B>,
    ) -> Result<bool> {
        self.status = None;

        // Take ownership of the current screen to avoid borrow conflicts.
        let screen = std::mem::replace(
            &mut self.screen,
            Screen::ConnectionList { connections: vec![], selected: 0 },
        );

        let next = match screen {
            Screen::ConnectionList { connections, selected } => {
                self.on_connection_list(key, terminal, connections, selected).await?
            }
            Screen::CreateConnection { form, status } => {
                self.on_create_connection(key, terminal, form, status).await?
            }
            Screen::QueryList { connection, queries, selected } => {
                self.on_query_list(key, terminal, connection, queries, selected).await?
            }
            Screen::CreateQueryName { connection, input } => {
                self.on_create_query_name(key, terminal, connection, input).await?
            }
            Screen::QueryView { connection, query, content } => {
                self.on_query_view(key, terminal, connection, query, content).await?
            }
            Screen::Results { connection, query, result } => {
                self.on_results(key, connection, query, result)
            }
        };

        match next {
            Some(screen) => {
                self.screen = screen;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    // ── ConnectionList ─────────────────────────────────────────────────────────

    async fn on_connection_list<B: Backend>(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<B>,
        mut connections: Vec<String>,
        mut selected: usize,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),

            KeyCode::Up | KeyCode::Char('k') => {
                selected = selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !connections.is_empty() && selected < connections.len() - 1 {
                    selected += 1;
                }
            }

            KeyCode::Enter => {
                if !connections.is_empty() {
                    let conn = connections[selected].clone();
                    let queries = storage::list_queries(&conn)?;
                    return Ok(Some(Screen::QueryList { connection: conn, queries, selected: 0 }));
                }
            }

            KeyCode::Char('n') => {
                return Ok(Some(Screen::CreateConnection {
                    form: ConnectionForm::default(),
                    status: None,
                }));
            }

            KeyCode::Char('d') => {
                if !connections.is_empty() {
                    let name = connections[selected].clone();
                    storage::delete_connection(&name)?;
                    connections = storage::list_connections()?;
                    selected = selected.min(connections.len().saturating_sub(1));
                    self.status = Some(format!("Deleted '{name}'"));
                }
            }

            KeyCode::Char('e') => {
                if !connections.is_empty() {
                    let name = connections[selected].clone();
                    let path = storage::connection_config_path(&name);
                    open_editor(&path)?;
                    terminal.clear()?;
                    match storage::load_connection(&name) {
                        Ok(cfg) => match db::test_connection(&cfg).await {
                            Ok(()) => self.status = Some(format!("'{name}' saved, connection OK")),
                            Err(e) => self.status = Some(format!("Saved but connection failed: {e}")),
                        },
                        Err(e) => self.status = Some(format!("Error parsing config: {e}")),
                    }
                    connections = storage::list_connections()?;
                    selected = selected.min(connections.len().saturating_sub(1));
                }
            }

            _ => {}
        }

        Ok(Some(Screen::ConnectionList { connections, selected }))
    }

    // ── CreateConnection ───────────────────────────────────────────────────────

    async fn on_create_connection<B: Backend>(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<B>,
        mut form: ConnectionForm,
        mut status: Option<String>,
    ) -> Result<Option<Screen>> {
        if form.editing {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => form.editing = false,
                KeyCode::Char(c) => {
                    let c = if form.active_field == 0 && c == ' ' { '_' } else { c };
                    form.active_value_mut().push(c);
                }
                KeyCode::Backspace => { form.active_value_mut().pop(); }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => {
                    let connections = storage::list_connections()?;
                    return Ok(Some(Screen::ConnectionList { connections, selected: 0 }));
                }
                KeyCode::Down | KeyCode::Char('j') => form.next_field(),
                KeyCode::Up | KeyCode::Char('k') => form.prev_field(),
                KeyCode::Enter => form.editing = true,
                KeyCode::Char('s') => {
                    match form.to_config() {
                        Ok(cfg) => match db::test_connection(&cfg).await {
                            Ok(()) => {
                                storage::save_connection(&cfg)?;
                                let name = cfg.name.clone();
                                let connections = storage::list_connections()?;
                                self.status = Some(format!("'{name}' created, connection OK"));
                                return Ok(Some(Screen::ConnectionList { connections, selected: 0 }));
                            }
                            Err(e) => status = Some(format!("Connection failed: {e}")),
                        },
                        Err(e) => status = Some(e),
                    }
                }
                _ => {}
            }
        }

        Ok(Some(Screen::CreateConnection { form, status }))
    }

    // ── QueryList ──────────────────────────────────────────────────────────────

    async fn on_query_list<B: Backend>(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<B>,
        connection: String,
        mut queries: Vec<String>,
        mut selected: usize,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left | KeyCode::Char('h') => {
                let connections = storage::list_connections()?;
                return Ok(Some(Screen::ConnectionList { connections, selected: 0 }));
            }

            KeyCode::Up | KeyCode::Char('k') => {
                selected = selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !queries.is_empty() && selected < queries.len() - 1 {
                    selected += 1;
                }
            }

            KeyCode::Enter => {
                if !queries.is_empty() {
                    let query = queries[selected].clone();
                    let content = storage::load_query(&connection, &query)?;
                    return Ok(Some(Screen::QueryView { connection, query, content }));
                }
            }

            KeyCode::Char('n') => {
                return Ok(Some(Screen::CreateQueryName { connection, input: String::new() }));
            }

            KeyCode::Char('d') => {
                if !queries.is_empty() {
                    let name = queries[selected].clone();
                    storage::delete_query(&connection, &name)?;
                    queries = storage::list_queries(&connection)?;
                    selected = selected.min(queries.len().saturating_sub(1));
                    self.status = Some(format!("Deleted query '{name}'"));
                }
            }

            _ => {}
        }

        Ok(Some(Screen::QueryList { connection, queries, selected }))
    }

    // ── CreateQueryName ────────────────────────────────────────────────────────

    async fn on_create_query_name<B: Backend>(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<B>,
        connection: String,
        mut input: String,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Esc => {
                let queries = storage::list_queries(&connection)?;
                return Ok(Some(Screen::QueryList { connection, queries, selected: 0 }));
            }

            KeyCode::Enter => {
                if !input.is_empty() {
                    let name = input.trim().to_string();
                    storage::save_query(&connection, &name, "")?;
                    let path = storage::query_path(&connection, &name);
                    open_editor(&path)?;
                    terminal.clear()?;
                    let content = storage::load_query(&connection, &name).unwrap_or_default();
                    if content.trim().is_empty() {
                        // User saved nothing — discard
                        let _ = storage::delete_query(&connection, &name);
                        let queries = storage::list_queries(&connection)?;
                        return Ok(Some(Screen::QueryList { connection, queries, selected: 0 }));
                    }
                    return Ok(Some(Screen::QueryView { connection, query: name, content }));
                }
            }

            KeyCode::Char(c) => input.push(if c == ' ' { '_' } else { c }),
            KeyCode::Backspace => { input.pop(); }

            _ => {}
        }

        Ok(Some(Screen::CreateQueryName { connection, input }))
    }

    // ── QueryView ──────────────────────────────────────────────────────────────

    async fn on_query_view<B: Backend>(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<B>,
        connection: String,
        query: String,
        content: String,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                let queries = storage::list_queries(&connection)?;
                return Ok(Some(Screen::QueryList { connection, queries, selected: 0 }));
            }

            KeyCode::Enter => {
                match storage::load_connection(&connection) {
                    Ok(cfg) => match db::execute_query(&cfg, &content).await {
                        Ok(result) => {
                            return Ok(Some(Screen::Results { connection, query, result }));
                        }
                        Err(e) => self.status = Some(format!("Error: {e:#}")),
                    },
                    Err(e) => self.status = Some(format!("Cannot load connection: {e}")),
                }
            }

            KeyCode::Char('e') => {
                let path = storage::query_path(&connection, &query);
                open_editor(&path)?;
                terminal.clear()?;
                let content = storage::load_query(&connection, &query).unwrap_or_default();
                return Ok(Some(Screen::QueryView { connection, query, content }));
            }

            _ => {}
        }

        Ok(Some(Screen::QueryView { connection, query, content }))
    }

    // ── Results ────────────────────────────────────────────────────────────────

    fn on_results(
        &mut self,
        key: KeyEvent,
        connection: String,
        query: String,
        mut result: QueryResult,
    ) -> Option<Screen> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                let content = storage::load_query(&connection, &query).unwrap_or_default();
                return Some(Screen::QueryView { connection, query, content });
            }
            KeyCode::Right | KeyCode::Char('l') => result.next_page(),
            KeyCode::Left | KeyCode::Char('h') => result.prev_page(),
            _ => {}
        }

        Some(Screen::Results { connection, query, result })
    }
}

// ── Editor helper ──────────────────────────────────────────────────────────────

/// Suspends the TUI, opens $EDITOR, then restores raw mode.
/// The caller should call `terminal.clear()` afterwards if needed.
pub fn open_editor(path: &PathBuf) -> Result<()> {
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };

    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    std::process::Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to open editor '{editor}': {e}"))?;

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    use std::sync::Mutex;

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }

    fn make_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(80, 24)).unwrap()
    }

    fn in_create_connection() -> App {
        App {
            screen: Screen::CreateConnection { form: ConnectionForm::default(), status: None },
            status: None,
        }
    }

    fn form(app: &App) -> &ConnectionForm {
        match &app.screen {
            Screen::CreateConnection { form, .. } => form,
            _ => panic!("expected CreateConnection screen"),
        }
    }

    // ── Mode transitions ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn enter_starts_editing() {
        let mut app = in_create_connection();
        app.handle_key(key(KeyCode::Enter), &mut make_terminal()).await.unwrap();
        assert!(form(&app).editing);
    }

    #[tokio::test]
    async fn esc_in_edit_mode_stops_editing() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Esc), &mut t).await.unwrap();
        assert!(!form(&app).editing);
        assert!(matches!(app.screen, Screen::CreateConnection { .. }));
    }

    #[tokio::test]
    async fn enter_in_edit_mode_stops_editing() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        assert!(!form(&app).editing);
        assert!(matches!(app.screen, Screen::CreateConnection { .. }));
    }

    #[tokio::test]
    async fn esc_in_nav_mode_goes_to_connection_list() {
        let _lock = HOME_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", tmp.path()) };

        let mut app = in_create_connection();
        app.handle_key(key(KeyCode::Esc), &mut make_terminal()).await.unwrap();
        assert!(matches!(app.screen, Screen::ConnectionList { .. }));
    }

    // ── Text input ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn edit_mode_types_chars() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        for c in ['a', 'b', 'c'] {
            app.handle_key(key(KeyCode::Char(c)), &mut t).await.unwrap();
        }
        assert_eq!(form(&app).values[0], "abc");
    }

    #[tokio::test]
    async fn edit_mode_backspace_removes_char() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Char('a')), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Char('b')), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Backspace), &mut t).await.unwrap();
        assert_eq!(form(&app).values[0], "a");
    }

    #[tokio::test]
    async fn edit_mode_j_types_not_navigates() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Char('j')), &mut t).await.unwrap();
        assert_eq!(form(&app).values[0], "j");
        assert_eq!(form(&app).active_field, 0);
    }

    #[tokio::test]
    async fn edit_mode_k_types_not_navigates() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Enter), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Char('k')), &mut t).await.unwrap();
        assert_eq!(form(&app).values[0], "k");
        assert_eq!(form(&app).active_field, 0);
    }

    // ── Navigation ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn nav_j_moves_to_next_field() {
        let mut app = in_create_connection();
        app.handle_key(key(KeyCode::Char('j')), &mut make_terminal()).await.unwrap();
        assert_eq!(form(&app).active_field, 1);
        assert_eq!(form(&app).values[0], ""); // j was not typed
    }

    #[tokio::test]
    async fn nav_k_moves_to_prev_field() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Char('j')), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Char('j')), &mut t).await.unwrap();
        assert_eq!(form(&app).active_field, 2);
        app.handle_key(key(KeyCode::Char('k')), &mut t).await.unwrap();
        assert_eq!(form(&app).active_field, 1);
    }

    #[tokio::test]
    async fn nav_down_moves_to_next_field() {
        let mut app = in_create_connection();
        app.handle_key(key(KeyCode::Down), &mut make_terminal()).await.unwrap();
        assert_eq!(form(&app).active_field, 1);
    }

    #[tokio::test]
    async fn nav_up_moves_to_prev_field() {
        let mut app = in_create_connection();
        let mut t = make_terminal();
        app.handle_key(key(KeyCode::Down), &mut t).await.unwrap();
        app.handle_key(key(KeyCode::Up), &mut t).await.unwrap();
        assert_eq!(form(&app).active_field, 0);
    }
}
