use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
};

#[derive(Clone)]
pub struct SearchMsg {
    pub path: PathBuf,
}

#[derive(Clone)]
pub struct ProgressMsg {
    pub scanned_files: u64,
    pub scanned_dirs: u64,
    pub done: bool,
}

pub fn spawn_search(
    root: PathBuf,
    query: String,
    tx_results: Sender<SearchMsg>,
    tx_prog: Sender<ProgressMsg>,
    abort: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        fn walk(
            dir: &Path,
            query: &str,
            tx_results: &Sender<SearchMsg>,
            tx_prog: &Sender<ProgressMsg>,
            abort: &AtomicBool,
            counters: &mut (u64, u64),
        ) {
            if abort.load(Ordering::Relaxed) {
                return;
            }
            let read = match std::fs::read_dir(dir) {
                Ok(r) => r,
                Err(_) => return,
            };
            counters.1 += 1;
            let _ = tx_prog.send(ProgressMsg {
                scanned_files: counters.0,
                scanned_dirs: counters.1,
                done: false,
            });
            for entry in read.flatten() {
                if abort.load(Ordering::Relaxed) {
                    return;
                }
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, query, tx_results, tx_prog, abort, counters);
                } else {
                    counters.0 += 1;
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if name.to_lowercase().contains(&query.to_lowercase()) {
                            let _ = tx_results.send(SearchMsg { path: path.clone() });
                        }
                    }
                    let _ = tx_prog.send(ProgressMsg {
                        scanned_files: counters.0,
                        scanned_dirs: counters.1,
                        done: false,
                    });
                }
            }
        }
        let mut counters = (0u64, 0u64);
        walk(&root, &query, &tx_results, &tx_prog, &abort, &mut counters);
        let _ = tx_prog.send(ProgressMsg {
            scanned_files: counters.0,
            scanned_dirs: counters.1,
            done: true,
        });
    });
}
