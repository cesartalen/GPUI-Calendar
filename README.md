# gpuical

A small desktop calendar built with [GPUI](https://gpui.rs) that syncs over standard CalDAV
(Radicale, Nextcloud, Baïkal, ...).

## Config

Create `config.toml` in your config directory (`%APPDATA%\gpuical` on Windows,
`~/.config/gpuical` on Linux):

```toml
url = "http://localhost:5232"
username = "user"
password = "password"
theme = "system" # optional: "light", "dark" or "system" (default)
```

## Features

- Month view with event markers; agenda for the selected day
- Syncs on start, on month change, after every change, and every 60 seconds
- Add events (`+ New`, type a title, pick all day or a time, Enter), edit them (`✎`, pick another day to move) and delete them (`✕`)
- Light and dark themes: follows the system, with a toggle in the titlebar
- Recurring events are expanded by the server (read-only for now)
