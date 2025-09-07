use super::config;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub enum Op {
    Rename { from: PathBuf, to: PathBuf },
    Move { from: PathBuf, to: PathBuf },
    Copy { to: PathBuf },
    Delete { trashed: PathBuf, original: PathBuf },
    MkDir { path: PathBuf },
    Touch { path: PathBuf },
}

fn copy_rec(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for e in fs::read_dir(from)? {
            let e = e?;
            let src = e.path();
            let dst = to.join(e.file_name());
            copy_rec(&src, &dst)?;
        }
    } else {
        if let Some(p) = to.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(from, to)?;
    }
    Ok(())
}

fn move_rec(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(p) = to.parent() {
        fs::create_dir_all(p)?;
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_rec(from, to)?;
            remove_rec(from)
        }
    }
}

fn remove_rec(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
}

fn unique_in(dir: &Path, name: &str) -> PathBuf {
    let mut cand = dir.join(name);
    if !cand.exists() {
        return cand;
    }
    let mut idx = 1usize;
    loop {
        let with = format!("{} ({idx})", name);
        cand = dir.join(with);
        if !cand.exists() {
            return cand;
        }
        idx += 1;
    }
}

pub fn copy(from: &Path, to_dir: &Path) -> std::io::Result<Op> {
    let name = from.file_name().unwrap_or_default().to_string_lossy();
    let dst = unique_in(to_dir, &name);
    copy_rec(from, &dst)?;
    Ok(Op::Copy {
        to: dst,
    })
}

pub fn mv(from: &Path, to_dir: &Path) -> std::io::Result<Op> {
    let name = from.file_name().unwrap_or_default().to_string_lossy();
    let dst = unique_in(to_dir, &name);
    move_rec(from, &dst)?;
    Ok(Op::Move {
        from: from.to_path_buf(),
        to: dst,
    })
}

pub fn rename(from: &Path, new_name: &str) -> std::io::Result<Op> {
    let to = from.with_file_name(new_name);
    move_rec(from, &to)?;
    Ok(Op::Rename {
        from: from.to_path_buf(),
        to,
    })
}

pub fn mkdir(where_: &Path, name: &str) -> std::io::Result<Op> {
    let dst = where_.join(name);
    std::fs::create_dir_all(&dst)?;
    Ok(Op::MkDir { path: dst })
}

pub fn touch(where_: &Path, name: &str) -> std::io::Result<Op> {
    let dst = where_.join(name);
    if let Some(p) = dst.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&dst)?;
    Ok(Op::Touch { path: dst })
}

pub fn delete_to_trash(p: &Path) -> std::io::Result<Op> {
    let trash = config::trash_dir();
    std::fs::create_dir_all(&trash)?;
    let name = p.file_name().unwrap_or_default().to_string_lossy();
    let dst = super::fs_ops::unique_in(&trash, &name);
    super::fs_ops::move_rec(p, &dst)?;
    Ok(Op::Delete {
        trashed: dst,
        original: p.to_path_buf(),
    })
}

pub fn undo(op: &Op) -> std::io::Result<()> {
    match op {
        Op::Copy { to, .. } => super::fs_ops::remove_rec(to),
        Op::Move { from, to } | Op::Rename { from, to } => super::fs_ops::move_rec(to, from),
        Op::Delete { trashed, original } => super::fs_ops::move_rec(trashed, original),
        Op::MkDir { path } => super::fs_ops::remove_rec(path),
        Op::Touch { path } => super::fs_ops::remove_rec(path),
    }
}
