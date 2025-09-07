use super::fs_ops::Op;
use std::{collections::VecDeque, path::PathBuf};

#[derive(Default)]
pub struct NavHistory {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
}

impl NavHistory {
    pub fn push(&mut self, cur: PathBuf) {
        self.back.push(cur);
        self.forward.clear();
    }
    pub fn back(&mut self, cur: &mut PathBuf) -> bool {
        if let Some(prev) = self.back.pop() {
            self.forward.push(cur.clone());
            *cur = prev;
            return true;
        }
        false
    }
    pub fn forward(&mut self, cur: &mut PathBuf) -> bool {
        if let Some(next) = self.forward.pop() {
            self.back.push(cur.clone());
            *cur = next;
            return true;
        }
        false
    }
    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[derive(Default)]
pub struct OpsHistory {
    pub undo: VecDeque<Op>,
    pub capacity: usize,
}

impl OpsHistory {
    pub fn new(cap: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            capacity: cap,
        }
    }
    pub fn push(&mut self, op: Op) {
        if self.undo.len() == self.capacity {
            self.undo.pop_front();
        }
        self.undo.push_back(op);
    }
    pub fn pop_undo(&mut self) -> Option<Op> {
        self.undo.pop_back()
    }
}
