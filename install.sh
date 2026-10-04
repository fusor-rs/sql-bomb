#!/bin/sh
set -eu

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

release_target() {
  system=$(uname -s)
  processor=$(uname -m)
  case "$system" in
    Darwin) platform="apple-darwin" ;;
    Linux) platform="unknown-linux-musl" ;;
    *) fail "unsupported system $system; sql-bomb requires macOS or Linux" ;;
  esac
  case "$processor" in
    x86_64 | amd64) architecture="x86_64" ;;
    arm64 | aarch64) architecture="aarch64" ;;
    *) fail "unsupported processor $processor; use x86-64 or ARM64" ;;
  esac
  printf '%s-%s\n' "$architecture" "$platform"
}

verify_archive() {
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$1")
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$1")
  else
    fail "install sha256sum or shasum to verify the download"
  fi
  expected=$(cut -d ' ' -f 1 "$1.sha256")
  [ "$expected" = "${actual%% *}" ] || fail "checksum mismatch; download the release again"
}

main() {
  [ "$#" -le 1 ] || fail "usage: sh install.sh [VERSION]"
  for tool in curl tar; do
    command -v "$tool" >/dev/null 2>&1 || fail "install $tool before running this installer"
  done
  target=$(release_target)
  releases="https://github.com/fusor-rs/sql-bomb/releases"
  version=${1:-}
  if [ -z "$version" ]; then
    latest=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$releases/latest") ||
      fail "cannot find the latest release; check $releases or specify a version"
    case "$latest" in
      "$releases"/tag/v*) version=${latest##*/} ;;
      *) fail "no release found; check $releases" ;;
    esac
  fi
  version=${version#v}
  case "$version" in
    '' | *[!0-9A-Za-z.+-]*) fail "invalid version; use a release version such as v0.1.0" ;;
  esac
  download_base=${SQL_BOMB_DOWNLOAD_BASE:-$releases/download}
  install_directory=${SQL_BOMB_INSTALL:-$HOME/.sqlbomb}
  executable=boom
  [ ! -d "$install_directory/bin/$executable" ] ||
    fail "$install_directory/bin/$executable is a directory; choose another SQL_BOMB_INSTALL"
  name="sql-bomb-$version-$target"
  archive="$name.tar.gz"
  url="$download_base/v$version/$archive"
  umask 077
  mkdir -p "$install_directory/bin"
  temporary=$(mktemp -d "$install_directory/bin/.install.XXXXXX")
  trap 'rm -r "$temporary"' 0
  trap 'exit 1' HUP INT TERM

  printf 'Downloading sql-bomb v%s for %s\n' "$version" "$target"
  curl -fsSL "$url" -o "$temporary/$archive" || fail "cannot download $url"
  curl -fsSL "$url.sha256" -o "$temporary/$archive.sha256" ||
    fail "cannot download $url.sha256"
  verify_archive "$temporary/$archive"
  tar -xzf "$temporary/$archive" -C "$temporary" "$name/$executable"
  chmod 755 "$temporary/$name/$executable"
  mv -f "$temporary/$name/$executable" "$install_directory/bin/$executable"
  printf 'Installed sql-bomb v%s to %s/bin/%s\n' "$version" "$install_directory" "$executable"
  case ":$PATH:" in
    *":$install_directory/bin:"*) ;;
    *) printf 'Add %s/bin to PATH in your shell profile.\n' "$install_directory" ;;
  esac
}

main "$@"
