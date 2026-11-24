use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs::{self, File, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use tempfile::{Builder, NamedTempFile};

const DEFAULT_TEXT: &str = "双击这里开始记录\n右键调整样式";
const PROFILE_POINTER: &str = ".floatnote-active-profile";
const PROFILES_DIR: &str = ".floatnote-profiles";
const PROFILE_PREFIX: &str = "profile-";
pub const DEFAULT_SHORTCUT: &str = if cfg!(target_os = "macos") {
    "Control+Alt+KeyN"
} else {
    "Super+Shift+KeyQ"
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenPosition {
    pub name: Option<String>,
    pub origin_x: i32,
    pub origin_y: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub x: Option<i32>,
    pub y: Option<i32>,
    #[serde(default)]
    pub position: Option<ScreenPosition>,
    pub width: i32,
    pub font_size: i32,
    pub color: String,
    pub opacity: f64,
    pub outline: bool,
    pub shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            position: None,
            width: 360,
            font_size: 18,
            color: "#ffffff".into(),
            opacity: 1.0,
            outline: true,
            shortcut: DEFAULT_SHORTCUT.into(),
        }
    }
}

impl Config {
    fn from_json(raw: &Value) -> (Self, Option<&'static str>) {
        let mut cfg = Self::default();
        let coordinate = |key: &str| raw[key].as_i64().and_then(|n| i32::try_from(n).ok());
        if let (Some(x), Some(y)) = (coordinate("x"), coordinate("y")) {
            cfg.x = Some(x);
            cfg.y = Some(y);
        }
        if let Some(position) = raw.get("position") {
            cfg.position = serde_json::from_value(position.clone()).ok();
        }
        if let Some(n) = raw["width"].as_i64() {
            cfg.width = n.clamp(240, 1200) as i32;
        }
        if let Some(n) = raw["font_size"].as_i64() {
            cfg.font_size = n.clamp(10, 64) as i32;
        }
        if let Some(color) = raw["color"].as_str() {
            if color.len() == 7
                && color.starts_with('#')
                && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
            {
                cfg.color = color.to_ascii_lowercase();
            }
        }
        if let Some(n) = raw["opacity"].as_f64() {
            cfg.opacity = (100.0 * n.clamp(0.2, 1.0)).round() / 100.0;
        }
        if let Some(outline) = raw["outline"].as_bool() {
            cfg.outline = outline;
        }
        let mut warning = None;
        if let Some(shortcut) = raw["shortcut"].as_str().filter(|s| !s.trim().is_empty()) {
            cfg.shortcut = shortcut.into();
        } else if raw.get("hotkey_mods").is_some() || raw.get("hotkey_vk").is_some() {
            match raw["hotkey_mods"].as_u64().zip(raw["hotkey_vk"].as_u64()) {
                // The old Windows default would become the macOS logout shortcut.
                Some((mods, 0x51)) if mods & 15 == 12 => {}
                Some((mods, vk)) if let Some(shortcut) = windows_shortcut(mods, vk) => {
                    cfg.shortcut = shortcut;
                }
                _ => {
                    warning =
                        Some("旧版快捷键无法迁移，已使用默认快捷键。可在快捷键设置中重新绑定。")
                }
            }
        }
        (cfg, warning)
    }
}

// Translate the Win32 RegisterHotKey values recorded by the Python application
// into physical key codes accepted by the cross-platform shortcut plugin.
fn windows_shortcut(mods: u64, vk: u64) -> Option<String> {
    if mods & 15 == 0 {
        return None;
    }
    let key = match vk {
        0x30..=0x39 => format!("Digit{}", char::from(vk as u8)),
        0x41..=0x5a => format!("Key{}", char::from(vk as u8)),
        0x60..=0x69 => format!("Numpad{}", vk - 0x60),
        0x70..=0x87 => format!("F{}", vk - 0x6f),
        _ => match vk {
            0x08 => "Backspace",
            0x09 => "Tab",
            0x0d => "Enter",
            0x13 => "Pause",
            0x14 => "CapsLock",
            0x1b => "Escape",
            0x20 => "Space",
            0x21 => "PageUp",
            0x22 => "PageDown",
            0x23 => "End",
            0x24 => "Home",
            0x25 => "ArrowLeft",
            0x26 => "ArrowUp",
            0x27 => "ArrowRight",
            0x28 => "ArrowDown",
            0x2c => "PrintScreen",
            0x2d => "Insert",
            0x2e => "Delete",
            0x5d => "ContextMenu",
            0x6a => "NumpadMultiply",
            0x6b => "NumpadAdd",
            0x6d => "NumpadSubtract",
            0x6e => "NumpadDecimal",
            0x6f => "NumpadDivide",
            0x90 => "NumLock",
            0x91 => "ScrollLock",
            0xad => "AudioVolumeMute",
            0xae => "AudioVolumeDown",
            0xaf => "AudioVolumeUp",
            0xb0 => "MediaTrackNext",
            0xb1 => "MediaTrackPrevious",
            0xb2 => "MediaStop",
            0xb3 => "MediaPlayPause",
            0xba => "Semicolon",
            0xbb => "Equal",
            0xbc => "Comma",
            0xbd => "Minus",
            0xbe => "Period",
            0xbf => "Slash",
            0xc0 => "Backquote",
            0xdb => "BracketLeft",
            0xdc => "Backslash",
            0xdd => "BracketRight",
            0xde => "Quote",
            _ => return None,
        }
        .into(),
    };
    let mut parts = Vec::new();
    for (mask, name) in [(8, "Super"), (2, "Control"), (1, "Alt"), (4, "Shift")] {
        if mods & mask != 0 {
            parts.push(name);
        }
    }
    parts.push(&key);
    Some(parts.join("+"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    source_dir: String,
    has_notes: bool,
    has_config: bool,
    has_history: bool,
    has_existing_data: bool,
}

pub struct Store {
    pub dir: PathBuf,
    pub startup_warning: Option<String>,
    root: PathBuf,
    history_lock: Mutex<()>,
}

impl Store {
    pub fn discover(app_data_dir: PathBuf) -> Result<Self, String> {
        let executable = env::current_exe().map_err(|e| e.to_string())?;
        let portable = if cfg!(all(windows, not(debug_assertions))) {
            executable.parent()
        } else {
            None
        };
        Self::open(
            app_data_dir,
            env::var_os("FLOATNOTE_DATA_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            portable,
        )
    }

    fn open(
        app_data: PathBuf,
        explicit: Option<PathBuf>,
        portable: Option<&Path>,
    ) -> Result<Self, String> {
        let portable = portable.filter(|_| explicit.is_none());
        let root = explicit.unwrap_or(app_data);
        fs::create_dir_all(&root).map_err(|e| path_error(&root, e))?;
        let pointer = root.join(PROFILE_POINTER);
        let mut warning = None;
        let mut dir = root.clone();
        if pointer.exists() {
            let profile = fs::read_to_string(&pointer).map_err(|e| path_error(&pointer, e))?;
            if profile.starts_with(PROFILE_PREFIX)
                && profile
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                let candidate = root.join(PROFILES_DIR).join(profile);
                if candidate.is_dir() {
                    dir = candidate;
                } else {
                    warning = Some("导入副本已移动，正在使用默认数据目录。".into());
                }
            } else {
                warning = Some("数据目录记录无效，正在使用默认数据目录。".into());
            }
        } else if let Some(portable) = portable.filter(|path| {
            ["notes.txt", "config.json", "notes-history/.git"]
                .iter()
                .any(|name| path.join(name).exists())
        }) {
            let readonly = ["notes.txt", "config.json"].iter().any(|name| {
                fs::metadata(portable.join(name)).is_ok_and(|m| m.permissions().readonly())
            });
            if readonly || NamedTempFile::new_in(portable).is_err() {
                warning = Some(format!(
                    "旧版数据目录不可写。可在“数据与导入”复制旧数据：{}",
                    portable.display()
                ));
            } else {
                dir = portable.to_owned();
            }
        }
        if let Ok(bytes) = fs::read(dir.join("config.json")) {
            if let Ok(raw) = serde_json::from_slice::<Value>(
                bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes),
            ) {
                if raw["shortcut"].is_null()
                    && !raw["hotkey_vk"].is_null()
                    && raw["hotkey_mods"]
                        .as_u64()
                        .zip(raw["hotkey_vk"].as_u64())
                        .and_then(|(mods, vk)| windows_shortcut(mods, vk))
                        .is_none()
                {
                    warning = Some("旧版快捷键无法识别，请在快捷键设置重新绑定。".into());
                }
            }
        }
        Ok(Self {
            dir,
            startup_warning: warning,
            root,
            history_lock: Mutex::new(()),
        })
    }

    pub fn inspect_import(&self, source: &Path) -> Result<ImportPreview, String> {
        let source = source.canonicalize().map_err(|e| path_error(source, e))?;
        let active = self
            .dir
            .canonicalize()
            .map_err(|e| path_error(&self.dir, e))?;
        if !source.is_dir() || source == active {
            return Err("请选择另一处旧版数据目录。".into());
        }
        Ok(ImportPreview {
            source_dir: source.display().to_string(),
            has_notes: checked_entry(&source.join("notes.txt"), false)?,
            has_config: checked_entry(&source.join("config.json"), false)?,
            has_history: checked_entry(&source.join("notes-history"), true)?
                && checked_entry(&source.join("notes-history/.git"), true)?,
            has_existing_data: self.root.join(PROFILE_POINTER).exists()
                || self.dir.join("notes-history/.git").exists()
                || self.dir.join("config.json").exists()
                || (self.dir.join("notes.txt").exists() && self.load_notes()? != DEFAULT_TEXT),
        })
    }

    pub fn import_from(&self, source: &Path, allow_conflict: bool) -> Result<String, String> {
        let preview = self.inspect_import(source)?;
        if preview.has_existing_data && !allow_conflict {
            return Err("已有便签数据，请重新预览并确认保留原数据后导入。".into());
        }
        if !preview.has_notes && !preview.has_config && !preview.has_history {
            return Err("所选目录中没有旧版便签数据。".into());
        }
        let source = Path::new(&preview.source_dir);
        let profiles = self.root.join(PROFILES_DIR);
        fs::create_dir_all(&profiles).map_err(|e| path_error(&profiles, e))?;
        let staged = Builder::new()
            .prefix(PROFILE_PREFIX)
            .tempdir_in(&profiles)
            .map_err(|e| path_error(&profiles, e))?;

        if preview.has_notes {
            copy_entry(&source.join("notes.txt"), &staged.path().join("notes.txt"))?;
        }
        if preview.has_config {
            let path = source.join("config.json");
            let bytes = fs::read(&path).map_err(|e| path_error(&path, e))?;
            let raw = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
            let name = match serde_json::from_slice::<Value>(raw) {
                Ok(Value::Object(_)) => "config.json",
                _ => "config.json.corrupt-import",
            };
            atomic_write(&staged.path().join(name), &bytes)?;
        }
        if preview.has_history {
            let history = source.join("notes-history");
            let copy = staged.path().join("notes-history");
            let git = history.join(".git");
            let git_copy = copy.join(".git");
            fs::create_dir_all(&git_copy).map_err(|e| path_error(&git_copy, e))?;
            atomic_write(
                &git_copy.join("config"),
                b"[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
            )?;
            for name in ["HEAD", "objects"] {
                copy_entry(&git.join(name), &git_copy.join(name))?;
            }
            for (name, directory) in [("refs", true), ("packed-refs", false)] {
                if checked_entry(&git.join(name), directory)? {
                    copy_entry(&git.join(name), &git_copy.join(name))?;
                }
            }
            if checked_entry(&history.join("notes.txt"), false)? {
                copy_entry(&history.join("notes.txt"), &copy.join("notes.txt"))?;
                if !preview.has_notes {
                    copy_entry(&history.join("notes.txt"), &staged.path().join("notes.txt"))?;
                }
            }
        }
        let directory = staged.path().to_owned();
        let name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("导入目录名无效")?;
        atomic_write(&self.root.join(PROFILE_POINTER), name.as_bytes())?;
        Ok(staged.keep().display().to_string())
    }

    pub fn initialize_history(&self, text: &str) -> Result<(), String> {
        if text != DEFAULT_TEXT
            && self.dir.join("notes.txt").is_file()
            && !self.dir.join("notes-history/.git").exists()
        {
            self.start_history(text)?;
        }
        Ok(())
    }

    pub fn load_config(&self) -> Result<Config, String> {
        let path = self.dir.join("config.json");
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(path_error(&path, e)),
        };
        match serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes)) {
            Ok(raw) => Ok(Config::from_json(&raw).0),
            Err(_) => {
                let backup = self.dir.join(format!(
                    "config.json.corrupt-{}",
                    Local::now().format("%Y%m%d-%H%M%S-%f")
                ));
                fs::rename(&path, &backup).map_err(|e| path_error(&path, e))?;
                Ok(Config::default())
            }
        }
    }

    pub fn save_config(&self, cfg: &Config) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
        atomic_write(&self.dir.join("config.json"), &json)
    }

    pub fn load_notes(&self) -> Result<String, String> {
        let path = self.dir.join("notes.txt");
        match fs::read(&path) {
            Ok(bytes) => Ok(String::from_utf8_lossy(
                bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes),
            )
            .into_owned()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(DEFAULT_TEXT.into()),
            Err(e) => Err(path_error(&path, e)),
        }
    }

    pub fn save_notes(&self, text: &str) -> Result<(), String> {
        atomic_write(&self.dir.join("notes.txt"), text.as_bytes())
    }

    pub fn start_history(&self, text: &str) -> Result<(), String> {
        self.checkpoint_at(text, Local::now().timestamp())
    }

    fn checkpoint_at(&self, text: &str, epoch: i64) -> Result<(), String> {
        let _guard = self.history_lock.lock().map_err(|e| e.to_string())?;
        let dir = self.dir.join("notes-history");
        fs::create_dir_all(&dir).map_err(|e| path_error(&dir, e))?;
        if !dir
            .join(".git")
            .try_exists()
            .map_err(|e| path_error(&dir, e))?
        {
            git_ok(&dir, &["init", "-q"])?;
        }
        atomic_write(&dir.join("notes.txt"), text.as_bytes())?;
        git_ok(&dir, &["add", "--", "notes.txt"])?;
        let diff = git(
            &dir,
            &["diff", "--cached", "--quiet", "--", "notes.txt"],
            None,
        )?;
        match diff.status.code() {
            Some(0) => return Ok(()),
            Some(1) => {}
            _ => return Err(git_error(&diff)),
        }
        let date = Local
            .timestamp_opt(epoch, 0)
            .single()
            .ok_or("无效的保存时间")?;
        let message = format!("notes {}", date.format("%Y-%m-%d %H:%M:%S"));
        let output = git(
            &dir,
            &[
                "-c",
                "user.name=FloatNote",
                "-c",
                "user.email=floatnote@local",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-q",
                "--only",
                "-m",
                &message,
                "--",
                "notes.txt",
            ],
            Some(&date.to_rfc3339()),
        )?;
        checked(output).map(|_| ())
    }

    fn history_dir(&self) -> Result<Option<PathBuf>, String> {
        let dir = self.dir.join("notes-history");
        if !dir
            .join(".git")
            .try_exists()
            .map_err(|e| path_error(&dir, e))?
        {
            return Ok(None);
        }
        let output = git(&dir, &["rev-parse", "--verify", "--quiet", "HEAD"], None)?;
        match output.status.code() {
            Some(0) => Ok(Some(dir)),
            Some(1) => Ok(None),
            _ => Err(git_error(&output)),
        }
    }

    pub fn history_bounds(&self) -> Result<Option<(i64, i64)>, String> {
        let Some(dir) = self.history_dir()? else {
            return Ok(None);
        };
        let output = git_ok(&dir, &["log", "--format=%ct", "--", "notes.txt"])?;
        let mut bounds: Option<(i64, i64)> = None;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let epoch = line.parse::<i64>().map_err(|e| e.to_string())?;
            bounds = Some(bounds.map_or((epoch, epoch), |(lo, hi)| (lo.min(epoch), hi.max(epoch))));
        }
        Ok(bounds)
    }

    pub fn history_increment(&self, start: i64, end: i64) -> Result<String, String> {
        let (start, end) = (start.min(end), start.max(end));
        let range = format!(
            "变更范围：{} ~ {}\n\n",
            time_label(start)?,
            time_label(end)?
        );
        let Some(dir) = self.history_dir()? else {
            return Ok(format!("{range}还没有保存历史。退出一次编辑后就会有记录。"));
        };
        let output = git_ok(
            &dir,
            &[
                "log",
                "--reverse",
                "--format=%x00%ct",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--unified=0",
                "-p",
                "--",
                "notes.txt",
            ],
        )?;
        let mut report = range;
        let mut shown = 0;
        let raw = String::from_utf8_lossy(&output.stdout);
        for block in raw.split('\0').skip(1) {
            let mut lines = block.lines();
            let epoch = lines
                .next()
                .unwrap_or_default()
                .parse::<i64>()
                .map_err(|e| e.to_string())?;
            if !(start..=end).contains(&epoch) {
                continue;
            }
            let mut in_hunk = false;
            let mut printed_time = false;
            for line in lines {
                if line.starts_with("@@ ") {
                    in_hunk = true;
                    continue;
                }
                if !in_hunk || !(line.starts_with('+') || line.starts_with('-')) {
                    continue;
                }
                if !printed_time {
                    report.push_str(&format!("[{}]\n", time_label(epoch)?));
                    printed_time = true;
                }
                report.push_str(&format!("  {} {}\n", &line[..1], &line[1..]));
                shown += 1;
                if shown == 4000 {
                    report.push_str(
                        "……内容过长已截断，可在 notes-history 里用 git log 查看完整历史。",
                    );
                    return Ok(report);
                }
            }
            if printed_time {
                report.push('\n');
            }
        }
        if shown == 0 {
            report.push_str("该时间段没有变更记录。");
        }
        Ok(report)
    }
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn checked_entry(path: &Path, directory: bool) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta)
            if !is_link(&meta)
                && (if directory {
                    meta.is_dir()
                } else {
                    meta.is_file()
                }) =>
        {
            Ok(true)
        }
        Ok(_) => Err(format!("导入文件类型无效：{}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(path_error(path, error)),
    }
}

fn copy_entry(source: &Path, target: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(source).map_err(|e| path_error(source, e))?;
    if is_link(&meta) {
        return Err(format!("无法导入链接：{}", source.display()));
    }
    if meta.is_dir() {
        fs::create_dir(target).map_err(|e| path_error(target, e))?;
        for entry in fs::read_dir(source).map_err(|e| path_error(source, e))? {
            let entry = entry.map_err(|e| path_error(source, e))?;
            // Git objects must be self-contained. Never copy alternate object paths.
            if !matches!(
                entry.file_name().to_str(),
                Some("alternates" | "http-alternates")
            ) {
                copy_entry(&entry.path(), &target.join(entry.file_name()))?;
            }
        }
    } else if meta.is_file() {
        let mut file = File::open(source).map_err(|e| path_error(source, e))?;
        let mut output = File::create(target).map_err(|e| path_error(target, e))?;
        io::copy(&mut file, &mut output)
            .and_then(|_| output.sync_all())
            .map_err(|e| path_error(target, e))?;
    } else {
        return Err(format!("导入文件类型无效：{}", source.display()));
    }
    Ok(())
}

fn time_label(epoch: i64) -> Result<String, String> {
    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
        .ok_or_else(|| "无效的历史时间".into())
}

fn path_error(path: &Path, err: io::Error) -> String {
    format!("{}：{err}", path.display())
}

fn prepare_file(dir: &Path, bytes: &[u8]) -> Result<NamedTempFile, String> {
    let mut tmp = NamedTempFile::new_in(dir).map_err(|e| path_error(dir, e))?;
    tmp.write_all(bytes)
        .and_then(|()| tmp.as_file().sync_all())
        .map_err(|e| path_error(dir, e))?;
    Ok(tmp)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or("保存路径缺少父目录")?;
    prepare_file(dir, bytes)?
        .persist(path)
        .map_err(|e| path_error(path, e.error))?;
    Ok(())
}

fn git(dir: &Path, args: &[&str], date: Option<&str>) -> Result<Output, String> {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args);
    // A launch from a Git hook or IDE must never direct note commits into
    // the launching project's repository or index.
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(var);
    }
    if let Some(date) = date {
        command
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command.output().map_err(|e| format!("无法运行 Git：{e}"))
}

fn git_error(output: &Output) -> String {
    format!(
        "Git {}：{}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn checked(output: Output) -> Result<Output, String> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(git_error(&output))
    }
}

fn git_ok(dir: &Path, args: &[&str]) -> Result<Output, String> {
    checked(git(dir, args, None)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, tempdir_in, TempDir};

    fn fixture() -> (TempDir, Store) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().to_owned(), None, None).unwrap();
        (dir, store)
    }

    #[test]
    fn legacy_config_and_bom_notes_survive_loading_and_roundtrip() {
        let (_tmp, store) = fixture();
        fs::write(
            store.dir.join("config.json"),
            br##"{
            "x": -900, "y": 16, "width": 300, "font_size": 24, "color": "#DCB123",
            "opacity": 0.705, "outline": false,
            "hotkey_mods": 16387, "hotkey_vk": 186, "hotkey_label": "Ctrl+Alt+;"
        }"##,
        )
        .unwrap();
        let cfg = store.load_config().unwrap();
        assert_eq!((cfg.x, cfg.y), (Some(-900), Some(16)));
        assert_eq!(cfg.shortcut, "Control+Alt+Semicolon");
        assert_eq!((cfg.width, cfg.font_size), (300, 24));
        assert_eq!(cfg.color, "#dcb123");
        assert!(!cfg.outline);
        assert_eq!(cfg.opacity, 0.71);
        store.save_config(&cfg).unwrap();
        assert_eq!(cfg, store.load_config().unwrap());
        fs::write(store.dir.join("notes.txt"), "\u{feff}旧笔记\r\n第二行").unwrap();
        assert_eq!(store.load_notes().unwrap(), "旧笔记\r\n第二行");
        store.save_notes("").unwrap();
        assert_eq!(store.load_notes().unwrap(), "");
        assert_eq!(fs::read_dir(&store.dir).unwrap().count(), 2);
    }

    #[test]
    fn corrupted_config_is_preserved_and_individual_fields_are_validated() {
        let (_tmp, store) = fixture();
        assert_eq!(store.load_notes().unwrap(), DEFAULT_TEXT);
        let config = store.dir.join("config.json");
        fs::write(&config, "corrupt json").unwrap();
        assert_eq!(store.load_config().unwrap(), Config::default());
        assert!(!config.exists());
        assert!(fs::read_dir(&store.dir)
            .unwrap()
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("config.json.corrupt-")
                && fs::read_to_string(entry.path()).unwrap() == "corrupt json"));
        fs::write(
            &config,
            br##"{
            "x": true, "y": 4, "color": "#00zzff", "width": 9999, "font_size": -1,
            "opacity": 2, "outline": "true", "hotkey_mods": 0, "hotkey_vk": 81
        }"##,
        )
        .unwrap();
        let cfg = store.load_config().unwrap();
        assert_eq!(
            cfg,
            Config {
                width: 1200,
                font_size: 10,
                ..Config::default()
            }
        );
        let old_default = serde_json::json!({"hotkey_mods": 12, "hotkey_vk": 81});
        assert_eq!(Config::from_json(&old_default).0.shortcut, DEFAULT_SHORTCUT);
    }

    #[test]
    fn git_checkpoints_can_reopen_legacy_history_and_filter_inclusive_bounds() {
        let (_tmp, store) = fixture();
        let first = 1_700_000_000;
        let last = first + 120;
        assert_eq!(store.history_bounds().unwrap(), None);
        assert!(store
            .history_increment(first, last)
            .unwrap()
            .contains("还没有保存历史"));
        // Build the fixture with ordinary Git commands, as the Python version did.
        let history = store.dir.join("notes-history");
        fs::create_dir(&history).unwrap();
        git_ok(&history, &["init", "-q"]).unwrap();
        fs::write(history.join("notes.txt"), "原来的记录\n-- removed").unwrap();
        git_ok(&history, &["add", "notes.txt"]).unwrap();
        let stamp = Local.timestamp_opt(first, 0).unwrap().to_rfc3339();
        checked(
            git(
                &history,
                &[
                    "-c",
                    "user.name=Legacy",
                    "-c",
                    "user.email=legacy@local",
                    "-c",
                    "commit.gpgSign=false",
                    "commit",
                    "-qm",
                    "legacy",
                ],
                Some(&stamp),
            )
            .unwrap(),
        )
        .unwrap();
        store.checkpoint_at("原来的记录\n-- removed", last).unwrap();
        store.checkpoint_at("新增内容\n++ added", last).unwrap();
        assert_eq!(store.history_bounds().unwrap(), Some((first, last)));
        assert_eq!(
            String::from_utf8_lossy(
                &git_ok(&history, &["rev-list", "--count", "HEAD"])
                    .unwrap()
                    .stdout
            )
            .trim(),
            "2"
        );
        let report = store.history_increment(last, last).unwrap();
        assert!(report.contains("  + 新增内容"));
        assert!(report.contains("  - 原来的记录"));
        assert!(report.contains("  + ++ added"));
        assert!(report.contains("  - -- removed"));
        assert!(!report.contains("  + 原来的记录"));
        assert!(store
            .history_increment(last + 1, last + 2)
            .unwrap()
            .contains("没有变更"));
    }

    #[test]
    fn git_errors_leave_the_main_note_durable() {
        let (_tmp, store) = fixture();
        fs::write(store.dir.join("notes-history"), "占用目录名").unwrap();
        store.save_notes("本地保存成功").unwrap();
        assert!(store.start_history("本地保存成功").is_err());
        assert_eq!(store.load_notes().unwrap(), "本地保存成功");
    }

    #[test]
    fn first_launch_keeps_placeholder_out_of_notes_and_history() {
        let (_tmp, store) = fixture();
        let text = store.load_notes().unwrap();
        assert_eq!(text, DEFAULT_TEXT);
        store.initialize_history(&text).unwrap();
        assert_eq!(fs::read_dir(&store.dir).unwrap().count(), 0);
    }

    #[test]
    fn import_preview_requires_confirmation_for_saved_empty_text_and_settings() {
        let (tmp, store) = fixture();
        let source = tempdir_in(tmp.path()).unwrap();
        fs::write(source.path().join("notes.txt"), "旧便签").unwrap();
        assert!(
            !store
                .inspect_import(source.path())
                .unwrap()
                .has_existing_data
        );
        store.save_notes(DEFAULT_TEXT).unwrap();
        assert!(
            !store
                .inspect_import(source.path())
                .unwrap()
                .has_existing_data
        );
        store.save_notes("").unwrap();
        assert!(
            store
                .inspect_import(source.path())
                .unwrap()
                .has_existing_data
        );
        assert!(store.import_from(source.path(), false).is_err());
        assert_eq!(store.load_notes().unwrap(), "");
        fs::remove_file(store.dir.join("notes.txt")).unwrap();
        store.save_config(&Config::default()).unwrap();
        assert!(
            store
                .inspect_import(source.path())
                .unwrap()
                .has_existing_data
        );
    }

    #[test]
    fn imported_profile_preserves_legacy_git_history_and_uses_override_on_restart() {
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("installed");
        let override_dir = tmp.path().join("override");
        let start = || Store::open(app_data.clone(), Some(override_dir.clone()), None).unwrap();
        let store = start();
        let source = Store::open(tmp.path().join("legacy"), None, None).unwrap();
        let first = 1_700_000_000;
        let last = first + 120;
        source.checkpoint_at("第一条", first).unwrap();
        source.checkpoint_at("第二条", last).unwrap();
        fs::write(source.dir.join("notes.txt"), "\u{feff}最新便签\r\n末行").unwrap();
        fs::write(
            source.dir.join("config.json"),
            br#"{"width":580,"outline":false}"#,
        )
        .unwrap();
        let old_history = source.dir.join("notes-history");
        git_ok(&old_history, &["pack-refs", "--all", "--prune"]).unwrap();
        git_ok(&old_history, &["repack", "-ad"]).unwrap();
        let old_head = git_ok(&old_history, &["rev-parse", "HEAD"]).unwrap().stdout;
        let old_config = old_history.join(".git/config");
        let raw_config = b"[core]\n\thooksPath = custom-hooks\n\tfsmonitor = external-command\n";
        fs::write(&old_config, raw_config).unwrap();
        fs::write(old_history.join(".git/hooks/pre-commit"), "legacy hook").unwrap();

        let preview = store.inspect_import(&source.dir).unwrap();
        let preview_json = serde_json::to_value(preview).unwrap();
        assert!(preview_json["sourceDir"].is_string());
        assert_eq!(preview_json["hasNotes"], true);
        assert_eq!(preview_json["hasConfig"], true);
        assert_eq!(preview_json["hasHistory"], true);
        assert_eq!(preview_json["hasExistingData"], false);
        let profile = PathBuf::from(store.import_from(&source.dir, false).unwrap());
        assert!(profile.starts_with(&override_dir));
        assert!(!app_data.exists());
        assert_eq!(store.load_notes().unwrap(), DEFAULT_TEXT);
        assert_eq!(fs::read(&old_config).unwrap(), raw_config);

        let reopened = start();
        assert_eq!(reopened.dir, profile);
        let text = reopened.load_notes().unwrap();
        assert_eq!(text, "最新便签\r\n末行");
        assert_eq!(reopened.load_config().unwrap().width, 580);
        let imported_history = reopened.dir.join("notes-history");
        assert_eq!(
            git_ok(&imported_history, &["rev-parse", "HEAD"])
                .unwrap()
                .stdout,
            old_head
        );
        reopened.initialize_history(&text).unwrap();
        assert_eq!(
            git_ok(&imported_history, &["rev-parse", "HEAD"])
                .unwrap()
                .stdout,
            old_head
        );
        assert_eq!(reopened.history_bounds().unwrap(), Some((first, last)));
        assert!(!imported_history.join(".git/hooks/pre-commit").exists());
        assert!(!fs::read_to_string(imported_history.join(".git/config"))
            .unwrap()
            .contains("external-command"));
        assert!(reopened.inspect_import(&profile).is_err());

        reopened.checkpoint_at("第三条", last + 120).unwrap();
        assert_eq!(
            reopened.history_bounds().unwrap(),
            Some((first, last + 120))
        );
        assert_eq!(
            git_ok(&imported_history, &["rev-parse", "HEAD^"])
                .unwrap()
                .stdout,
            old_head
        );
        assert_eq!(
            fs::read_to_string(source.dir.join("notes.txt")).unwrap(),
            "\u{feff}最新便签\r\n末行"
        );
    }

    #[test]
    fn failed_import_leaves_selected_profile_and_both_sources_intact() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("target");
        let start = || Store::open(root.clone(), None, None).unwrap();
        let source = tempdir_in(tmp.path()).unwrap();
        fs::write(source.path().join("notes.txt"), "第一次导入").unwrap();
        let first = PathBuf::from(start().import_from(source.path(), false).unwrap());
        let active = start();
        let bad_source = tempdir_in(tmp.path()).unwrap();
        fs::write(bad_source.path().join("notes.txt"), "不完整的第二次导入").unwrap();
        fs::write(bad_source.path().join("config.json"), "corrupt settings").unwrap();
        fs::create_dir_all(bad_source.path().join("notes-history/.git")).unwrap();
        assert!(active.import_from(bad_source.path(), true).is_err());
        assert_eq!(start().dir, first);
        assert_eq!(start().load_notes().unwrap(), "第一次导入");
        assert_eq!(fs::read_dir(first.parent().unwrap()).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(bad_source.path().join("config.json")).unwrap(),
            "corrupt settings"
        );

        fs::write(source.path().join("notes.txt"), "重新导入").unwrap();
        assert!(
            active
                .inspect_import(source.path())
                .unwrap()
                .has_existing_data
        );
        let second = PathBuf::from(active.import_from(source.path(), true).unwrap());
        assert_eq!(start().dir, second);
        assert_eq!(
            fs::read_to_string(first.join("notes.txt")).unwrap(),
            "第一次导入"
        );
        let reopened = start();
        assert!(reopened.inspect_import(&first).unwrap().has_existing_data);
        reopened.import_from(&first, true).unwrap();
        assert_eq!(start().load_notes().unwrap(), "第一次导入");
    }

    #[test]
    fn corrupt_import_config_is_backed_up_only_in_destination() {
        let (_tmp, store) = fixture();
        let source = tempdir().unwrap();
        let config = source.path().join("config.json");
        fs::write(&config, "[1, 2, 3]").unwrap();
        let imported = PathBuf::from(store.import_from(source.path(), false).unwrap());
        let active = Store::open(store.dir.clone(), None, None).unwrap();
        assert_eq!(active.load_config().unwrap(), Config::default());
        assert!(!imported.join("config.json").exists());
        assert!(fs::read_dir(imported).unwrap().flatten().any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("config.json.corrupt-")
            && fs::read_to_string(entry.path()).unwrap() == "[1, 2, 3]"));
        assert_eq!(fs::read_to_string(config).unwrap(), "[1, 2, 3]");
    }

    #[test]
    fn readonly_portable_data_falls_back_and_can_be_imported() {
        let tmp = tempdir().unwrap();
        let portable = tmp.path().join("portable");
        let installed = tmp.path().join("installed");
        fs::create_dir(&portable).unwrap();
        let notes = portable.join("notes.txt");
        fs::write(&notes, "只读便签").unwrap();
        let original_permissions = fs::metadata(&notes).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&notes, permissions).unwrap();
        let store = Store::open(installed.clone(), None, Some(&portable)).unwrap();
        let imported = store.import_from(&portable, false);
        fs::set_permissions(&notes, original_permissions).unwrap();
        assert_eq!(store.dir, installed);
        assert!(store.startup_warning.is_some());
        let reopened = Store::open(store.dir.clone(), None, Some(&portable)).unwrap();
        assert_eq!(reopened.dir, PathBuf::from(imported.unwrap()));
        assert_eq!(reopened.load_notes().unwrap(), "只读便签");
        reopened.save_notes("可以保存").unwrap();
        assert_eq!(fs::read_to_string(notes).unwrap(), "只读便签");
    }

    #[test]
    fn unknown_legacy_shortcut_warns_and_keeps_other_settings() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("config.json"),
            br#"{"width":520,"hotkey_mods":12,"hotkey_vk":255}"#,
        )
        .unwrap();
        let store = Store::open(tmp.path().to_owned(), None, None).unwrap();
        assert_eq!(store.load_config().unwrap().shortcut, DEFAULT_SHORTCUT);
        assert_eq!(store.load_config().unwrap().width, 520);
        assert!(store.startup_warning.as_deref().unwrap().contains("快捷键"));
    }
}
