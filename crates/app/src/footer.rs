use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub cwd: Option<PathBuf>,
    pub branch: Option<String>,
}

pub struct Footer {
    shell: String,
    info: Arc<Mutex<Info>>,
}

impl Footer {
    pub fn start(login_pid: u32) -> Self {
        let info = Arc::new(Mutex::new(Info::default()));
        let shared = info.clone();
        thread::spawn(move || {
            loop {
                let cwd = read_cwd(shell_process(login_pid));
                let branch = cwd.as_deref().and_then(git_branch);
                if let Ok(mut guard) = shared.lock() {
                    *guard = Info { cwd, branch };
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
        Self {
            shell: shell_name(),
            info,
        }
    }

    pub fn line(&self, cols: usize, terminal: (u16, u16)) -> String {
        let info = self
            .info
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let mut left = vec!["UTF-8".to_string(), self.shell.clone()];
        if let Some(cwd) = &info.cwd {
            left.push(display_path(cwd));
        }
        if let Some(branch) = &info.branch {
            left.push(format!("git:{branch}"));
        }
        let right = format!("{}   {}×{}", local_time(), terminal.0, terminal.1);
        compose(cols, &left.join("   "), &right)
    }
}

fn shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|path| {
            Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "sh".into())
}

fn shell_process(login_pid: u32) -> u32 {
    Command::new("pgrep")
        .args(["-P", &login_pid.to_string()])
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .find_map(|line| line.trim().parse().ok())
        })
        .unwrap_or(login_pid)
}

fn read_cwd(pid: u32) -> Option<PathBuf> {
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix('n'))
        .map(PathBuf::from)
}

fn git_branch(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

fn display_path(path: &Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

fn local_time() -> String {
    // SAFETY: localtime_r writes into `tm` and reads the valid `time_t` we pass in.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    }
}

fn compose(width: usize, left: &str, right: &str) -> String {
    let right_len = right.chars().count();
    if width <= right_len + 2 {
        return right.chars().take(width).collect();
    }
    let left_room = width - right_len - 2;
    let left: String = left.chars().take(left_room).collect();
    let gap = width - right_len - left.chars().count();
    format!("{left}{}{right}", " ".repeat(gap))
}
