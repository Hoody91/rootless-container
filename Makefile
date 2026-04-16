.PHONY: rootfs check test clippy

all: format check lint

# Builds a rootfs tarball via busybox (in docker), you can then unpack in desired location with
# tar -xpf path/to/tar/busybox-rootfs.tar
# The runtime will create the container directory if needed.
rootfs:
	docker buildx build \
      --target artifact \
      --output type=local,dest=. \
      -f Dockerfile .

check:
	cargo check

test:
	cargo test

lint:
	cargo clippy -- -D warnings

format:
	cargo fmt

