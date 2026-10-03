#!/bin/sh
# Install or update the Tilde CLI.
#
#   curl -fsSL https://trytilde.ai/install.sh | sh
#
# Environment:
#   TILDE_VERSION      Release to install, e.g. 3.1.0. Defaults to the latest release.
#   TILDE_INSTALL_DIR  Where to put the binary. Defaults to ~/.tilde/bin.
#   TILDE_NO_MODIFY_PATH  Set to 1 to leave your shell profile alone.
#
# Running it again installs over the existing binary, so this is also the update path.

set -eu

REPO="trytilde/tilde"
reset="\033[0m"
blue="\033[34;1m"
dim="\033[2m"
red="\033[31;1m"

info() { printf "${blue}%s${reset} %s\n" "$1" "$2"; }
abort() { printf "${red}error${reset} %s\n" "$1" >&2; exit 1; }

probe_arch() {
  arch="$(uname -m)"
  case "$arch" in
    x86_64 | amd64) ARCH="x86_64" ;;
    aarch64 | arm64) ARCH="arm64" ;;
    *) abort "Architecture $arch is not supported. Build from source: cargo install --git https://github.com/$REPO tilde-cli" ;;
  esac
}

probe_os() {
  os="$(uname -s)"
  case "$os" in
    Darwin) OS="darwin" ;;
    Linux) OS="linux" ;;
    MINGW* | MSYS* | CYGWIN*) abort "Install the Tilde CLI on Windows with 'npm install -g @trytilde/cli' or 'pip install trytilde-cli'" ;;
    *) abort "Operating system $os is not supported by this installer" ;;
  esac
}

need() { command -v "$1" > /dev/null 2>&1 || abort "$1 is required to install the Tilde CLI"; }

# Resolve the release to install and the URL prefix its assets live under.
resolve_version() {
  if [ -n "${TILDE_VERSION:-}" ]; then
    VERSION="${TILDE_VERSION#v}"
    PREFIX="https://github.com/$REPO/releases/download/v$VERSION"
  else
    VERSION="latest"
    PREFIX="https://github.com/$REPO/releases/latest/download"
  fi
}

# Verify the download against the release's checksums file. A release without one is refused
# rather than installed unverified.
verify() {
  file="$1"
  name="$2"
  sums="$DOWNLOAD_DIR/checksums.txt"
  if ! curl -fsSL "$PREFIX/checksums.txt" -o "$sums"; then
    abort "Could not download checksums.txt for this release"
  fi
  expected="$(awk -v name="$name" '$2 == name || $2 == "*" name { print $1 }' "$sums" | head -1)"
  [ -n "$expected" ] || abort "$name is not listed in this release's checksums.txt"
  if command -v sha256sum > /dev/null 2>&1; then
    actual="$(sha256sum "$file" | cut -d' ' -f1)"
  elif command -v shasum > /dev/null 2>&1; then
    actual="$(shasum -a 256 "$file" | cut -d' ' -f1)"
  else
    abort "sha256sum or shasum is required to verify the download"
  fi
  [ "$actual" = "$expected" ] || abort "$name failed checksum verification"
}

install_cli() {
  name="tilde-$OS-$ARCH.tar.gz"
  info "Downloading" "$name ($VERSION)"
  archive="$DOWNLOAD_DIR/$name"
  curl -fsSL --progress-bar "$PREFIX/$name" -o "$archive" ||
    abort "Could not download $name from $PREFIX. Check that the release exists."
  verify "$archive" "$name"
  info "Installing to" "$INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
  tar -C "$DOWNLOAD_DIR" -xzf "$archive" tilde
  # Replace by rename so a running `tilde dev` keeps its own open binary.
  mv -f "$DOWNLOAD_DIR/tilde" "$INSTALL_DIR/tilde"
  chmod 755 "$INSTALL_DIR/tilde"
}

detect_profile() {
  shell="$(basename "/${SHELL:-}")"
  if [ "$shell" = "bash" ]; then
    if [ -f "$HOME/.bashrc" ]; then echo "$HOME/.bashrc"; return; fi
    if [ -f "$HOME/.bash_profile" ]; then echo "$HOME/.bash_profile"; return; fi
  elif [ "$shell" = "zsh" ]; then
    echo "${ZDOTDIR:-$HOME}/.zshrc"; return
  elif [ "$shell" = "fish" ]; then
    echo "$HOME/.config/fish/conf.d/tilde.fish"; return
  fi
  for candidate in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.bash_profile" "${ZDOTDIR:-$HOME}/.zshrc"; do
    if [ -f "$candidate" ]; then echo "$candidate"; return; fi
  done
  if [ -d "$HOME/.config/fish" ]; then echo "$HOME/.config/fish/conf.d/tilde.fish"; fi
}

update_profile() {
  case ":${PATH}:" in
    *":$INSTALL_DIR:"*) return ;;
  esac
  if [ "${TILDE_NO_MODIFY_PATH:-0}" = "1" ]; then
    printf "\nAdd the Tilde CLI to your PATH:\n\n  export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
    return
  fi
  profile="$(detect_profile)"
  if [ -z "$profile" ]; then
    printf "\n${blue}Could not detect a shell profile.${reset} Add this to yours:\n\n  export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
    return
  fi
  if ! grep -q "\.tilde" "$profile" 2> /dev/null; then
    info "Updating profile" "$profile"
    case "$profile" in
      *.fish) printf "\n# Tilde\nfish_add_path %s\n" "$INSTALL_DIR" >> "$profile" ;;
      *) printf "\n# Tilde\nexport PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR" >> "$profile" ;;
    esac
    PROFILE_UPDATED="$profile"
  fi
}

# Everything happens in main so a partially downloaded script cannot half-install the CLI.
main() {
  need curl
  need tar
  probe_arch
  probe_os
  resolve_version
  INSTALL_DIR="${TILDE_INSTALL_DIR:-$HOME/.tilde/bin}"
  DOWNLOAD_DIR="$(mktemp -d)"
  trap 'rm -rf "$DOWNLOAD_DIR"' EXIT INT TERM

  printf "\n${blue}Tilde${reset} ${dim}the open-source agent registry${reset}\n\n"
  install_cli
  update_profile

  installed="$("$INSTALL_DIR/tilde" --version 2> /dev/null || echo tilde)"
  printf "\n${blue}Installed${reset} %s\n" "$installed"
  if [ -n "${PROFILE_UPDATED:-}" ]; then
    printf "\nOpen a new terminal, or run:\n\n  . %s\n" "$PROFILE_UPDATED"
  fi
  printf "\nStart a gateway and your agent with ${blue}tilde dev${reset}, or read https://trytilde.ai/docs/cli\n\n"
}

main
