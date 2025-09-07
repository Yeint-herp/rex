use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Copy,
    Cut,
}

#[derive(Default, Clone)]
pub struct Clipboard {
    pub items: Vec<PathBuf>,
    pub mode: Option<Mode>,
    pub last_paste_targets: Vec<(PathBuf, PathBuf)>,
}

impl Clipboard {
    pub fn clear(&mut self) {
        self.items.clear();
        self.mode = None;
        self.last_paste_targets.clear();
    }
    pub fn set(&mut self, items: Vec<PathBuf>, mode: Mode) {
        self.items = items;
        self.mode = Some(mode);
        self.last_paste_targets.clear();
    }
    pub fn has_items(&self) -> bool {
        !self.items.is_empty() && self.mode.is_some()
    }
}
