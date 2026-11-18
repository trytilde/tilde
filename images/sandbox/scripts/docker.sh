#!/bin/sh
# Docker Engine, Buildx and Compose from Docker's apt repository. Sandbox launchers replace the
# image's entrypoint, so nothing starts dockerd at boot: the `docker` wrapper starts it on first use
# (start-dockerd), and `docker-compose` runs the Compose plugin.
set -eu
dir=/usr/local/share/tilde-sandbox
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable" \
    > /etc/apt/sources.list.d/docker.list
apt-get update
apt-get install -y --no-install-recommends \
    containerd.io docker-buildx-plugin docker-ce docker-ce-cli docker-compose-plugin fuse-overlayfs iptables pigz
rm -rf /var/lib/apt/lists/*
# nftables isn't available in every sandbox kernel; the legacy backend works in all of them.
update-alternatives --set iptables /usr/sbin/iptables-legacy
update-alternatives --set ip6tables /usr/sbin/ip6tables-legacy
# The classic storage drivers rather than containerd's snapshotter: overlay2 checks whether it can
# run (not on overlayfs, as in a container's own root filesystem) and falls back to
# fuse-overlayfs, then vfs, where the snapshotter fails to mount layers.
mkdir -p /etc/docker
cat > /etc/docker/daemon.json <<'JSON'
{
  "features": { "containerd-snapshotter": false },
  "log-driver": "json-file",
  "log-opts": { "max-size": "10m", "max-file": "3" }
}
JSON
install -m 0755 "$dir/start-dockerd" /usr/local/bin/start-dockerd
install -m 0755 "$dir/docker" /usr/local/bin/docker
printf '#!/bin/sh\nexec docker compose "$@"\n' > /usr/local/bin/docker-compose
chmod 0755 /usr/local/bin/docker-compose
/usr/bin/docker --version && /usr/bin/docker compose version
