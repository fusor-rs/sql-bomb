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

release_version() {
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
  printf '%s\n' "$version"
}

main() {
  action=${1:-}
  if [ "$action" = --archive ]; then
    [ "$#" -eq 2 ] || fail "usage: sh install.sh --archive VERSION"
    shift
  fi
  [ "$#" -le 1 ] || fail "usage: sh install.sh [VERSION]"
  releases="https://github.com/fusor-rs/sql-bomb/releases"
  target=$(release_target)
  version=$(release_version "${1:-}")
  name="sql-bomb-$version-$target"
  archive="$name.tar.gz"
  if [ "$action" = --archive ]; then
    printf '%s\n' "$archive"
    return
  fi
  for tool in curl tar; do
    command -v "$tool" >/dev/null 2>&1 || fail "install $tool before running this installer"
  done
  download_base=${SQL_BOMB_DOWNLOAD_BASE:-$releases/download}
  executable=boom
  destination=${SQL_BOMB_BIN:-${SQL_BOMB_INSTALL:-$HOME/.sqlbomb}/bin/$executable}
  [ ! -d "$destination" ] || fail "$destination is a directory; choose another installation path"
  bin_directory=$(dirname "$destination")
  url="$download_base/v$version/$archive"
  umask 077
  mkdir -p "$bin_directory"
  temporary=$(mktemp -d "$bin_directory/.install.XXXXXX")
  trap 'rm -r "$temporary"' 0
  trap 'exit 1' HUP INT TERM

  printf 'Downloading sql-bomb v%s for %s\n' "$version" "$target"
  curl -fsSL "$url" -o "$temporary/$archive" || fail "cannot download $url"
  curl -fsSL "$url.sha256" -o "$temporary/$archive.sha256" ||
    fail "cannot download $url.sha256"
  verify_archive "$temporary/$archive"
  tar -xzf "$temporary/$archive" -C "$temporary" "$name/$executable"
  replacement="$temporary/$name/$executable"
  [ -f "$replacement" ] && [ ! -L "$replacement" ] ||
    fail "archive has no regular $executable executable"
  chmod 755 "$replacement"
  installed_version=$("$replacement" --version) || fail "downloaded $executable could not run"
  [ "$installed_version" = "$executable $version" ] ||
    fail "downloaded $executable has the wrong version"
  mv -f "$replacement" "$destination"
  printf 'Installed sql-bomb v%s to %s\n' "$version" "$destination"
  [ -z "${SQL_BOMB_BIN:-}" ] || return 0
  case ":$PATH:" in
    *":$bin_directory:"*) ;;
    *) printf 'Add %s to PATH in your shell profile.\n' "$bin_directory" ;;
  esac
}

main "$@"
