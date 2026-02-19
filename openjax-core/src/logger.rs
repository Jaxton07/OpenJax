use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use time::format_description::well_known::Rfc3339;
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::OffsetTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry;

const DEFAULT_MAX_LINES: usize = 10_000;
const DEFAULT_MAX_ARCHIVES: usize = 4;
const LOG_FILE_NAME: &str = "openjax.log";

static LINE_COUNT: AtomicU64 = AtomicU64::new(0);

pub struct LoggerConfig {
    pub log_dir: PathBuf,
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
            max_lines: DEFAULT_MAX_LINES,
            max_archives: DEFAULT_MAX_ARCHIVES,
            session_id,
        }
    }

    fn resolve_log_dir() -> PathBuf {
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_logs = cwd.join(".openjax").join("logs");
            if fs::create_dir_all(&cwd_logs).is_ok() {
                return cwd_logs;
            }
        }

        if let Some(home) = dirs::home_dir() {
            let home_logs = home.join(".openjax").join("logs");
            if fs::create_dir_all(&home_logs).is_ok() {
                return home_logs;
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
    if std::env::var("OPENJAX_LOG").is_ok_and(|v| v == "off") {
        return Some(());
    }

    let config = LoggerConfig::from_env();

    if fs::create_dir_all(&config.log_dir).is_err() {
        return Some(());
    }

    let log_file_path = config.log_dir.join(LOG_FILE_NAME);
    let existing_lines = count_file_lines(&log_file_path).unwrap_or(0);
    LINE_COUNT.store(existing_lines as u64, Ordering::SeqCst);

    let file_writer = RollingFileWriter::new(
        config.log_dir.clone(),
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
        "logger initialized session={} pid={} log_dir={}",
        config.session_id,
        std::process::id(),
        config.log_dir.display()
    );

    Some(())
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
    max_lines: usize,
    max_archives: usize,
}

impl RollingFileWriter {
    pub fn new(log_dir: PathBuf, max_lines: usize, max_archives: usize) -> Self {
        Self {
            log_dir,
            max_lines,
            max_archives,
        }
    }

    fn rotate(&self) {
        let archive_name = format!("openjax.{}.log", chrono_timestamp());
        let archive_path = self.log_dir.join(&archive_name);
        let current_path = self.log_dir.join(LOG_FILE_NAME);

        if let Err(e) = fs::rename(&current_path, &archive_path) {
            eprintln!("[logger] failed to rotate log: {}", e);
            return;
        }

        LINE_COUNT.store(0, Ordering::SeqCst);

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
                name.starts_with("openjax.") && name.ends_with(".log") && name != LOG_FILE_NAME
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
        let log_file_path = self.log_dir.join(LOG_FILE_NAME);
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
        if LINE_COUNT.load(Ordering::SeqCst) >= self.max_lines as u64 {
            self.rotate();
        }

        LINE_COUNT.fetch_add(1, Ordering::SeqCst);

        RollingFileWriterGuard {
            writer: self.open_file(),
        }
    }
}
