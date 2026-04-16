# Small Rootless Container

Based on a video by [Carlo Quick](https://www.youtube.com/watch?v=scwI-xrawP8)

## Prerequisites

- Linux with unprivileged user namespaces enabled
- Rust/Cargo
- Docker or compatible OCI tooling to build the BusyBox rootfs

## What it does

This is a small container-runtime experiment. It creates separate user, UTS, PID, and mount namespaces, bind-mounts a root filesystem, mounts `/proc`, and then execs the command you pass in.

It is useful as a learning project, but it is not a hardened security boundary.

## Build the rootfs

```bash
make rootfs
tar -xpf busybox-rootfs.tar -C /path/to/rootfs
```

## Run

```bash
cargo run -- /path/to/rootfs /path/to/container -- /bin/sh
```

If you omit the command, the program defaults to `ls`.

If you want to use the built-in default rootfs and container paths, pass the command after `--`:

```bash
cargo run -- -- /bin/sh
```

The command line is:

```text
rootless-container [ROOTFS_DIR] [CONTAINER_DIR] [-- COMMAND [ARGS...]]
```

## Development

```bash
make check
make test
make clippy
```

## Notes

- The rootfs should already contain a `/proc` mount point.
- The container directory must be writable and empty enough for a bind mount target.
- This demo assumes the BusyBox rootfs produced by the Dockerfile.
