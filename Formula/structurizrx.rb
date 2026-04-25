# This formula is automatically updated by the release workflow when a new tag is pushed.
# To install, first add the tap:
#
#   brew tap pomali/structurizrx https://github.com/pomali/structurizrx
#   brew install structurizrx
#
# A GitHub release must exist before installing. Create one by pushing a version tag:
#
#   git tag v0.1.0 && git push origin v0.1.0
class Structurizrx < Formula
  desc "Structurizr DSL toolchain - Rust implementation"
  homepage "https://github.com/pomali/structurizrx"
  version "0.0.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/pomali/structurizrx/releases/download/v0.0.0/structurizrx-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    else
      url "https://github.com/pomali/structurizrx/releases/download/v0.0.0/structurizrx-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    url "https://github.com/pomali/structurizrx/releases/download/v0.0.0/structurizrx-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  def install
    bin.install "structurizrx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/structurizrx --version")
  end
end
