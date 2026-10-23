//! `vibegate update` —— 自更新子命令。
//!
//! 行为与 `install.sh` 一致:从 GitHub Releases 下载对应平台的预编译二进制,
//! 装成 `vibegate-<版本>`,再把 `vibegate` 软链切过去。升级不覆盖正在运行的
//! 二进制;同版本 + 软链完好时提示并退出。
//!
//! 下载用外部 curl(复用系统 TLS,不引 reqwest);旧版本清理的交互提示只在
//! 有终端时出现,`VIBEGATE_NONINTERACTIVE=1` 或无 tty 时走安全默认(保留)。

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Release repo for downloads: CI builds embed the real GITHUB_REPOSITORY
/// via VIBEGATE_REPO (see release.yml); local builds fall back to the default.
const REPO: &str = match option_env!("VIBEGATE_REPO") {
    Some(r) => r,
    None => "second-state/vibegate",
};

/// 检测 (资产名, 安装文件名)。与 install.sh / release.yml 的命名一致。
fn detect() -> Result<(String, String), String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("linux", "x86_64") => Ok(("vibegate-linux-x64".into(), "vibegate".into())),
        ("linux", "aarch64") => Ok(("vibegate-linux-arm64".into(), "vibegate".into())),
        ("macos", "aarch64") => Ok(("vibegate-macos-arm64".into(), "vibegate".into())),
        ("windows", "x86_64") => Ok(("vibegate-windows-x64.exe".into(), "vibegate.exe".into())),
        (os, arch) => Err(format!(
            "no prebuilt binary for {os}/{arch} — check https://github.com/{REPO}/releases"
        )),
    }
}

/// 安装目录:当前二进制所在目录(自然覆盖 cargo install / install.sh 的位置),
/// 拿不到则回退 `~/.cargo/bin`。
fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .filter(|d| d.exists())
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".cargo").join("bin"))
                .unwrap_or_else(|_| PathBuf::from("."))
        })
}

fn is_interactive() -> bool {
    std::io::stdin().is_terminal() && std::env::var("VIBEGATE_NONINTERACTIVE").as_deref() != Ok("1")
}

fn curl_download(url: &str, dest: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args(["-fL", "--progress-bar", "-o"])
        .arg(dest)
        .arg(url)
        .status()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("download failed (curl exit {status})"))
    }
}

/// 从文件读版本(--version 输出如 "vibegate 0.1.0")。
fn version_of(exe: &Path) -> Option<String> {
    let out = Command::new(exe).arg("--version").output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .nth(1)
        .map(String::from)
}

fn ask_delete_old() -> bool {
    print!("Delete old version(s)? [y/N] ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut ans = String::new();
    std::io::stdin()
        .read_line(&mut ans)
        .is_ok()
        && ans.trim().starts_with(['y', 'Y'])
}

/// 入口:`vibegate update [TAG]`。TAG 缺省 = latest(不含 prerelease)。
pub async fn run(tag: Option<String>) -> Result<(), String> {
    let (asset, name) = detect()?;
    let tag = tag.unwrap_or_else(|| "latest".to_string());
    let install_dir = install_dir();

    let url = if tag == "latest" {
        format!("https://github.com/{REPO}/releases/latest/download/{asset}")
    } else {
        format!("https://github.com/{REPO}/releases/download/{tag}/{asset}")
    };

    std::fs::create_dir_all(&install_dir).map_err(|e| e.to_string())?;
    let staging = install_dir.join(format!(".vibegate.update.{}", std::process::id()));

    println!("Downloading {url}");
    curl_download(&url, &staging)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755));
    }

    let binver = match version_of(&staging) {
        Some(v) if !v.is_empty() => v,
        _ => {
            let _ = std::fs::remove_file(&staging);
            return Err("cannot read version from the downloaded binary".to_string());
        }
    };

    #[cfg(unix)]
    {
        let versioned = install_dir.join(format!("{name}-{binver}"));
        let link = install_dir.join(&name);
        if versioned.exists() && std::fs::read_link(&link).is_ok_and(|l| l == versioned) {
            let _ = std::fs::remove_file(&staging);
            println!(
                "vibegate {binver} is already installed at {} — up to date",
                versioned.display()
            );
            return Ok(());
        }
        std::fs::rename(&staging, &versioned).map_err(|e| e.to_string())?;
        // 建链前先移除旧文件/旧软链:cargo install 装出的是普通文件,直接
        // symlink 会报 EEXIST。rm 对运行中的进程是安全的。
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&versioned, &link).map_err(|e| e.to_string())?;
        println!(
            "Installed: {} -> {} ({binver})",
            versioned.display(),
            link.display()
        );
    }

    #[cfg(windows)]
    {
        let target = install_dir.join(&name);
        if target.exists()
            && version_of(&target).as_deref() == Some(binver.as_str())
        {
            let _ = std::fs::remove_file(&staging);
            println!(
                "vibegate {binver} is already installed at {} — up to date",
                target.display()
            );
            return Ok(());
        }
        // Windows:先给旧 exe 改名留档(运行中的 exe 只能 rename 不能覆盖),
        // 再把新 exe 改名到位。
        if target.exists() {
            let oldver = version_of(&target).unwrap_or_else(|| "old".into());
            let old_path = install_dir.join(format!("{name}-{oldver}.exe"));
            let _ = std::fs::remove_file(&old_path);
            std::fs::rename(&target, &old_path).map_err(|e| format!("cannot move old exe: {e}"))?;
        }
        std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
        println!("Installed: {} ({binver})", target.display());
    }

    // --- 旧版本清理:列出 vibegate-<数字>* 历史版本,交互时询问(默认 N)。 ---
    let prefix = format!("{name}-");
    #[cfg(unix)]
    let installed_file = install_dir.join(format!("{name}-{binver}"));
    #[cfg(windows)]
    let installed_file = install_dir.join(&name);
    let olds: Vec<PathBuf> = std::fs::read_dir(&install_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| {
                        n.starts_with(&prefix)
                            && n[prefix.len()..].starts_with(|c: char| c.is_ascii_digit())
                    })
        })
        .filter(|p| *p != installed_file)
        .collect();
    if !olds.is_empty() {
        println!("Found old version(s):");
        for p in &olds {
            println!("  {}", p.display());
        }
        if is_interactive() && ask_delete_old() {
            for p in &olds {
                let _ = std::fs::remove_file(p);
                println!("Removed: {}", p.display());
            }
        } else {
            println!("kept — delete manually if no longer needed");
        }
    }

    // --- PATH 检查:不在 PATH 就提示。 ---
    let dir_str = install_dir.display().to_string();
    let in_path = std::env::var("PATH")
        .map(|p| p.split(':').chain(p.split(';')).any(|s| s == dir_str))
        .unwrap_or(false);
    if !in_path {
        println!("note: {dir_str} is not on your PATH — add it to your shell rc if needed");
    }

    // --- 升级的是常驻守护进程的话,提示重启。 ---
    println!("if a daemon is still running the old version: vibegate stop && vibegate run -d");

    Ok(())
}
