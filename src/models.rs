use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl ConnectionConfig {
    pub fn connection_string(&self) -> String {
        format!(
            "host={} port={} dbname={} user={} password={}",
            self.host, self.port, self.database, self.username, self.password
        )
    }
}

#[derive(Debug, Clone)]
pub enum QueryResult {
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        page: usize,
        page_size: usize,
    },
    AffectedRows(u64),
}

impl QueryResult {
    pub const PAGE_SIZE: usize = 20;

    pub fn page_count(&self) -> usize {
        match self {
            QueryResult::Rows { rows, page_size, .. } => {
                if rows.is_empty() {
                    1
                } else {
                    rows.len().div_ceil(*page_size)
                }
            }
            QueryResult::AffectedRows(_) => 1,
        }
    }

    #[allow(dead_code)]
    pub fn current_page(&self) -> usize {
        match self {
            QueryResult::Rows { page, .. } => *page,
            QueryResult::AffectedRows(_) => 0,
        }
    }

    pub fn current_page_rows(&self) -> &[Vec<String>] {
        match self {
            QueryResult::Rows { rows, page, page_size, .. } => {
                let start = page * page_size;
                let end = (start + page_size).min(rows.len());
                &rows[start..end]
            }
            QueryResult::AffectedRows(_) => &[],
        }
    }

    pub fn next_page(&mut self) {
        if let QueryResult::Rows { page, page_size, rows, .. } = self {
            let max_page = if rows.is_empty() { 0 } else { (rows.len() - 1) / *page_size };
            if *page < max_page {
                *page += 1;
            }
        }
    }

    pub fn prev_page(&mut self) {
        if let QueryResult::Rows { page, .. } = self
            && *page > 0 {
                *page -= 1;
            }
    }
}

pub const FORM_FIELD_NAMES: [&str; 6] = ["Name", "Host", "Port", "Database", "Username", "Password"];

#[derive(Clone, Debug)]
pub struct ConnectionForm {
    pub values: [String; 6],
    pub active_field: usize,
    pub editing: bool,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            values: [
                String::new(),
                String::from("localhost"),
                String::from("5432"),
                String::new(),
                String::from("postgres"),
                String::new(),
            ],
            active_field: 0,
            editing: false,
        }
    }
}

impl ConnectionForm {
    #[allow(dead_code)]
    pub fn from_config(cfg: &ConnectionConfig) -> Self {
        Self {
            values: [
                cfg.name.clone(),
                cfg.host.clone(),
                cfg.port.to_string(),
                cfg.database.clone(),
                cfg.username.clone(),
                cfg.password.clone(),
            ],
            active_field: 0,
            editing: false,
        }
    }

    pub fn to_config(&self) -> Result<ConnectionConfig, String> {
        if self.values[0].is_empty() {
            return Err("Name is required".to_string());
        }
        let port = self.values[2]
            .parse::<u16>()
            .map_err(|_| "Port must be a number (1–65535)".to_string())?;
        if self.values[3].is_empty() {
            return Err("Database is required".to_string());
        }
        Ok(ConnectionConfig {
            name: self.values[0].clone(),
            host: self.values[1].clone(),
            port,
            database: self.values[3].clone(),
            username: self.values[4].clone(),
            password: self.values[5].clone(),
        })
    }

    pub fn next_field(&mut self) {
        self.active_field = (self.active_field + 1) % FORM_FIELD_NAMES.len();
    }

    pub fn prev_field(&mut self) {
        if self.active_field == 0 {
            self.active_field = FORM_FIELD_NAMES.len() - 1;
        } else {
            self.active_field -= 1;
        }
    }

    pub fn active_value_mut(&mut self) -> &mut String {
        &mut self.values[self.active_field]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ConnectionForm ──────────────────────────────────────────────────────────

    #[test]
    fn form_default_values() {
        let form = ConnectionForm::default();
        assert_eq!(form.active_field, 0);
        assert!(!form.editing);
        assert_eq!(form.values[1], "localhost");
        assert_eq!(form.values[2], "5432");
        assert_eq!(form.values[4], "postgres");
        assert!(form.values[0].is_empty());
        assert!(form.values[3].is_empty());
        assert!(form.values[5].is_empty());
    }

    #[test]
    fn form_next_field_wraps() {
        let mut form = ConnectionForm::default();
        for expected in 1..=5 {
            form.next_field();
            assert_eq!(form.active_field, expected);
        }
        form.next_field();
        assert_eq!(form.active_field, 0);
    }

    #[test]
    fn form_prev_field_wraps() {
        let mut form = ConnectionForm::default();
        form.prev_field();
        assert_eq!(form.active_field, 5);
        form.prev_field();
        assert_eq!(form.active_field, 4);
    }

    #[test]
    fn form_to_config_success() {
        let mut form = ConnectionForm::default();
        form.values[0] = "mydb".to_string();
        form.values[3] = "testdb".to_string();
        let cfg = form.to_config().unwrap();
        assert_eq!(cfg.name, "mydb");
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.database, "testdb");
        assert_eq!(cfg.username, "postgres");
    }

    #[test]
    fn form_to_config_missing_name() {
        let form = ConnectionForm::default();
        assert!(form.to_config().is_err());
    }

    #[test]
    fn form_to_config_missing_database() {
        let mut form = ConnectionForm::default();
        form.values[0] = "mydb".to_string();
        assert!(form.to_config().is_err());
    }

    #[test]
    fn form_to_config_invalid_port() {
        let mut form = ConnectionForm::default();
        form.values[0] = "mydb".to_string();
        form.values[2] = "notaport".to_string();
        form.values[3] = "testdb".to_string();
        assert!(form.to_config().is_err());
    }

    // ── QueryResult pagination ──────────────────────────────────────────────────

    fn rows_result(count: usize) -> QueryResult {
        QueryResult::Rows {
            columns: vec!["id".to_string()],
            rows: (0..count).map(|i| vec![i.to_string()]).collect(),
            page: 0,
            page_size: 20,
        }
    }

    #[test]
    fn page_count_empty() {
        let r = QueryResult::Rows { columns: vec![], rows: vec![], page: 0, page_size: 20 };
        assert_eq!(r.page_count(), 1);
    }

    #[test]
    fn page_count_exact_multiple() {
        assert_eq!(rows_result(20).page_count(), 1);
        assert_eq!(rows_result(40).page_count(), 2);
    }

    #[test]
    fn page_count_remainder() {
        assert_eq!(rows_result(21).page_count(), 2);
        assert_eq!(rows_result(25).page_count(), 2);
    }

    #[test]
    fn current_page_rows_first_page() {
        assert_eq!(rows_result(25).current_page_rows().len(), 20);
    }

    #[test]
    fn current_page_rows_last_page() {
        let mut r = rows_result(25);
        r.next_page();
        assert_eq!(r.current_page_rows().len(), 5);
    }

    #[test]
    fn next_page_clamps_at_last() {
        let mut r = rows_result(25);
        r.next_page();
        r.next_page(); // should not go past page 1
        assert_eq!(r.current_page(), 1);
        assert_eq!(r.current_page_rows().len(), 5);
    }

    #[test]
    fn prev_page_clamps_at_zero() {
        let mut r = rows_result(25);
        r.prev_page(); // should not underflow
        assert_eq!(r.current_page(), 0);
    }
}
