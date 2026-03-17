# Squirrel

A terminal UI (TUI) client for PostgreSQL. Manage named connections and saved SQL queries, then run them against a database — all from the terminal.

## Requirements

- Rust (stable)
- A running PostgreSQL instance

## Installation

```bash
git clone <repo>
cd squirrel
cargo build --release
```

Run directly:

```bash
cargo run
```

Or install to `~/.cargo/bin`:

```bash
cargo install --path .
squirrel
```

## Usage

### Navigation

All navigation uses keyboard only.

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | Select / confirm |
| `Esc` / `q` | Back / quit |

### Connections screen (startup)

| Key | Action |
|-----|--------|
| `n` | Create new connection |
| `e` | Edit connection config in `$EDITOR` |
| `d` | Delete connection |
| `Enter` | Open query list for this connection |
| `q` | Quit |

### Creating a connection

Fill in the 6 fields (Name, Host, Port, Database, Username, Password). Navigate between fields with `Tab` / `↑↓`. Press `Enter` on the last field to test and save the connection. The app verifies connectivity before saving.

### Query list screen

| Key | Action |
|-----|--------|
| `n` | Create new query (prompts for name, then opens `$EDITOR`) |
| `d` | Delete query |
| `Enter` | Open query |
| `←` / `h` / `Esc` | Back to connections |

### Query view screen

| Key | Action |
|-----|--------|
| `Enter` | Run query |
| `e` | Edit SQL in `$EDITOR` |
| `q` / `Esc` | Back to query list |

### Results screen

| Key | Action |
|-----|--------|
| `l` / `→` | Next page |
| `h` / `←` | Previous page |
| `q` / `Esc` | Back to query view |

Results are paginated at 20 rows per page. Non-SELECT statements show the number of affected rows.

## Data storage

All data is stored locally under `~/.squirrel/`:

```
~/.squirrel/
  connections/
    <name>/
      config.toml    # connection credentials
      queries/
        <query>.sql  # saved SQL files
```

You can edit `config.toml` directly or use the `e` key in the app.

## Editor integration

Squirrel uses `$EDITOR` (falling back to `$VISUAL`, then `vi`) to edit SQL files and connection configs. The TUI suspends while the editor is open and resumes when you close it.
