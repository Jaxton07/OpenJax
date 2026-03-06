use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use time::format_description::well_known::Rfc3339;
use tracing::Level;
use tracing_subscriber::filter::FilterExt;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::OffsetTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry;

use crate::OpenJaxPaths;

const DEFAULT_MAX_LINES: usize = 10_000;
const DEFAULT_MAX_ARCHIVES: usize = 4;
const LOG_FILE_NAME: &str = "openjax.log";

pub struct LoggerConfig {
    pub log_dir: PathBuf,
    pub log_file_name: String,
    pub max_lines: usize,
    pub max_archives: usize,
    pub session_id: String,
}

impl LoggerConfig {
    pub fn new() -> Self {
        let log_dir = Self::resolve_log_dir();

        let session_id = generate_session_id();

        Self {
            log_dir,
            log_file_name: LOG_FILE_NAME.to_string(),
            max_lines: DEFAULT_MAX_LINES,
            max_archives: DEFAULT_MAX_ARCHIVES,
            session_id,
        }
    }

    fn resolve_log_dir() -> PathBuf {
        if let Some(paths) = OpenJaxPaths::detect() {
            if paths.ensure_runtime_dirs().is_ok() {
                return paths.logs_dir;
            }
        }

        PathBuf::from(".openjax/logs")
    }

    pub fn from_env() -> Self {
        let mut config = Self::new();

        if let Ok(val) = std::env::var("OPENJAX_LOG_MAX_LINES") {
            if let Ok(lines) = val.parse::<usize>() {
                if lines > 0 {
                    config.max_lines = lines;
                }
            }
        }

        if let Ok(val) = std::env::var("OPENJAX_LOG_MAX_ARCHIVES") {
            if let Ok(archives) = val.parse::<usize>() {
                config.max_archives = archives;
            }
        }
        if let Ok(val) = std::env::var("OPENJAX_LOG_FILE") {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                config.log_file_name = trimmed.to_string();
            }
        }

        config
    }
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn generate_session_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let hash = format!("{:x}", now.wrapping_add(pid as u128));
    hash.chars().take(4).collect()
}

pub fn init_logger() -> Option<()> {
    init_logger_with_file(LOG_FILE_NAME)
}

pub fn init_logger_with_file(log_file_name: &str) -> Option<()> {
    if std::env::var("OPENJAX_LOG").is_ok_and(|v| v == "off") {
        return Some(());
    }

    let mut config = LoggerConfig::from_env();
    let trimmed_file = log_file_name.trim();
    if !trimmed_file.is_empty() {
        config.log_file_name = trimmed_file.to_string();
    }

    if fs::create_dir_all(&config.log_dir).is_err() {
        return Some(());
    }

    let file_writer = RollingFileWriter::new(
        config.log_dir.clone(),
        config.log_file_name.clone(),
        config.max_lines,
        config.max_archives,
    );

    let level_filter = get_log_level_from_env();

    let timer = OffsetTime::local_rfc_3339().unwrap_or_else(|_| {
        let offset = time::UtcOffset::from_hms(8, 0, 0).unwrap();
        OffsetTime::new(offset, Rfc3339)
    });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_timer(timer)
        .with_filter(tracing_subscriber::filter::LevelFilter::from(level_filter));

    let subscriber = registry().with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| eprintln!("[logger] failed to set subscriber: {}", e))
        .ok()?;

    tracing::info!(
        "logger initialized session={} pid={} log_dir={} file={}",
        config.session_id,
        std::process::id(),
        config.log_dir.display(),
        config.log_file_name
    );

    Some(())
}

pub fn init_split_logger(core_log_file_name: &str, tui_log_file_name: &str) -> Option<()> {
    if std::env::var("OPENJAX_LOG").is_ok_and(|v| v == "off") {
        return Some(());
    }

    let mut config = LoggerConfig::from_env();
    let core_file = core_log_file_name.trim();
    let tui_file = tui_log_file_name.trim();
    if core_file.is_empty() || tui_file.is_empty() {
        return init_logger();
    }
    config.log_file_name = core_file.to_string();

    if fs::create_dir_all(&config.log_dir).is_err() {
        return Some(());
    }

    let core_writer = RollingFileWriter::new(
        config.log_dir.clone(),
        core_file.to_string(),
        config.max_lines,
        config.max_archives,
    );
    let tui_writer = RollingFileWriter::new(
        config.log_dir.clone(),
        tui_file.to_string(),
        config.max_lines,
        config.max_archives,
    );

    let level_filter = get_log_level_from_env();
    let timer = OffsetTime::local_rfc_3339().unwrap_or_else(|_| {
        let offset = time::UtcOffset::from_hms(8, 0, 0).unwrap();
        OffsetTime::new(offset, Rfc3339)
    });

    let core_filter = tracing_subscriber::filter::filter_fn(|meta| !is_tui_target(meta.target()))
        .and(tracing_subscriber::filter::LevelFilter::from(level_filter));
    let tui_filter = tracing_subscriber::filter::filter_fn(|meta| is_tui_target(meta.target()))
        .and(tracing_subscriber::filter::LevelFilter::from(level_filter));

    let core_layer = tracing_subscriber::fmt::layer()
        .with_writer(core_writer)
        .with_ansi(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_timer(timer.clone())
        .with_filter(core_filter);

    let tui_layer = tracing_subscriber::fmt::layer()
        .with_writer(tui_writer)
        .with_ansi(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_timer(timer)
        .with_filter(tui_filter);

    let subscriber = registry().with(core_layer).with(tui_layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| eprintln!("[logger] failed to set subscriber: {}", e))
        .ok()?;

    tracing::info!(
        "split logger initialized session={} pid={} log_dir={} core_file={} tui_file={}",
        config.session_id,
        std::process::id(),
        config.log_dir.display(),
        core_file,
        tui_file
    );

    Some(())
}

fn is_tui_target(target: &str) -> bool {
    target == "tui_next" || target.starts_with("tui_next::")
}

fn get_log_level_from_env() -> Level {
    match std::env::var("OPENJAX_LOG_LEVEL")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    }
}

fn count_file_lines(path: &PathBuf) -> Option<usize> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    Some(reader.lines().count())
}

#[derive(Clone)]
pub struct RollingFileWriter {
    log_dir: PathBuf,
    log_file_name: String,
    archive_prefix: String,
    max_lines: usize,
    max_archives: usize,
    line_count: Arc<AtomicU64>,
}

impl RollingFileWriter {
    pub fn new(
        log_dir: PathBuf,
        log_file_name: String,
        max_lines: usize,
        max_archives: usize,
    ) -> Self {
        let archive_prefix = PathBuf::from(&log_file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("openjax")
            .to_string();
        let log_file_path = log_dir.join(&log_file_name);
        let existing_lines = count_file_lines(&log_file_path).unwrap_or(0) as u64;
        Self {
            log_dir,
            log_file_name,
            archive_prefix,
            max_lines,
            max_archives,
            line_count: Arc::new(AtomicU64::new(existing_lines)),
        }
    }

    fn rotate(&self) {
        let archive_name = format!("{}.{}.log", self.archive_prefix, chrono_timestamp());
        let archive_path = self.log_dir.join(&archive_name);
        let current_path = self.log_dir.join(&self.log_file_name);

        if let Err(e) = fs::rename(&current_path, &archive_path) {
            eprintln!("[logger] failed to rotate log: {}", e);
            return;
        }

        self.line_count.store(0, Ordering::SeqCst);

        self.cleanup_old_archives();
    }

    fn cleanup_old_archives(&self) {
        let Ok(entries) = fs::read_dir(&self.log_dir) else {
            return;
        };

        let mut archives: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let prefix = format!("{}.", self.archive_prefix);
                name.starts_with(&prefix) && name.ends_with(".log") && name != self.log_file_name
            })
            .map(|e| e.path())
            .collect();

        archives.sort();

        while archives.len() > self.max_archives {
            if let Some(old) = archives.first() {
                let _ = fs::remove_file(old);
            }
            archives.remove(0);
        }
    }

    fn open_file(&self) -> File {
        let log_file_path = self.log_dir.join(&self.log_file_name);
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .unwrap_or_else(|e| {
                eprintln!("[logger] failed to open log file: {}", e);
                panic!("cannot open log file");
            })
    }
}

fn chrono_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    chrono_like_format(now)
}

fn chrono_like_format(unix_secs: u64) -> String {
    const SECONDS_PER_DAY: u64 = 86400;
    const DAYS_BEFORE_1970: u64 = 719527;

    let days = unix_secs / SECONDS_PER_DAY;
    let secs_in_day = unix_secs % SECONDS_PER_DAY;

    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;

    let (year, month, day) = days_to_ymd(days + DAYS_BEFORE_1970);

    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let days = days as i64;

    let a = days + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (146097 * b) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;

    let day = e - (153 * m + 2) / 153 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = 100 * b + d - 4800 + (m / 10);

    (year as u64, month as u64, day as u64)
}

pub struct RollingFileWriterGuard {
    writer: File,
}

impl IoWrite for RollingFileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl MakeWriter<'_> for RollingFileWriter {
    type Writer = RollingFileWriterGuard;

    fn make_writer(&self) -> Self::Writer {
        if self.line_count.load(Ordering::SeqCst) >= self.max_lines as u64 {
            self.rotate();
        }

        self.line_count.fetch_add(1, Ordering::SeqCst);

        RollingFileWriterGuard {
            writer: self.open_file(),
        }
    }
}
