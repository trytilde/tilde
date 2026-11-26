# Debian images to try Tilde sandboxes with (E2B, Modal). Each has the `tilde` CLI, which the
# sandbox launcher starts as `tilde sandbox connect`, plus common agent tools:
#   node    Node.js 24 with npm, pnpm and yarn (corepack)
#   python  Python 3 with pip, venv and uv
#   full    node + python + Docker Engine and Docker Compose
# They run as root, since `tilde sandbox connect` writes /etc/gitconfig for egress.
#
# Published as ghcr.io/trytilde/sandbox-debian-<target>. TILDE_VERSION is the release version
# (Changie replaces it with VERSION); the image's tags and its CLI share it.
ARG TILDE_VERSION=5.8.0

FROM debian:bookworm-slim AS base
ARG TILDE_VERSION
ARG TARGETARCH
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl dnsutils fd-find file git git-lfs gnupg htop iproute2 \
        jq less lsof make nano netcat-openbsd openssh-client patch procps ripgrep rsync sqlite3 \
        sudo tree unzip vim-tiny wget xz-utils zip \
    && ln -s /usr/bin/fdfind /usr/local/bin/fd \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
        -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*
# The CLI's release archive, not the server image's `tilde` (a different binary).
RUN arch="$([ "$TARGETARCH" = arm64 ] && echo arm64 || echo x86_64)" \
    && curl -fsSL "https://github.com/trytilde/tilde/releases/download/v${TILDE_VERSION}/tilde-linux-${arch}.tar.gz" \
        | tar -xz -C /usr/local/bin tilde \
    && tilde --version
COPY scripts/ /usr/local/share/tilde-sandbox/
WORKDIR /root
CMD ["bash"]

FROM base AS python
ENV PIP_BREAK_SYSTEM_PACKAGES=1 PIP_ROOT_USER_ACTION=ignore
RUN /usr/local/share/tilde-sandbox/python.sh

FROM base AS node
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
COPY --from=node:24-bookworm-slim /usr/local/bin/node /usr/local/bin/node
COPY --from=node:24-bookworm-slim /usr/local/include/node /usr/local/include/node
COPY --from=node:24-bookworm-slim /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
    && ln -s ../lib/node_modules/corepack/dist/corepack.js /usr/local/bin/corepack \
    && corepack enable \
    && node --version && npm --version

FROM node AS full
ENV PIP_BREAK_SYSTEM_PACKAGES=1 PIP_ROOT_USER_ACTION=ignore
RUN /usr/local/share/tilde-sandbox/python.sh && /usr/local/share/tilde-sandbox/docker.sh
