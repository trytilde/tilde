#!/bin/sh
# Python 3 with pip and venv from Debian, and uv/uvx.
set -eu
apt-get update
apt-get install -y --no-install-recommends python-is-python3 python3 python3-dev python3-pip python3-venv
rm -rf /var/lib/apt/lists/*
curl -fsSL https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin UV_NO_MODIFY_PATH=1 sh
python --version && uv --version
