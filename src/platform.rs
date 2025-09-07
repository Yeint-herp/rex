use std::path::Path;

pub fn open_file(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
}

pub fn open_with(path: &Path, cmdline: &str) {
    // TODO quote-aware parsing
    let mut parts = cmdline.split_whitespace();
    if let Some(cmd) = parts.next() {
        let args: Vec<String> = parts.map(|s| s.to_string()).collect();
        let _ = std::process::Command::new(cmd).args(args).arg(path).spawn();
    }
}

pub fn open_terminal_in(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let candidates: &[(&str, &[&str])] = &[("wt", &["-d"])];
        for (bin, pre) in candidates {
            let mut cmd = std::process::Command::new(bin);
            for p in *pre {
                cmd.arg(p);
            }
            cmd.arg(path);
            if cmd.spawn().is_ok() {
                return;
            }
        }
        let _ = std::process::Command::new("powershell")
            .args(&["-NoExit", "-Command", &format!("cd '{}'", path.display())])
            .spawn()
            .or_else(|_| {
                std::process::Command::new("cmd")
                    .args(&["/K", &format!("cd /d {}", path.display())])
                    .spawn()
            });
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application "Terminal" to do script "cd '{}'; clear""#,
            path.display()
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let terms = [
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "xterm",
            "x-terminal-emulator",
            "foot",
            "kitty",
            "alacritty",
        ];
        for t in terms {
            if std::process::Command::new(t)
                .current_dir(path)
                .spawn()
                .is_ok()
            {
                return;
            }
        }
    }
}
