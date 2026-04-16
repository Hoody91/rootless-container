FROM alpine:3.21 AS builder

RUN apk add --no-cache \
    build-base \
    linux-headers \
    wget \
    tar \
    bzip2 \
    ca-certificates

ARG BUSYBOX_VERSION=1.36.1

WORKDIR /src

RUN wget https://busybox.net/downloads/busybox-${BUSYBOX_VERSION}.tar.bz2 \
    && tar -xjf busybox-${BUSYBOX_VERSION}.tar.bz2 \
    && rm -f busybox-${BUSYBOX_VERSION}.tar.bz2 \
    && mv busybox-${BUSYBOX_VERSION} busybox

WORKDIR /src/busybox

RUN make defconfig \
    && sed -i 's/^# CONFIG_STATIC is not set/CONFIG_STATIC=y/' .config \
    && make -j"$(nproc)" \
    && make CONFIG_PREFIX=/rootfs install

RUN mkdir -p \
      /rootfs/dev \
      /rootfs/etc \
      /rootfs/proc \
      /rootfs/sys \
      /rootfs/tmp \
      /rootfs/root \
    && chmod 1777 /rootfs/tmp \
    && printf 'root:x:0:0:root:/root:/bin/sh\n' > /rootfs/etc/passwd \
    && printf 'root:x:0:\n' > /rootfs/etc/group

RUN tar -C /rootfs -cpf /busybox-rootfs.tar .

FROM scratch AS artifact
COPY --from=builder /busybox-rootfs.tar /busybox-rootfs.tar
