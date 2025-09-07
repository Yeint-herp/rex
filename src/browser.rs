use eframe::egui::{self, Context, Key, Ui};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(PartialEq)]
enum Interaction {
    None,
    Rename { path: PathBuf, buffer: String },
}

pub struct FileBrowser {
    entries: Vec<fs::DirEntry>,
    pub selected: Option<usize>,
    interaction: Interaction,
    last_path: Option<PathBuf>,
}

impl FileBrowser {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: None,
            interaction: Interaction::None,
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
        all.sort_by_key(|e| (!e.path().is_dir(), e.path()));
        self.entries = all;
    }

    pub fn update(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        cwd: &Path,
        on_open: &mut Option<PathBuf>,
        on_pin: &mut Option<PathBuf>,
        on_rename_request: &mut Option<(PathBuf, String)>,
        on_delete_request: &mut Option<PathBuf>,
        on_open_with_request: &mut Option<PathBuf>,
        on_open_terminal: &mut Option<PathBuf>,

        on_copy_request: &mut Option<PathBuf>,
        on_cut_request: &mut Option<PathBuf>,
        on_paste_here: &mut Option<PathBuf>,
        on_undo_request: &mut bool,
        has_clipboard: bool,
        on_new_folder_here: &mut Option<PathBuf>,
        on_new_file_here: &mut Option<PathBuf>,
    ) {
        if self.entries.is_empty()
            || !cwd.exists()
            || self.last_path.as_ref().map_or(true, |p| p != cwd)
        {
            self.reload(cwd);
            self.last_path = Some(cwd.to_path_buf());
            self.selected = None;
        }

        let snapshot: Vec<(usize, PathBuf, bool, String)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let path = e.path();
                let is_dir = path.is_dir();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                (i, path, is_dir, name)
            })
            .collect();

        let in_rename = egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let bg_id = ui.make_persistent_id("filebrowser-bg");
                let bg_rect = ui.max_rect();
                let bg_resp = ui.interact(bg_rect, bg_id, egui::Sense::click());

                bg_resp.context_menu(|ui| {
                    if ui
                        .add_enabled(has_clipboard, egui::Button::new("📋 Paste here"))
                        .clicked()
                    {
                        *on_paste_here = Some(cwd.to_path_buf());
                        ui.close_menu();
                    }
                    if ui.button("📄 New file...").clicked() {
                        *on_new_file_here = Some(cwd.to_path_buf());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📁 New folder...").clicked() {
                        *on_new_folder_here = Some(cwd.to_path_buf());
                        ui.close_menu();
                    }
                    if ui.button("🖥 Terminal here").clicked() {
                        *on_open_terminal = Some(cwd.to_path_buf());
                        ui.close_menu();
                    }
                    if ui.button("↻ Refresh").clicked() {
                        self.invalidate();
                        ui.close_menu();
                    }

                    if let Some(parent) = cwd.parent() {
                        ui.separator();
                        if ui.button("📂 Open parent").clicked() {
                            *on_open = Some(parent.to_path_buf());
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_clipboard, egui::Button::new("📋 Paste into parent"))
                            .clicked()
                        {
                            *on_paste_here = Some(parent.to_path_buf());
                            ui.close_menu();
                        }
                    }
                });

                if bg_resp.clicked() {
                    self.selected = None;
                }

                for (i, path, is_dir, name) in snapshot {
                    let icon = if is_dir { "📁" } else { "📄" };
                    let label = format!("{icon} {name}");

                    let response: egui::Response = ui
                        .with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            ui.add(egui::SelectableLabel::new(
                                self.selected == Some(i),
                                label.clone(),
                            ))
                        })
                        .inner;
                    if response.clicked() {
                        self.selected = Some(i);
                    }

                    if response.double_clicked() {
                        if is_dir {
                            *on_open = Some(path.clone());
                        } else {
                            super::platform::open_file(&path);
                        }
                    }

                    response.context_menu(|ui| {
                        if ui.button("📝 Rename").clicked() {
                            let initial = name.clone();
                            self.interaction = Interaction::Rename {
                                path: path.clone(),
                                buffer: initial,
                            };
                            ui.close_menu();
                        }
                        if ui.button("📎 Open with...").clicked() {
                            *on_open_with_request = Some(path.clone());
                            ui.close_menu();
                        }
                        if ui.button("❌ Delete").clicked() {
                            *on_delete_request = Some(path.clone());
                            ui.close_menu();
                        }
                        if is_dir && ui.button("📌 Pin").clicked() {
                            *on_pin = Some(path.clone());
                            ui.close_menu();
                        }
                        if is_dir && ui.button("🖥 Terminal here").clicked() {
                            *on_open_terminal = Some(path.clone());
                            ui.close_menu();
                        }

                        ui.separator();

                        if ui.button("📄 Copy").clicked() {
                            *on_copy_request = Some(path.clone());
                            ui.close_menu();
                        }
                        if ui.button("✂ Cut").clicked() {
                            *on_cut_request = Some(path.clone());
                            ui.close_menu();
                        }
                        if is_dir {
                            ui.separator();
                            if ui.button("📁 New folder here...").clicked() {
                                *on_new_folder_here = Some(path.clone());
                                ui.close_menu();
                            }
                            if ui.button("📄 New file here...").clicked() {
                                *on_new_file_here = Some(path.clone());
                                ui.close_menu();
                            }
                        }
                        let target_dir = if is_dir {
                            path.clone()
                        } else {
                            path.parent().unwrap_or(cwd).to_path_buf()
                        };
                        if ui
                            .add_enabled(has_clipboard, egui::Button::new("📋 Paste here"))
                            .clicked()
                        {
                            *on_paste_here = Some(target_dir);
                            ui.close_menu();
                        }

                        if ui.button("⟲ Undo last operation").clicked() {
                            *on_undo_request = true;
                            ui.close_menu();
                        }
                    });

                    if let Interaction::Rename {
                        path: target,
                        buffer,
                    } = &mut self.interaction
                    {
                        if &path == target {
                            let text_resp =
                                ui.add(egui::TextEdit::singleline(buffer).desired_width(300.0));
                            if text_resp.lost_focus() || ctx.input(|i| i.key_pressed(Key::Enter)) {
                                *on_rename_request = Some((target.clone(), buffer.clone()));
                                self.interaction = Interaction::None;
                            }
                            return true;
                        }
                    }
                }
                false
            })
            .inner;

        if !in_rename {
            ctx.input(|i| {
                if let Some(index) = self.selected {
                    if index < self.entries.len() {
                        let path = self.entries[index].path();
                        if i.key_pressed(Key::Delete) {
                            *on_delete_request = Some(path);
                        } else if i.key_pressed(Key::F2) {
                            let nm = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            self.interaction = Interaction::Rename { path, buffer: nm };
                        }
                    } else {
                        self.selected = None;
                    }
                }
            });
        }
    }
}
