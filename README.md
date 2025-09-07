# REX File Explorer

**REX** is a fast and native file explorer built using [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) and [`egui`](https://github.com/emilk/egui). Designed to be minimal, extensible, and intuitive, REX provides a simple graphical interface with powerful navigation and search features.

---

## Features

- Tree-less directory browser
- Asynchronous recursive search (non-blocking UI)
- Pin frequently visited folders
- Undo / Redo navigation stack
- Autocomplete path input with fuzzy matching
- Dynamic UI scaling (`Ctrl` + `+` / `-` / `0`)
- Launch terminal in current folder (Linux, Windows, macOS supported)
- Clean and self-contained configuration & state files

---

## Installation

Build and install system-wide using the provided install script:

```bash
./install.sh
```

This will:

- Compile the project in release mode
- Install the binary into `/usr/local/bin/rex`
- Automatically install a `.desktop` file shortcut (if XFCE or KDE is detected)

> Note: root is required to install into system directories
> Note: macOS and Windows installers not supported.

## configuration

| File                | Purpose                    |
| ------------------- | -------------------------- |
| `~/.rex/pinned.ini` | Stores pinned folder paths |
| `~/.rex/config.ini` | Stores UI scale factor     |

> Note: Windows equivalent is located in `%AppData%\\rex\\`

# TODO

- [X] Creating folders and files
- [ ] Move files and folders
- [X] Copy files and folders
- [ ] Drag n Drop for pins and moving
- [ ] File preview panel
- [ ] Properties pannel
- [ ] Personalization of visuals
- [ ] Extension plugin system


