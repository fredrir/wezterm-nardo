use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// `WEZPLUG_LOG` file logger; never stderr (the tty is the UI). Refuses symlinks, mode 0600.
#[derive(Clone, Default)]
pub struct Logger {
    file: Option<Arc<Mutex<File>>>,
}

fn open_private(path: &std::ffi::OsStr) -> Option<File> {
    if std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return None;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).ok()
}

impl Logger {
    pub fn from_env() -> Self {
        let file = std::env::var_os("WEZPLUG_LOG").and_then(|p| open_private(&p)).map(|f| Arc::new(Mutex::new(f)));
        Self { file }
    }

    pub fn log(&self, msg: impl AsRef<str>) {
        let Some(file) = &self.file else { return };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        if let Ok(mut f) = file.lock() {
            let _ = writeln!(f, "{}.{:03} {}", now.as_secs(), now.subsec_millis(), msg.as_ref());
        }
    }
}
