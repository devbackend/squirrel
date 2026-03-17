# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build          # compile
cargo run            # build and run
cargo test           # run all tests
cargo test <name>    # run a single test by name filter
cargo clippy         # lint
cargo fmt            # format code
```

## Project

**Squirrel** is a terminal UI (TUI) PostgreSQL client built with ratatui + crossterm. It lets users manage named connections and saved SQL queries, then run them against a Postgres database.

### Architecture

The app is a state machine driven by a screen enum. Each screen variant owns all data for that view; transitions happen in `App::handle_key` by taking ownership of the current screen (`std::mem::replace`) and returning the next one.

- **`app.rs`** — `App` struct + `Screen` enum. Each screen variant has a dedicated `on_<screen>` async handler. Quitting returns `None` from the handler; staying returns `Some(next_screen)`.
- **`ui.rs`** — Pure rendering: `render(frame, screen, status)` dispatches to screen-specific render functions. No state mutation here.
- **`db.rs`** — Async Postgres operations (`test_connection`, `execute_query`). Each call opens a new connection. SELECT-like statements use `client.query`, everything else uses `client.execute`.
- **`storage.rs`** — File-system persistence under `~/.squirrel/connections/<name>/`. Each connection has a `config.toml` (TOML-serialized `ConnectionConfig`) and a `queries/` directory of `.sql` files. Tests in this module use a `HOME_LOCK` mutex + temp dir to avoid races when mutating `$HOME`.
- **`models.rs`** — `ConnectionConfig`, `QueryResult` (with pagination helpers), `ConnectionForm` (6-field form state).

### Screen flow

```
ConnectionList → CreateConnection (n)
             └→ QueryList (Enter) → CreateQueryName (n)
                                 └→ QueryView (Enter) → Results (Enter)
```

`e` on ConnectionList / QueryView suspends the TUI, opens `$EDITOR`/`$VISUAL`/`vi`, then restores raw mode.

### Storage layout

```
~/.squirrel/
  connections/
    <name>/
      config.toml
      queries/
        <query>.sql
```

### Key dependencies

| Crate | Purpose |
|---|---|
| `ratatui` | TUI widgets and layout |
| `crossterm` | Terminal raw mode and key events |
| `tokio` | Async runtime (full features) |
| `tokio-postgres` | Postgres driver (no TLS) |
| `serde` + `toml` | Config serialization |
| `anyhow` | Error handling |
| `tempfile` | Test isolation (dev-dependency) |
