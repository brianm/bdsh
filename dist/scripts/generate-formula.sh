#!/bin/bash
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
DIST_DIR="${DIST_DIR:-dist/out}"
BASE_URL="https://github.com/brianm/bdsh/releases/download/v${VERSION}"

# Calculate SHA256 for each tarball
sha256_for() {
    local target=$1
    local file="${DIST_DIR}/bdsh-${VERSION}-${target}.tar.gz"
    if [[ -f "$file" ]]; then
        shasum -a 256 "$file" | cut -d' ' -f1
    else
        echo "WARNING: $file not found, using placeholder" >&2
        echo "PLACEHOLDER"
    fi
}

SHA_AARCH64_DARWIN=$(sha256_for aarch64-apple-darwin)
SHA_X86_64_DARWIN=$(sha256_for x86_64-apple-darwin)
SHA_AARCH64_LINUX=$(sha256_for aarch64-unknown-linux-gnu)
SHA_X86_64_LINUX=$(sha256_for x86_64-unknown-linux-gnu)

cat <<EOF
class Bdsh < Formula
  desc "Run commands on multiple hosts via SSH with consensus view"
  homepage "https://github.com/brianm/bdsh"
  version "${VERSION}"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "${BASE_URL}/bdsh-${VERSION}-aarch64-apple-darwin.tar.gz"
      sha256 "${SHA_AARCH64_DARWIN}"
    else
      url "${BASE_URL}/bdsh-${VERSION}-x86_64-apple-darwin.tar.gz"
      sha256 "${SHA_X86_64_DARWIN}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "${BASE_URL}/bdsh-${VERSION}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${SHA_AARCH64_LINUX}"
    else
      url "${BASE_URL}/bdsh-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${SHA_X86_64_LINUX}"
    end
  end

  depends_on "tmux"

  def install
    bin.install "bdsh"
    man1.install "bdsh.1"
  end

  test do
    assert_match "bdsh #{version}", shell_output("#{bin}/bdsh --version")
  end
end
EOF
