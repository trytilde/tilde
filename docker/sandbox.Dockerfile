# Base image for Tilde sandboxes (E2B, Modal): Debian with the released `tilde` CLI, which the
# sandbox launcher starts as `tilde sandbox connect`. It runs as root, since the process writes
# /etc/gitconfig for egress. Extend it with the tools your agents need.
FROM debian:bookworm-slim
ARG TILDE_VERSION
ARG TARGETARCH
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl git git-lfs gnupg jq less openssh-client procps unzip xz-utils \
    && rm -rf /var/lib/apt/lists/*
# The CLI's release archive, not the server image's `tilde` (a different binary).
RUN test -n "$TILDE_VERSION" \
    && arch="$([ "$TARGETARCH" = arm64 ] && echo arm64 || echo x86_64)" \
    && curl -fsSL "https://github.com/trytilde/tilde/releases/download/v${TILDE_VERSION}/tilde-linux-${arch}.tar.gz" \
        | tar -xz -C /usr/local/bin tilde \
    && tilde --version
WORKDIR /root
CMD ["bash"]
