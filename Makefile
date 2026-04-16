.PHONY: rootfs

# Builds a rootfs tarball via busybox (in docker), you can then unpack in desired location with
# tar -xpf path/to/tar/busybox-rootfs.tar
# For the container to run you also need to create a container dir as well
rootfs:
	docker buildx build \
      --target artifact \
      --output type=local,dest=. \
      -f Dockerfile .

