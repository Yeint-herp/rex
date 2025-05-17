use eframe::{egui, Frame};
use egui::{Align, Button, Context, Layout, TopBottomPanel};
use std::path::PathBuf;
use std::sync::mpsc::{Sender, Receiver};
use std::thread;
use std::sync::mpsc;

pub enum ViewMode {
    Browsing,
    Searching {
        results: Vec<PathBuf>,
        receiver: Receiver<PathBuf>,
    },
}

mod browser;

struct AppData {
    current_path: PathBuf,
    search_query: String,
    pinned: Vec<PathBuf>,
    path_edit: String,
    undo_stack: Vec<(PathBuf, ViewMode)>,
    redo_stack: Vec<(PathBuf, ViewMode)>,
    autocomplete: Vec<String>,
    scale_factor: f32,
    browser: browser::FileBrowser,
    search: ViewMode,
}

fn get_os_root() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from("C:\\")
    }

    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/")
    }
}

fn load_pinned_folders() -> Vec<PathBuf> {
    let path = get_pinned_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }

    if !path.exists() {
        return vec![dirs::home_dir().unwrap_or_default(), get_os_root()];
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![dirs::home_dir().unwrap_or_default(), get_os_root()];
    };

    let lines: Vec<_> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        return vec![dirs::home_dir().unwrap_or_default(), get_os_root()];
    }

    lines.into_iter().map(PathBuf::from).collect()
}

fn save_pinned_folders(pinned: &[PathBuf]) {
    let path = get_pinned_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }

    let content = pinned
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(path, content).ok();
}

fn load_config_scale() -> f32 {
    let path = get_config_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("scale=") {
                if let Ok(scale) = value.trim().parse::<f32>() {
                    return scale.clamp(0.5, 3.0);
                }
            }
        }
    }
    1.0
}

fn save_config_scale(scale: f32) {
    let path = get_config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let content = format!("scale={:.2}\n", scale.clamp(0.5, 3.0));
    std::fs::write(path, content).ok();
}

fn get_pinned_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("C:\\rex")); 

    #[cfg(not(target_os = "windows"))]
    let base = dirs::home_dir()
        .map(|h| h.join(".rex"))
        .unwrap_or_else(|| PathBuf::from(".rex"));

    base.join("pinned.ini")
}

fn get_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("C:\\rex"));

    #[cfg(not(target_os = "windows"))]
    let base = dirs::home_dir().map(|h| h.join(".rex")).unwrap_or_else(|| PathBuf::from(".rex"));

    base.join("config.ini")
}

impl Default for AppData {
    fn default() -> Self {
        let current_path = std::env::current_dir().unwrap_or_default();
        Self {
            path_edit: current_path.display().to_string(),
            current_path,
            search_query: String::new(),
            pinned: load_pinned_folders(),
            undo_stack: vec![],
            redo_stack: vec![],
            autocomplete: vec![],
            scale_factor: load_config_scale(),
            browser: browser::FileBrowser::new(),
            search: ViewMode::Browsing,
        }
    }
}

impl Drop for AppData {
    fn drop(&mut self) {
        save_pinned_folders(&self.pinned);
        save_config_scale(self.scale_factor);
    }
}

impl AppData {
    fn navigate_to(&mut self, new_path: PathBuf) {
        if new_path.exists() && new_path.is_dir() {
            self.undo_stack.push((self.current_path.clone(), std::mem::replace(&mut self.search, ViewMode::Browsing)));
            self.redo_stack.clear();
            self.current_path = new_path;
            self.path_edit = self.current_path.display().to_string();
        } else {
            self.path_edit = self.current_path.display().to_string();
        }
    }


    fn undo(&mut self) {
        if let Some((prev_path, prev_search)) = self.undo_stack.pop() {
            self.redo_stack.push((self.current_path.clone(), std::mem::replace(&mut self.search, prev_search)));
            self.current_path = prev_path;
            self.path_edit = self.current_path.display().to_string();
        }
    }

    fn redo(&mut self) {
        if let Some((next_path, next_search)) = self.redo_stack.pop() {
            self.undo_stack.push((self.current_path.clone(), std::mem::replace(&mut self.search, next_search)));
            self.current_path = next_path;
            self.path_edit = self.current_path.display().to_string();
        }
    }


    fn update_autocomplete(&mut self) {
        let input = self.path_edit.clone();
        let parent = PathBuf::from(&input).parent().map(PathBuf::from)
            .unwrap_or_else(|| self.current_path.clone());

        if !parent.exists() || !parent.is_dir() {
            self.autocomplete.clear();
            return;
        }

        let prefix = PathBuf::from(&input)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let mut matches = vec![];

        if let Ok(read) = std::fs::read_dir(parent) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if name.to_lowercase().starts_with(&prefix) {
                        matches.push(path.display().to_string());
                    }
                }
            }
        }

        matches.sort();
        matches.truncate(5);
        self.autocomplete = matches;
    }
}

impl eframe::App for AppData {
    fn update(&mut self, ctx: &Context, _: &mut Frame) {
        ctx.set_pixels_per_point(self.scale_factor);

        self.scale_factor = ctx.input(|i| {
            let mut scale = self.scale_factor;

            if i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Equals) {
                    scale = (scale + 0.1).clamp(0.5, 3.0);
                }
                if i.key_pressed(egui::Key::Minus) {
                    scale = (scale - 0.1).clamp(0.5, 3.0);
                }
                if i.raw_scroll_delta.y.abs() > f32::EPSILON {
                    scale = (scale + i.raw_scroll_delta.y * 0.01).clamp(0.5, 3.0);
                }
                if i.key_pressed(egui::Key::Num0) {
                    scale = 1.0;
                }
            }

            scale
        });



        TopBottomPanel::top("titlebar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⬆").clicked() {
                    if let Some(parent) = self.current_path.parent() {
                        self.navigate_to(parent.to_path_buf());
                    }
                }

                if ui.add_enabled(!self.undo_stack.is_empty(), Button::new("⮌")).clicked() {
                    self.undo();
                }

                if ui.add_enabled(!self.redo_stack.is_empty(), Button::new("⮎")).clicked() {
                    self.redo();
                }

                let response = ui.text_edit_singleline(&mut self.path_edit);
                if response.changed() {
                    self.update_autocomplete();
                }

                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if response.lost_focus() || enter_pressed {
                    self.autocomplete.clear();
                }

                if enter_pressed {
                    self.navigate_to(PathBuf::from(self.path_edit.clone()));
                }


                if !self.autocomplete.is_empty() {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        for suggestion in self.autocomplete.clone() {
                            if ui.button(&suggestion).clicked() {
                                self.path_edit = suggestion.clone();
                                self.autocomplete.clear();
                                self.navigate_to(PathBuf::from(suggestion));
                            }
                        }
                    });
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("🔍").clicked() {
                            let query = self.search_query.clone();
                            let start_path = self.current_path.clone();
                            let (tx, rx) = mpsc::channel();

                            thread::spawn(move || {
                                fn search_dir(dir: PathBuf, query: &str, tx: &Sender<PathBuf>) {
                                    if let Ok(read) = std::fs::read_dir(dir) {
                                        for entry in read.flatten() {
                                            let path = entry.path();
                                            if path.is_file() {
                                                if let Some(name) = path.file_name()
                                                    .and_then(|s| s.to_str()) {
                                                    if name.contains(query) {
                                                        tx.send(path.clone()).ok();
                                                    }
                                                }
                                            }
                                            if path.is_dir() {
                                                search_dir(path, query, tx);
                                            }
                                        }
                                    }
                                }

                                search_dir(start_path, &query, &tx);
                            });

                            self.search = ViewMode::Searching {
                                results: vec![],
                                receiver: rx,
                            };
                    }
                    ui.text_edit_singleline(&mut self.search_query);
                });
            });
        });
        
        let pinned = self.pinned.clone();

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                ui.heading("📌 Pinned");

                let mut to_unpin = None;

                for pin in pinned.iter() {
                    let name = pin
                        .file_name()
                        .or_else(|| pin.components().last().map(|c| c.as_os_str()))
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    let response = ui.button(&name);

                    if response.clicked() {
                        self.navigate_to(pin.clone());
                    }

                    response.context_menu(|ui| {
                        if ui.button("❌ Unpin").clicked() {
                            to_unpin = Some(pin.clone());
                            ui.close_menu();
                        }
                    });
                }

                if let Some(unpin_path) = to_unpin {
                    self.pinned.retain(|p| p != &unpin_path);
                }


            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut navigate_to = None;

            match &mut self.search {
                ViewMode::Browsing => {
                    if let Some((navigate, pin)) = self.browser.update(ctx, ui, &self.current_path) {
                        if let Some(p) = pin {
                            if !self.pinned.contains(&p) {
                                self.pinned.push(p);
                                self.pinned.sort();
                                self.pinned.dedup();
                            }
                        }
                        if let Some(new_path) = navigate {
                            self.navigate_to(new_path);
                            self.browser.invalidate();
                        }
                    }
                }
                ViewMode::Searching { results, receiver } => {
                    while let Ok(path) = receiver.try_recv() {
                        results.push(path);
                    }

                    ui.label(format!("Found {} results...", results.len()));
                    ui.separator();

                    for path in results.iter() {
                        if ui.button(path.display().to_string()).clicked() {
                            navigate_to = Some(path.clone());
                            break;
                        }
                    }

                    if ui.button("❌ Cancel Search").clicked() {
                        self.search = ViewMode::Browsing;
                    }
                }
            }
            if let Some(path) = navigate_to {
                self.navigate_to(path);
                self.search = ViewMode::Browsing;
            }
        });

    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "REX File Explorer",
        options,
        Box::new(|_cc| Ok(Box::new(AppData::default()))),
    )
    .unwrap();
}
