use anyhow::Result;

fn main() -> Result<()> {
    let GitInfo {
        commit_hash,
        commit_date,
    } = get_git_info()?;

    println!("cargo:rustc-env=EMULATOR_COMMIT_HASH={commit_hash}");
    println!("cargo:rustc-env=EMULATOR_COMMIT_DATE={commit_date}");
    Ok(())
}

struct GitInfo {
    commit_hash: String,
    commit_date: String,
}

fn get_git_info() -> Result<GitInfo> {
    let pkg_dir = std::path::PathBuf::from(env("CARGO_MANIFEST_DIR")?);
    let git_dir = command("git", &["rev-parse", "--git-dir"], Some(pkg_dir));
    let git_dir = match git_dir {
        #[cfg(unix)]
        Ok(git_dir) => {
            use std::os::unix::ffi::OsStringExt;
            std::path::PathBuf::from(std::ffi::OsString::from_vec(git_dir))
        }
        #[cfg(windows)]
        Ok(git_dir) => std::path::PathBuf::from(String::from_utf8_lossy(&git_dir).into_owned()),
        Err(msg) => {
            println!("cargo:warning=unable to determine git version (not in git repository?)");
            println!("cargo:warning={msg}");
            return Ok(GitInfo {
                commit_hash: "unknown".to_owned(),
                commit_date: "unknown".to_owned(),
            });
        }
    };

    for subpath in ["HEAD", "logs/HEAD", "index"] {
        let path = git_dir.join(subpath).canonicalize()?;
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let args = &["show", "-s", "--format=%H"];
    let out = command("git", args, None)?;
    let commit_hash = match String::from_utf8_lossy(&out) {
        std::borrow::Cow::Borrowed(version) => version.trim().to_string(),
        std::borrow::Cow::Owned(version) => {
            anyhow::bail!("git: invalid output: {version}")
        }
    };

    let args = &["show", "-s", "--format=%ci"];
    let out = command("git", args, None)?;
    let commit_date = match String::from_utf8_lossy(&out) {
        std::borrow::Cow::Borrowed(version) => version.trim().to_string(),
        std::borrow::Cow::Owned(version) => {
            anyhow::bail!("git: invalid output: {version}")
        }
    };

    Ok(GitInfo {
        commit_hash,
        commit_date,
    })
}

fn command(prog: &str, args: &[&str], cwd: Option<std::path::PathBuf>) -> Result<Vec<u8>> {
    println!("cargo:rerun-if-env-changed=PATH");
    let mut cmd = std::process::Command::new(prog);
    cmd.args(args);
    cmd.stderr(std::process::Stdio::inherit());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let out = cmd.output()?;
    if out.status.success() {
        let mut stdout = out.stdout;
        if let Some(b'\n') = stdout.last() {
            stdout.pop();
            if let Some(b'\r') = stdout.last() {
                stdout.pop();
            }
        }
        Ok(stdout)
    } else if let Some(code) = out.status.code() {
        anyhow::bail!("{prog}: terminated with {code}");
    } else {
        anyhow::bail!("{prog}: killed by signal")
    }
}

fn env(key: &str) -> Result<std::ffi::OsString> {
    println!("cargo:rerun-if-env-changed={}", key);
    std::env::var_os(key).ok_or_else(|| anyhow::anyhow!("missing '{}' environment variable", key))
}
