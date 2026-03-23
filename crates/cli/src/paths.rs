use std::path::PathBuf;

/// Returns `~/.braille/`, creating it with mode 0700 if it doesn't exist.
pub fn runtime_dir() -> PathBuf {
    let dir = home_dir().join(".braille");
    if !dir.exists() {
        #[cfg(unix)]
        {
            use std::fs::DirBuilder;
            use std::os::unix::fs::DirBuilderExt;
            DirBuilder::new().mode(0o700).recursive(true).create(&dir).ok();
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&dir).ok();
        }
    }
    dir
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("daemon.sock")
}

pub fn pid_path() -> PathBuf {
    runtime_dir().join("daemon.pid")
}

pub fn log_path() -> PathBuf {
    runtime_dir().join("daemon.log")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

