# calendar-gpui

A small desktop calendar built with [GPUI](https://gpui.rs) that syncs over standard CalDAV
(Radicale, Nextcloud, Baïkal, ...).

## Config

Create `config.toml` in your config directory (`%APPDATA%\calendar-gpui` on Windows,
`~/.config/calendar-gpui` on Linux):

```toml
url = "http://localhost:5232"
username = "user"
password = "password"
```

## Features

- Month view with event markers; agenda for the selected day
- Syncs on start, on month change, after every change, and every 60 seconds
- Add all-day events (`+ New`, type a title, Enter) and delete events (`✕`)
- Recurring events are expanded by the server
