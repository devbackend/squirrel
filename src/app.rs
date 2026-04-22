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
        preview: String,
    },
    CreateQueryName {
        connection: String,
        input: String,
    },
    RenameConnection {
        old_name: String,
        input: String,
    },
    RenameQuery {
        connection: String,
        old_name: String,
        input: String,
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

fn load_preview(connection: &str, queries: &[String], selected: usize) -> String {
    queries.get(selected)
        .and_then(|q| storage::load_query(connection, q).ok())
        .unwrap_or_default()
}

impl App {
    pub fn new(connection: Option<&str>, query: Option<&str>) -> Result<Self> {
        if let Some(conn) = connection
            && let Ok(cfg) = storage::load_connection(conn) {
                let conn = cfg.name.clone();
                let queries = storage::list_queries(&conn)?;
                let selected = query
                    .and_then(|q| queries.iter().position(|name| name == q))
                    .unwrap_or(0);
                let preview = load_preview(&conn, &queries, selected);
                return Ok(Self {
                    screen: Screen::QueryList { connection: conn, queries, selected, preview },
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

        if key.code == KeyCode::Char('q') && key.modifiers == crossterm::event::KeyModifiers::CONTROL {
            return Ok(false);
        }

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
            Screen::QueryList { connection, queries, selected, preview } => {
                self.on_query_list(key, terminal, connection, queries, selected, preview).await?
            }
            Screen::CreateQueryName { connection, input } => {
                self.on_create_query_name(key, terminal, connection, input).await?
            }
            Screen::RenameConnection { old_name, input } => {
                self.on_rename_connection(key, terminal, old_name, input).await?
            }
            Screen::RenameQuery { connection, old_name, input } => {
                self.on_rename_query(key, terminal, connection, old_name, input).await?
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
            KeyCode::Down | KeyCode::Char('j') if !connections.is_empty() && selected < connections.len() - 1 => {
                selected += 1;
            }

            KeyCode::Enter if !connections.is_empty() => {
                let conn = connections[selected].clone();
                let queries = storage::list_queries(&conn)?;
                let preview = load_preview(&conn, &queries, 0);
                return Ok(Some(Screen::QueryList { connection: conn, queries, selected: 0, preview }));
            }

            KeyCode::Char('n') => {
                return Ok(Some(Screen::CreateConnection {
                    form: ConnectionForm::default(),
                    status: None,
                }));
            }

            KeyCode::Char('d') if !connections.is_empty() => {
                let name = connections[selected].clone();
                storage::delete_connection(&name)?;
                connections = storage::list_connections()?;
                selected = selected.min(connections.len().saturating_sub(1));
                self.status = Some(format!("Deleted '{name}'"));
            }

            KeyCode::Char('e') if !connections.is_empty() => {
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

            KeyCode::Char('r') if !connections.is_empty() => {
                let old_name = connections[selected].clone();
                return Ok(Some(Screen::RenameConnection { old_name, input: String::new() }));
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
        terminal: &mut Terminal<B>,
        connection: String,
        mut queries: Vec<String>,
        mut selected: usize,
        mut preview: String,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                let connections = storage::list_connections()?;
                return Ok(Some(Screen::ConnectionList { connections, selected: 0 }));
            }

            KeyCode::Up | KeyCode::Char('k') => {
                selected = selected.saturating_sub(1);
                preview = load_preview(&connection, &queries, selected);
            }
            KeyCode::Down | KeyCode::Char('j') if !queries.is_empty() && selected < queries.len() - 1 => {
                selected += 1;
                preview = load_preview(&connection, &queries, selected);
            }

            KeyCode::Enter if !queries.is_empty() => {
                let query = queries[selected].clone();
                let content = storage::load_query(&connection, &query)?;
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

            KeyCode::Char('r') if !queries.is_empty() => {
                let old_name = queries[selected].clone();
                return Ok(Some(Screen::RenameQuery {
                    connection,
                    old_name,
                    input: String::new(),
                }));
            }

            KeyCode::Char('e') if !queries.is_empty() => {
                let query = &queries[selected];
                let path = storage::query_path(&connection, query);
                open_editor(&path)?;
                terminal.clear()?;
                preview = load_preview(&connection, &queries, selected);
            }

            KeyCode::Char('n') => {
                return Ok(Some(Screen::CreateQueryName { connection, input: String::new() }));
            }

            KeyCode::Char('d') if !queries.is_empty() => {
                let name = queries[selected].clone();
                storage::delete_query(&connection, &name)?;
                queries = storage::list_queries(&connection)?;
                selected = selected.min(queries.len().saturating_sub(1));
                preview = load_preview(&connection, &queries, selected);
                self.status = Some(format!("Deleted query '{name}'"));
            }

            _ => {}
        }

        Ok(Some(Screen::QueryList { connection, queries, selected, preview }))
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
                let preview = load_preview(&connection, &queries, 0);
                return Ok(Some(Screen::QueryList { connection, queries, selected: 0, preview }));
            }

            KeyCode::Enter if !input.is_empty() => {
                let name = input.trim().to_string();
                storage::save_query(&connection, &name, "")?;
                let path = storage::query_path(&connection, &name);
                open_editor(&path)?;
                terminal.clear()?;
                let content = storage::load_query(&connection, &name).unwrap_or_default();
                if content.trim().is_empty() {
                    let _ = storage::delete_query(&connection, &name);
                }
                let queries = storage::list_queries(&connection)?;
                let selected = queries.iter().position(|q| q == &name).unwrap_or(0);
                let preview = load_preview(&connection, &queries, selected);
                return Ok(Some(Screen::QueryList { connection, queries, selected, preview }));
            }

            KeyCode::Char(c) => input.push(if c == ' ' { '_' } else { c }),
            KeyCode::Backspace => { input.pop(); }

            _ => {}
        }

        Ok(Some(Screen::CreateQueryName { connection, input }))
    }

    // ── RenameConnection ───────────────────────────────────────────────────────

    async fn on_rename_connection<B: Backend>(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<B>,
        old_name: String,
        mut input: String,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Esc => {
                let connections = storage::list_connections()?;
                let selected = connections.iter().position(|c| c == &old_name).unwrap_or(0);
                return Ok(Some(Screen::ConnectionList { connections, selected }));
            }

            KeyCode::Enter if !input.is_empty() => {
                let new_name = input.trim().to_string();
                match storage::rename_connection(&old_name, &new_name) {
                    Ok(()) => {
                        let connections = storage::list_connections()?;
                        let selected = connections.iter().position(|c| c == &new_name).unwrap_or(0);
                        self.status = Some(format!("Renamed '{old_name}' → '{new_name}'"));
                        return Ok(Some(Screen::ConnectionList { connections, selected }));
                    }
                    Err(e) => {
                        self.status = Some(format!("Rename failed: {e}"));
                        // Stay on the rename screen so the user can correct input.
                    }
                }
            }

            KeyCode::Char(c) => input.push(if c == ' ' { '_' } else { c }),
            KeyCode::Backspace => { input.pop(); }

            _ => {}
        }

        Ok(Some(Screen::RenameConnection { old_name, input }))
    }

    // ── RenameQuery ────────────────────────────────────────────────────────────

    async fn on_rename_query<B: Backend>(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<B>,
        connection: String,
        old_name: String,
        mut input: String,
    ) -> Result<Option<Screen>> {
        match key.code {
            KeyCode::Esc => {
                let queries = storage::list_queries(&connection)?;
                let selected = queries.iter().position(|q| q == &old_name).unwrap_or(0);
                let preview = load_preview(&connection, &queries, selected);
                return Ok(Some(Screen::QueryList { connection, queries, selected, preview }));
            }

            KeyCode::Enter if !input.is_empty() => {
                let new_name = input.trim().to_string();
                match storage::rename_query(&connection, &old_name, &new_name) {
                    Ok(()) => {
                        let queries = storage::list_queries(&connection)?;
                        let selected = queries.iter().position(|q| q == &new_name).unwrap_or(0);
                        let preview = load_preview(&connection, &queries, selected);
                        self.status = Some(format!("Renamed '{old_name}' → '{new_name}'"));
                        return Ok(Some(Screen::QueryList { connection, queries, selected, preview }));
                    }
                    Err(e) => {
                        self.status = Some(format!("Rename failed: {e}"));
                        // Stay on the rename screen so the user can correct input.
                    }
                }
            }

            KeyCode::Char(c) => input.push(if c == ' ' { '_' } else { c }),
            KeyCode::Backspace => { input.pop(); }

            _ => {}
        }

        Ok(Some(Screen::RenameQuery { connection, old_name, input }))
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
                let queries = storage::list_queries(&connection).unwrap_or_default();
                let selected = queries.iter().position(|q| q == &query).unwrap_or(0);
                let preview = load_preview(&connection, &queries, selected);
                return Some(Screen::QueryList { connection, queries, selected, preview });
            }
            KeyCode::Right | KeyCode::Char('l') => result.next_page(),
            KeyCode::Left | KeyCode::Char('h') => result.prev_page(),
            KeyCode::Down | KeyCode::Char('j') => result.select_next_row(),
            KeyCode::Up | KeyCode::Char('k') => result.select_prev_row(),
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

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent { code, modifiers: KeyModifiers::CONTROL, kind: KeyEventKind::Press, state: KeyEventState::NONE }
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

    // ── Global Ctrl+Q ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn ctrl_q_quits_from_create_connection() {
        let mut app = in_create_connection();
        let result = app.handle_key(ctrl(KeyCode::Char('q')), &mut make_terminal()).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn ctrl_q_quits_from_query_list() {
        let mut app = App {
            screen: Screen::QueryList { connection: "db".to_string(), queries: vec![], selected: 0, preview: String::new() },
            status: None,
        };
        let result = app.handle_key(ctrl(KeyCode::Char('q')), &mut make_terminal()).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn ctrl_q_quits_from_results() {
        let mut app = App {
            screen: Screen::Results {
                connection: "db".to_string(),
                query: "q".to_string(),
                result: crate::models::QueryResult::AffectedRows(0),
            },
            status: None,
        };
        let result = app.handle_key(ctrl(KeyCode::Char('q')), &mut make_terminal()).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn plain_q_does_not_quit_from_create_connection() {
        let mut app = in_create_connection();
        let result = app.handle_key(key(KeyCode::Char('q')), &mut make_terminal()).await.unwrap();
        assert!(result);
    }
}
