use std::ffi::{CString, OsString};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use nix::mount::{MsFlags, mount};
use nix::sched::{CloneFlags, unshare};
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::{
    ForkResult, chdir, chroot, execvp, fork, getcwd, getgid, gethostname, getpid, getuid,
    sethostname,
};

const DEFAULT_ROOT_FS: &str = "/home/riley/testing/rootfs";
const DEFAULT_CONTAINER_DIR: &str = "/home/riley/testing/container";
const DEFAULT_COMMAND: &str = "ls";

struct Config {
    root_fs: PathBuf,
    container_dir: PathBuf,
    command: Vec<CString>,
}

fn main() -> anyhow::Result<()> {
    let config = parse_args(std::env::args_os().skip(1))?;

    print_proc_info("Before isolation")?;

    let uid_map = format!("0 {} 1", getuid());
    let gid_map = format!("0 {} 1", getgid());

    unshare(CloneFlags::CLONE_NEWUSER).context("failed to isolate user namespace")?;
    write_proc_mappings(&uid_map, &gid_map)?;

    unshare(CloneFlags::CLONE_NEWUTS).context("failed to isolate UTS namespace")?;
    sethostname("my-container").context("failed to set hostname")?;

    unshare(CloneFlags::CLONE_NEWPID).context("failed to isolate PID namespace")?;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => match waitpid(child, None)? {
            WaitStatus::Exited(_, 0) => Ok(()),
            WaitStatus::Exited(_, code) => bail!("container exited with status {code}"),
            WaitStatus::Signaled(_, signal, _) => {
                bail!("container terminated by signal {signal}")
            }
            status => bail!("unexpected wait status: {status:?}"),
        },
        Ok(ForkResult::Child) => child(&config.root_fs, &config.container_dir, &config.command),
        Err(e) => Err(e).context("fork() failed"),
    }
}

fn parse_args<I>(args: I) -> anyhow::Result<Config>
where
    I: IntoIterator<Item = OsString>,
{
    let mut root_fs = PathBuf::from(DEFAULT_ROOT_FS);
    let mut container_dir = PathBuf::from(DEFAULT_CONTAINER_DIR);
    let mut command = Vec::<OsString>::new();
    let mut positional_dirs = Vec::<PathBuf>::new();
    let mut command_mode = false;

    for arg in args {
        if !command_mode && arg == "--" {
            command_mode = true;
            continue;
        }

        if command_mode {
            command.push(arg);
            continue;
        }

        if positional_dirs.len() < 2 {
            positional_dirs.push(PathBuf::from(arg));
        } else {
            command_mode = true;
            command.push(arg);
        }
    }

    if let Some(path) = positional_dirs.first() {
        root_fs.clone_from(path);
    }

    if let Some(path) = positional_dirs.get(1) {
        container_dir.clone_from(path);
    }

    let command = if command.is_empty() {
        vec![CString::new(DEFAULT_COMMAND).context("default command contains a null byte")?]
    } else {
        command
            .into_iter()
            .map(os_string_to_cstring)
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(Config {
        root_fs,
        container_dir,
        command,
    })
}

fn child(root_fs: &Path, container_dir: &Path, command: &[CString]) -> anyhow::Result<()> {
    unshare(CloneFlags::CLONE_NEWNS).context("failed to isolate mount namespace")?;
    mount(
        None::<&str>,
        Path::new("/"),
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )
    .context("failed to make mount namespace private")?;

    prepare_container_dir(root_fs, container_dir)?;
    mount(
        Some(root_fs),
        container_dir,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .with_context(|| {
        format!(
            "failed to bind mount rootfs from {} to {}",
            root_fs.display(),
            container_dir.display()
        )
    })?;

    chroot(container_dir).context("failed to change the root directory")?;
    chdir("/").context("failed to change working directory to /")?;
    mount(
        Some("proc"),
        Path::new("/proc"),
        Some("proc"),
        MsFlags::empty(),
        None::<&str>,
    )
    .context("failed to mount proc inside the container")?;

    print_proc_info("Container isolation")?;
    execvp(&command[0], command).context("execvp() failed")?;
    Ok(())
}

fn prepare_container_dir(root_fs: &Path, container_dir: &Path) -> anyhow::Result<()> {
    if !root_fs.is_dir() {
        bail!(
            "rootfs path does not exist or is not a directory: {}",
            root_fs.display()
        );
    }

    if let Some(parent) = container_dir.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory {}", parent.display()))?;
    }

    std::fs::create_dir_all(container_dir).with_context(|| {
        format!(
            "failed to create container directory {}",
            container_dir.display()
        )
    })?;

    Ok(())
}

fn print_proc_info(label: &str) -> anyhow::Result<()> {
    eprintln!("[{label}]");
    eprintln!(
        "uid [{}]\n\thostname [{}] \n\tpid [{}] \n\tcwd [{}]",
        getuid(),
        gethostname()?.display(),
        getpid(),
        getcwd()?.display()
    );
    Ok(())
}

fn write_proc_mappings(uid_map: &str, gid_map: &str) -> anyhow::Result<()> {
    std::fs::write("/proc/self/setgroups", "deny").context("failed to disable setgroups")?;
    std::fs::write("/proc/self/uid_map", uid_map).context("failed to write uid map")?;
    std::fs::write("/proc/self/gid_map", gid_map).context("failed to write gid map")?;

    Ok(())
}

fn os_string_to_cstring(value: OsString) -> anyhow::Result<CString> {
    CString::new(value.into_vec()).context("command contains an interior null byte")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn defaults_to_ls_when_no_command_is_provided() {
        let config = parse_args(Vec::<OsString>::new()).expect("parse config");

        assert_eq!(config.root_fs, PathBuf::from(DEFAULT_ROOT_FS));
        assert_eq!(config.container_dir, PathBuf::from(DEFAULT_CONTAINER_DIR));
        assert_eq!(config.command.len(), 1);
        assert_eq!(
            config.command[0].as_c_str().to_str().unwrap(),
            DEFAULT_COMMAND
        );
    }

    #[test]
    fn accepts_positional_rootfs_container_and_command() {
        let config = parse_args([
            OsString::from("/tmp/rootfs"),
            OsString::from("/tmp/container"),
            OsString::from("/bin/sh"),
            OsString::from("-l"),
        ])
        .expect("parse config");

        assert_eq!(config.root_fs, PathBuf::from("/tmp/rootfs"));
        assert_eq!(config.container_dir, PathBuf::from("/tmp/container"));
        assert_eq!(config.command.len(), 2);
        assert_eq!(config.command[0].as_c_str().to_str().unwrap(), "/bin/sh");
        assert_eq!(config.command[1].as_c_str().to_str().unwrap(), "-l");
    }

    #[test]
    fn accepts_command_after_double_dash() {
        let config = parse_args([
            OsString::from("/tmp/rootfs"),
            OsString::from("/tmp/container"),
            OsString::from("--"),
            OsString::from("ls"),
            OsString::from("-la"),
        ])
        .expect("parse config");

        assert_eq!(config.root_fs, PathBuf::from("/tmp/rootfs"));
        assert_eq!(config.container_dir, PathBuf::from("/tmp/container"));
        assert_eq!(config.command.len(), 2);
        assert_eq!(config.command[0].as_c_str().to_str().unwrap(), "ls");
        assert_eq!(config.command[1].as_c_str().to_str().unwrap(), "-la");
    }

    #[test]
    fn rejects_command_arguments_with_null_bytes() {
        let result = parse_args([
            OsString::from("/tmp/rootfs"),
            OsString::from("/tmp/container"),
            OsString::from("--"),
            OsString::from_vec(vec![b'a', 0, b'b']),
        ]);
        assert!(result.is_err());
    }
}
