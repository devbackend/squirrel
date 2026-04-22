use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::models::ConnectionConfig;

pub fn squirrel_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".squirrel")
}

pub fn connections_dir() -> PathBuf {
    squirrel_dir().join("connections")
}

pub fn connection_dir(name: &str) -> PathBuf {
    connections_dir().join(name)
}

pub fn connection_config_path(name: &str) -> PathBuf {
    connection_dir(name).join("config.toml")
}

pub fn queries_dir(connection: &str) -> PathBuf {
    connection_dir(connection).join("queries")
}

pub fn query_path(connection: &str, query_name: &str) -> PathBuf {
    queries_dir(connection).join(format!("{query_name}.sql"))
}

pub fn list_connections() -> Result<Vec<String>> {
    let dir = connections_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&dir).context("reading connections dir")? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
                && connection_config_path(name).exists() {
                    names.push(name.to_string());
                }
    }
    names.sort();
    Ok(names)
}

pub fn load_connection(name: &str) -> Result<ConnectionConfig> {
    let path = connection_config_path(name);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

pub fn save_connection(cfg: &ConnectionConfig) -> Result<()> {
    let dir = connection_dir(&cfg.name);
    std::fs::create_dir_all(&dir).context("creating connection dir")?;
    std::fs::create_dir_all(queries_dir(&cfg.name)).context("creating queries dir")?;
    let path = connection_config_path(&cfg.name);
    let content = toml::to_string_pretty(cfg).context("serializing config")?;
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))
}

pub fn delete_connection(name: &str) -> Result<()> {
    let dir = connection_dir(name);
    std::fs::remove_dir_all(&dir)
        .with_context(|| format!("deleting {}", dir.display()))
}

/// Renames a connection directory from `old` to `new`.
///
/// # Errors
///
/// Returns an error if `new` already exists or if the rename syscall fails.
pub fn rename_connection(old: &str, new: &str) -> Result<()> {
    let old_dir = connection_dir(old);
    let new_dir = connection_dir(new);
    if new_dir.exists() {
        anyhow::bail!("A connection named '{new}' already exists");
    }
    std::fs::rename(&old_dir, &new_dir)
        .with_context(|| format!("renaming '{}' to '{}'", old_dir.display(), new_dir.display()))?;
    // The config.toml inside still records the old name — update it.
    let cfg_path = connection_config_path(new);
    let content = std::fs::read_to_string(&cfg_path)
        .with_context(|| format!("reading {}", cfg_path.display()))?;
    let mut cfg: crate::models::ConnectionConfig =
        toml::from_str(&content).with_context(|| format!("parsing {}", cfg_path.display()))?;
    cfg.name = new.to_string();
    let updated = toml::to_string_pretty(&cfg).context("serializing config")?;
    std::fs::write(&cfg_path, updated)
        .with_context(|| format!("writing {}", cfg_path.display()))
}

pub fn list_queries(connection: &str) -> Result<Vec<String>> {
    let dir = queries_dir(connection);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&dir).context("reading queries dir")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sql")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
    }
    names.sort();
    Ok(names)
}

pub fn load_query(connection: &str, name: &str) -> Result<String> {
    let path = query_path(connection, name);
    std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))
}

pub fn save_query(connection: &str, name: &str, sql: &str) -> Result<()> {
    let dir = queries_dir(connection);
    std::fs::create_dir_all(&dir).context("creating queries dir")?;
    let path = query_path(connection, name);
    std::fs::write(&path, sql)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn delete_query(connection: &str, name: &str) -> Result<()> {
    let path = query_path(connection, name);
    std::fs::remove_file(&path)
        .with_context(|| format!("deleting {}", path.display()))
}

/// Renames a query `.sql` file within a connection's queries directory.
///
/// # Errors
///
/// Returns an error if a query named `new` already exists or if the rename
/// syscall fails.
pub fn rename_query(connection: &str, old: &str, new: &str) -> Result<()> {
    let old_path = query_path(connection, old);
    let new_path = query_path(connection, new);
    if new_path.exists() {
        anyhow::bail!("A query named '{new}' already exists");
    }
    std::fs::rename(&old_path, &new_path)
        .with_context(|| format!("renaming '{}' to '{}'", old_path.display(), new_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_home<F: FnOnce()>(f: F) {
        let _lock = HOME_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: single-threaded via HOME_LOCK mutex, no other threads access HOME concurrently
        unsafe { std::env::set_var("HOME", tmp.path()) };
        f();
        // tmp drops and cleans up automatically
    }

    fn sample_config(name: &str) -> ConnectionConfig {
        ConnectionConfig {
            name: name.to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "postgres".to_string(),
            password: "secret".to_string(),
        }
    }

    #[test]
    fn test_list_connections_empty() {
        with_temp_home(|| {
            assert_eq!(list_connections().unwrap(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_save_and_load_connection() {
        with_temp_home(|| {
            let cfg = sample_config("mydb");
            save_connection(&cfg).unwrap();
            let loaded = load_connection("mydb").unwrap();
            assert_eq!(loaded.name, "mydb");
            assert_eq!(loaded.host, "localhost");
            assert_eq!(loaded.port, 5432);
            assert_eq!(loaded.database, "testdb");
            assert_eq!(loaded.username, "postgres");
        });
    }

    #[test]
    fn test_list_connections_sorted() {
        with_temp_home(|| {
            save_connection(&sample_config("bbb")).unwrap();
            save_connection(&sample_config("aaa")).unwrap();
            save_connection(&sample_config("ccc")).unwrap();
            let list = list_connections().unwrap();
            assert_eq!(list, vec!["aaa", "bbb", "ccc"]);
        });
    }

    #[test]
    fn test_delete_connection() {
        with_temp_home(|| {
            save_connection(&sample_config("gone")).unwrap();
            assert_eq!(list_connections().unwrap(), vec!["gone"]);
            delete_connection("gone").unwrap();
            assert_eq!(list_connections().unwrap(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_queries_crud() {
        with_temp_home(|| {
            save_connection(&sample_config("db")).unwrap();

            assert_eq!(list_queries("db").unwrap(), Vec::<String>::new());

            save_query("db", "select_all", "SELECT * FROM users").unwrap();
            let queries = list_queries("db").unwrap();
            assert_eq!(queries, vec!["select_all"]);

            let sql = load_query("db", "select_all").unwrap();
            assert_eq!(sql, "SELECT * FROM users");

            delete_query("db", "select_all").unwrap();
            assert_eq!(list_queries("db").unwrap(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_rename_connection() {
        with_temp_home(|| {
            save_connection(&sample_config("alpha")).unwrap();
            assert_eq!(list_connections().unwrap(), vec!["alpha"]);

            rename_connection("alpha", "beta").unwrap();

            let list = list_connections().unwrap();
            assert_eq!(list, vec!["beta"]);

            // config.toml inside should have the updated name
            let cfg = load_connection("beta").unwrap();
            assert_eq!(cfg.name, "beta");
        });
    }

    #[test]
    fn test_rename_connection_conflicts() {
        with_temp_home(|| {
            save_connection(&sample_config("alpha")).unwrap();
            save_connection(&sample_config("beta")).unwrap();

            let err = rename_connection("alpha", "beta").unwrap_err();
            assert!(err.to_string().contains("already exists"), "unexpected error: {err}");

            // Both should still exist
            let list = list_connections().unwrap();
            assert!(list.contains(&"alpha".to_string()));
            assert!(list.contains(&"beta".to_string()));
        });
    }

    #[test]
    fn test_rename_query() {
        with_temp_home(|| {
            save_connection(&sample_config("db")).unwrap();
            save_query("db", "old_query", "SELECT 1").unwrap();
            assert_eq!(list_queries("db").unwrap(), vec!["old_query"]);

            rename_query("db", "old_query", "new_query").unwrap();

            let queries = list_queries("db").unwrap();
            assert_eq!(queries, vec!["new_query"]);

            let sql = load_query("db", "new_query").unwrap();
            assert_eq!(sql, "SELECT 1");
        });
    }

    #[test]
    fn test_rename_query_conflicts() {
        with_temp_home(|| {
            save_connection(&sample_config("db")).unwrap();
            save_query("db", "alpha", "SELECT 1").unwrap();
            save_query("db", "beta", "SELECT 2").unwrap();

            let err = rename_query("db", "alpha", "beta").unwrap_err();
            assert!(err.to_string().contains("already exists"), "unexpected error: {err}");

            // Both should still exist
            let queries = list_queries("db").unwrap();
            assert!(queries.contains(&"alpha".to_string()));
            assert!(queries.contains(&"beta".to_string()));
        });
    }

    #[test]
    fn test_queries_sorted() {
        with_temp_home(|| {
            save_connection(&sample_config("db")).unwrap();
            save_query("db", "zzz", "SELECT 3").unwrap();
            save_query("db", "aaa", "SELECT 1").unwrap();
            save_query("db", "mmm", "SELECT 2").unwrap();
            let queries = list_queries("db").unwrap();
            assert_eq!(queries, vec!["aaa", "mmm", "zzz"]);
        });
    }
}
