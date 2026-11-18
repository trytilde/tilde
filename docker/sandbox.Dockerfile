# A Debian sandbox image to try Tilde sandboxes with (E2B, Modal): the `tilde` CLI, which the
# sandbox launcher starts as `tilde sandbox connect`, plus Node.js, Python and common agent
# tools. Runs as root, since the process writes /etc/gitconfig for egress.
FROM node:24-bookworm-slim
ARG TILDE_VERSION
ARG TARGETARCH
ENV DEBIAN_FRONTEND=noninteractive \
    PIP_BREAK_SYSTEM_PACKAGES=1 \
    PIP_ROOT_USER_ACTION=ignore \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl dnsutils fd-find file git git-lfs gnupg htop iproute2 \
        jq less lsof make nano netcat-openbsd openssh-client patch procps python-is-python3 python3 \
        python3-dev python3-pip python3-venv ripgrep rsync sqlite3 sudo tree unzip vim-tiny wget \
        xz-utils zip \
    && ln -s /usr/bin/fdfind /usr/local/bin/fd \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
        -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*
# uv/uvx for Python projects; pnpm and yarn through corepack.
RUN curl -fsSL https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin UV_NO_MODIFY_PATH=1 sh \
    && corepack enable \
    && corepack prepare pnpm@latest --activate
# The CLI's release archive, not the server image's `tilde` (a different binary).
RUN test -n "$TILDE_VERSION" \
    && arch="$([ "$TARGETARCH" = arm64 ] && echo arm64 || echo x86_64)" \
    && curl -fsSL "https://github.com/trytilde/tilde/releases/download/v${TILDE_VERSION}/tilde-linux-${arch}.tar.gz" \
        | tar -xz -C /usr/local/bin tilde \
    && tilde --version
WORKDIR /root
CMD ["bash"]
