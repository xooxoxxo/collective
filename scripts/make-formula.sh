#!/usr/bin/env bash
# Emit the Homebrew formula for a release on stdout.
# Usage: make-formula.sh <version> <artifact-dir>
# <artifact-dir> must contain collective-<version>-<target>.tar.gz.sha256 files.
set -euo pipefail
VERSION="$1"
DIR="$2"

sha() { cut -d' ' -f1 <"$DIR/collective-${VERSION}-$1.tar.gz.sha256"; }
url() { echo "https://github.com/xooxoxxo/collective/releases/download/v${VERSION}/collective-${VERSION}-$1.tar.gz"; }

cat <<EOF
class Collective < Formula
  desc "Searchable directory of developer commands with TUI and flashcards"
  homepage "https://github.com/xooxoxxo/collective"
  version "${VERSION}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "$(url aarch64-apple-darwin)"
      sha256 "$(sha aarch64-apple-darwin)"
    else
      url "$(url x86_64-apple-darwin)"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "$(url aarch64-unknown-linux-gnu)"
      sha256 "$(sha aarch64-unknown-linux-gnu)"
    else
      url "$(url x86_64-unknown-linux-gnu)"
      sha256 "$(sha x86_64-unknown-linux-gnu)"
    end
  end

  def install
    bin.install "collective"
  end

  test do
    assert_match "collective", shell_output("#{bin}/collective --help")
  end
end
EOF
