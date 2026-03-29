use anyhow::{Context, bail};
use clap::Args;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::{Path, PathBuf};

const DEFAULT_REPO: &str = "Jaxton07/OpenJax";
const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Args)]
pub struct UpdateArgs {
    /// Target version, e.g. 1.2.3 (default: latest)
    #[arg(long, default_value = "latest")]
    pub version: String,

    /// GitHub repository owner/name
    #[arg(long, default_value = DEFAULT_REPO)]
    pub repo: String,

    /// Install prefix (default: auto-detected from binary location)
    #[arg(long)]
    pub prefix: Option<PathBuf>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Do not stop running OpenJax processes before updating
    #[arg(long)]
    pub skip_stop: bool,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

pub async fn run(args: UpdateArgs) -> anyhow::Result<()> {
    let prefix = resolve_prefix(args.prefix)?;
    let platform = detect_platform()?;
    let client = build_client()?;

    println!("[update] resolving version...");
    let tag = if args.version == "latest" {
        fetch_latest_tag(&client, &args.repo).await?
    } else {
        format!("v{}", args.version.trim_start_matches('v'))
    };
    let version = tag.trim_start_matches('v').to_string();
    let artifact = format!("openjax-v{}-{}.tar.gz", version, platform);

    println!("[update] version : {}", version);
    println!("[update] platform: {}", platform);
    println!("[update] prefix  : {}", prefix.display());

    if !args.yes {
        print!("[update] proceed? [y/N] ");
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        if !line.trim().eq_ignore_ascii_case("y") {
            println!("[update] aborted");
            return Ok(());
        }
    }

    if !args.skip_stop {
        stop_openjax_processes();
    }

    let tmp = tempfile::tempdir().context("creating temp dir")?;
    let base_url = format!("https://github.com/{}/releases/download/{}", args.repo, tag);

    let artifact_path = tmp.path().join(&artifact);
    println!("[update] downloading {}...", artifact);
    download_file(
        &client,
        &format!("{}/{}", base_url, artifact),
        &artifact_path,
    )
    .await
    .with_context(|| format!("downloading {}", artifact))?;

    let checksums_path = tmp.path().join("SHA256SUMS");
    download_file(
        &client,
        &format!("{}/SHA256SUMS", base_url),
        &checksums_path,
    )
    .await
    .context("downloading SHA256SUMS")?;

    verify_checksum(&artifact_path, &checksums_path, &artifact)?;

    extract_and_install(&artifact_path, tmp.path(), &prefix)?;

    println!("[update] update completed successfully ({})", version);
    println!("[update] restart services if needed:");
    println!("  openjax-gateway");
    println!("  tui_next");
    Ok(())
}

// --- helpers ---

fn resolve_prefix(arg: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(p) = arg {
        return Ok(p);
    }
    // Detect from binary location: <prefix>/bin/openjax -> <prefix>
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin) = exe.parent() {
            if let Some(prefix) = bin.parent() {
                return Ok(prefix.to_path_buf());
            }
        }
    }
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".local").join("openjax"))
}

fn detect_platform() -> anyhow::Result<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Ok("macos-aarch64");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Ok("linux-x86_64");
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))]
    bail!(
        "unsupported platform: {} {}. Supported: macOS arm64, Linux x86_64",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

fn build_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("openjax-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("building HTTP client")
}

async fn fetch_latest_tag(client: &reqwest::Client, repo: &str) -> anyhow::Result<String> {
    let url = format!("{}/repos/{}/releases/latest", GITHUB_API_BASE, repo);
    let release: GithubRelease = client
        .get(&url)
        .send()
        .await
        .context("fetching latest release info")?
        .error_for_status()
        .context("GitHub API returned error")?
        .json()
        .await
        .context("parsing release JSON")?;
    Ok(release.tag_name)
}

async fn download_file(client: &reqwest::Client, url: &str, dest: &Path) -> anyhow::Result<()> {
    let bytes = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?
        .error_for_status()
        .with_context(|| format!("HTTP error for {}", url))?
        .bytes()
        .await
        .context("reading response body")?;
    std::fs::write(dest, &bytes).with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

fn verify_checksum(
    artifact_path: &Path,
    checksums_path: &Path,
    artifact_name: &str,
) -> anyhow::Result<()> {
    let data = std::fs::read(artifact_path).context("reading artifact for checksum")?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual = format!("{:x}", hasher.finalize());

    let checksums = std::fs::read_to_string(checksums_path).context("reading SHA256SUMS")?;
    let expected = checksums
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let name = parts.next()?;
            if name == artifact_name {
                Some(hash.to_string())
            } else {
                None
            }
        })
        .with_context(|| format!("checksum not found for {}", artifact_name))?;

    if actual != expected {
        bail!(
            "checksum mismatch for {}\n  expected: {}\n  actual:   {}",
            artifact_name,
            expected,
            actual
        );
    }
    println!("[update] checksum verified");
    Ok(())
}

fn extract_and_install(archive: &Path, tmp_dir: &Path, prefix: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive).context("opening archive")?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(tmp_dir).context("extracting archive")?;

    let pkg_dir = std::fs::read_dir(tmp_dir)
        .context("reading temp dir")?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && e.file_name().to_string_lossy().starts_with("openjax-v")
        })
        .map(|e| e.path())
        .context("package directory not found in archive")?;

    let bin_src = pkg_dir.join("bin");
    let web_src = pkg_dir.join("web");
    let bin_dst = prefix.join("bin");
    let web_dst = prefix.join("web");

    std::fs::create_dir_all(&bin_dst).context("creating bin dir")?;
    std::fs::create_dir_all(&web_dst).context("creating web dir")?;

    for entry in std::fs::read_dir(&bin_src).context("reading bin dir")? {
        let entry = entry?;
        let dst = bin_dst.join(entry.file_name());
        std::fs::copy(entry.path(), &dst)
            .with_context(|| format!("installing {}", entry.file_name().to_string_lossy()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    println!("[update] binaries installed to: {}", bin_dst.display());

    copy_dir_all(&web_src, &web_dst).context("installing web assets")?;
    println!("[update] web assets installed to: {}", web_dst.display());

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

fn stop_openjax_processes() {
    #[cfg(unix)]
    {
        let targets = ["openjax-gateway", "openjaxd", "tui_next"];
        let mut found = false;
        for name in &targets {
            let status = std::process::Command::new("pkill")
                .arg("-f")
                .arg(name)
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                println!("[update] stopped process: {}", name);
                found = true;
            }
        }
        if found {
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    }
}
