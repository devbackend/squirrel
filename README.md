<p align="center">
  <img src="assets/logo.png" alt="Squirrel" width="200"/>
</p>

# Squirrel

A terminal UI (TUI) client for PostgreSQL. Manage named connections and saved SQL queries, then run them against a database — all from the terminal.

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"/></a>
  <a href="https://github.com/devbackend/squirrel/tags"><img src="https://img.shields.io/github/v/tag/devbackend/squirrel" alt="version"/></a>
</p>

## Quick Start

```bash
# 1. Clone and install
git clone https://github.com/devbackend/squirrel
cd squirrel
cargo install --path .

# 2. Launch
squirrel

# 3. Press `n` to add your first connection and you're in
```

> Requires Rust (stable) and a running PostgreSQL instance.

## How it works

**Add a connection** — press `n` on the connections screen, fill in the fields (host, port, database, credentials). Squirrel tests the connection before saving, so you know it works right away.

**Save a query** — open a connection, press `n` to create a new query, give it a name. Your `$EDITOR` opens automatically — write the SQL, save and close.

**Run it** — select the query, press `Enter`. Results appear in a paginated table. Non-SELECT statements show the number of affected rows.

**Edit anytime** — press `e` on any connection or query to open it in your editor. Squirrel steps aside and comes back when you're done.

## Keyboard shortcuts

| Screen | Key | Action |
|--------|-----|--------|
| Any | `j` / `k` or `↑` / `↓` | Navigate up / down |
| Any | `Esc` / `q` | Go back |
| Connections | `n` | New connection |
| Connections | `e` | Edit connection in `$EDITOR` |
| Connections | `d` | Delete connection |
| Connections | `Enter` | Open queries |
| Create connection | `Enter` | Start / finish editing a field |
| Create connection | `s` | Test & save |
| Queries | `n` | New query |
| Queries | `d` | Delete query |
| Queries | `Enter` | Open query |
| Query view | `Enter` | Run query |
| Query view | `e` | Edit SQL in `$EDITOR` |
| Results | `l` / `→` | Next page |
| Results | `h` / `←` | Previous page |

## Editor integration

Squirrel uses `$EDITOR` (falling back to `$VISUAL`, then `vi`). The TUI suspends while the editor is open and resumes when you close it.
