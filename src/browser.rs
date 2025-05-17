use eframe::egui::{self, Context, Key, Ui};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(PartialEq)]
enum FileInteraction {
    None,
    Rename { path: PathBuf, buffer: String },
}

pub struct FileBrowser {
    entries: Vec<fs::DirEntry>,
    selected: Option<usize>,
    interaction: FileInteraction,
    last_path: Option<PathBuf>,
}

fn open_terminal_in(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let terminals = [
            ("wt", &["-d", path.to_str().unwrap()])
            ("powershell", &["-NoExit", "-Command", &format!("cd '{}'", path.display())]),
            ("cmd", &["/K", &format!("cd /d {}", path.display())]),
        ];

        for (cmd, args) in terminals {
            if std::process::Command::new(cmd).args(args).spawn().is_ok() {
                return;
            }
        }

        eprintln!("⚠ Failed to launch any known terminal on Windows.");
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application "Terminal" to do script "cd '{}'; clear""#,
            path.display()
        );
        let result = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();

        if result.is_err() {
            eprintln!("⚠ Failed to launch Terminal on macOS.");
        }
    }

    #[cfg(target_os = "linux")]
    {
        let terminals = [
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "xterm",
            "x-terminal-emulator",
            "foot",
            "kitty",
        ];

        for term in terminals {
            let try_spawn = std::process::Command::new(term)
                .current_dir(path)
                .spawn();

            if try_spawn.is_ok() {
                return;
            }
        }

        eprintln!("⚠ Failed to launch any known terminal on Linux.");
    }
}

impl FileBrowser {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: None,
            interaction: FileInteraction::None,
            last_path: None,
        }
    }

    pub fn invalidate(&mut self) {
        self.entries.clear();
    }

    fn reload(&mut self, cwd: &Path) {
        let mut all = fs::read_dir(cwd)
            .unwrap_or_else(|_| fs::read_dir(Path::new("/")).unwrap())
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        all.sort_by_key(|e| e.path());
        self.entries = all;
    }

    pub fn update(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        cwd: &Path,
    ) -> Option<(Option<PathBuf>, Option<PathBuf>)> {
        if self.entries.is_empty()
            || !cwd.exists()
            || self.last_path.as_ref().map_or(true, |p| p != cwd)
        {
            self.reload(cwd);
            self.last_path = Some(cwd.to_path_buf());
            self.selected = None;
        }

        let mut path_to_navigate = None;
        let mut pin_request = None;
        let mut refresh_entries = false;

        let snapshot = self.entries.iter().enumerate().collect::<Vec<_>>();
        let in_rename = egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, entry) in snapshot {
                let path = entry.path();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let is_dir = path.is_dir();
                let icon = if is_dir { "📁" } else { "📄" };
                let label = format!("{icon} {name}");

                let response = ui.selectable_label(self.selected == Some(i), label);

                if response.clicked() {
                    self.selected = Some(i);
                }

                if response.double_clicked() {
                    if is_dir {
                        path_to_navigate = Some(path.clone());
                    } else {
                        #[cfg(target_os = "windows")]
                        let _ = std::process::Command::new("explorer").arg(&path).spawn();
                        #[cfg(target_os = "linux")]
                        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                        #[cfg(target_os = "macos")]
                        let _ = std::process::Command::new("open").arg(&path).spawn();
                    }
                }

                if let Some(menu) = response.context_menu(|ui| {
                    if ui.button("📝 Rename").clicked() {
                        let initial_name = path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        self.interaction = FileInteraction::Rename { path: path.clone(), buffer: initial_name };
                        ui.close_menu();
                    }
                    if ui.button("❌ Delete").clicked() {
                        let _ = fs::remove_file(&path).or_else(|_| fs::remove_dir_all(&path));
                        refresh_entries = true;
                        ui.close_menu();
                    }

                    if is_dir {
                        if ui.button("📌 Pin").clicked() {
                            pin_request = Some(path.clone());
                            ui.close_menu();
                        }

                        if ui.button("🖥 Terminal").clicked() {
                            open_terminal_in(&path);
                            ui.close_menu();
                        }
                    }
                }) {
                    if menu.response.hovered() {
                        self.selected = Some(i);
                    }
                }

                if let FileInteraction::Rename { path: target_path, buffer } = &mut self.interaction {
                    if &path == target_path {
                        let text_resp = ui.text_edit_singleline(buffer);

                        if text_resp.lost_focus() || ctx.input(|i| i.key_pressed(Key::Enter)) {
                            if !buffer.is_empty() && *buffer != name {
                                let new_path = target_path.with_file_name(&*buffer);
                                let _ = fs::rename(target_path, new_path);
                            }
                            self.interaction = FileInteraction::None;
                            refresh_entries = true;
                        }

                        return true;
                    }
                }
            }

            false
        }).inner;

        if !in_rename {
            ctx.input(|i| {
                if let Some(index) = self.selected {
                    if index < self.entries.len() {
                        let path = self.entries[index].path();
                        if i.key_pressed(Key::Delete) {
                            let _ = fs::remove_file(&path).or_else(|_| fs::remove_dir_all(&path));
                            refresh_entries = true;
                        }
                        if i.key_pressed(Key::F2) {
                            let initial_name = path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            self.interaction = FileInteraction::Rename { path, buffer: initial_name };
                        }
                    } else {
                        self.selected = None;
                    }
                }
            });
        }

        if refresh_entries && self.interaction == FileInteraction::None {
            self.entries.clear();
        }

        Some((path_to_navigate, pin_request))
    }
}
