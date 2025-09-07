use std::path::PathBuf;

pub fn os_root() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from("C:\\")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/")
    }
}

pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("C:\\rex"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .map(|h| h.join(".rex"))
            .unwrap_or_else(|| PathBuf::from(".rex"))
    }
}

pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("C:\\rex"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .map(|h| h.join(".rex"))
            .unwrap_or_else(|| PathBuf::from(".rex"))
    }
}

pub fn pinned_path() -> PathBuf {
    data_dir().join("pinned.ini")
}
pub fn prefs_path() -> PathBuf {
    config_dir().join("config.ini")
}
pub fn trash_dir() -> PathBuf {
    data_dir().join("trash")
}

pub fn load_pinned() -> Vec<PathBuf> {
    let path = pinned_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if !path.exists() {
        return vec![dirs::home_dir().unwrap_or_default(), os_root()];
    }
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let v: Vec<_> = s
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect();
            if v.is_empty() {
                vec![dirs::home_dir().unwrap_or_default(), os_root()]
            } else {
                v
            }
        }
        Err(_) => vec![dirs::home_dir().unwrap_or_default(), os_root()],
    }
}

pub fn save_pinned(p: &[PathBuf]) {
    let path = pinned_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = p
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(path, content);
}

pub fn load_scale() -> f32 {
    let path = prefs_path();
    if let Ok(s) = std::fs::read_to_string(&path) {
        for line in s.lines() {
            if let Some(v) = line.strip_prefix("scale=") {
                if let Ok(f) = v.trim().parse::<f32>() {
                    return f.clamp(0.5, 3.0);
                }
            }
        }
    }
    1.0
}

pub fn save_scale(scale: f32) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format!("scale={:.2}\n", scale.clamp(0.5, 3.0)));
}
