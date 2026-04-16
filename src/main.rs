use std::ffi::CString;
use std::path::PathBuf;
use anyhow::Context;
use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{chroot, execvp, fork, getcwd, getgid, gethostname, getpid, getuid, sethostname, ForkResult};

const ROOT_FS: &str = "/home/riley/testing/rootfs";
const CONTAINER_DIR: &str = "/home/riley/testing/container";


fn main() -> anyhow::Result<()> {
    
    let arg1 = std::env::args().nth(1);
    let arg_rootfs_dir = arg1.as_deref();

    let arg2 = std::env::args().nth(2);
    let arg_container_dir = arg2.as_deref();
    
    let root_fs = PathBuf::from(arg_rootfs_dir.unwrap_or(ROOT_FS));
    let container_dir = PathBuf::from(arg_container_dir.unwrap_or(CONTAINER_DIR));
    print_proc_info("Before Isolation")?;

    let uid_map = format!("0 {} 1", getuid());
    let gid_map = format!("0 {} 1", getgid());
    unshare(CloneFlags::CLONE_NEWUSER).context("Failed to isolate user namespace")?;
    write_proc_mapings(&uid_map, &gid_map)?;

    unshare(CloneFlags::CLONE_NEWUTS).context("Failed to isolate uts namespace")?;
    sethostname("my-container")?;

    unshare(CloneFlags::CLONE_NEWPID).context("Failed to isolate pid namespace")?;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child}) => {
            waitpid(child, None)?;
        }
        Ok(ForkResult::Child) => {
            let argv = [CString::new("ls")?; 1];
            child(&container_dir, &root_fs, &argv)?;
        }
        Err(e) => Err(e).context("fork() failed")?
    }

    Ok(())
}

fn child(container_dir: &PathBuf, rootfs: &PathBuf, argv: &[CString]) -> anyhow::Result<()> {
    unshare(CloneFlags::CLONE_NEWUTS).context("Failed to isolate uts namespace")?;
    sethostname("my-container")?;
    unshare(CloneFlags::CLONE_NEWNS).context("Failed to isolate mount namespace")?;
    mount(Some(rootfs), container_dir, None::<&str>, MsFlags::MS_BIND, None::<&str>)?;
    chroot(container_dir).context("Failed to change the root dir")?;
    std::env::set_current_dir("/")?;
    print_proc_info("Container Isolation")?;
    execvp(&argv[0], argv)?;
    Ok(())
}

fn print_proc_info(label: &str) -> anyhow::Result<()> {
    eprintln!("[{}]", label);
    eprintln!(
        "uid [{}]\n\thostname [{:?}] \n\tpid [{}] \n\tcwd [{:?}]",
        getuid(),
        gethostname()?,
        getpid(),
        getcwd()?
    );
    Ok(())
}

fn write_proc_mapings(uid_map: &str, gid_map: &str) -> anyhow::Result<()> {
    std::fs::write("/proc/self/uid_map", uid_map).context("Failed to write to uid")?;

    std::fs::write("/proc/self/setgroups", "deny").context("Failed to write to gid setgroup")?;

    std::fs::write("/proc/self/gid_map", gid_map).context("Failed to write to gid")?;

    Ok(())

}