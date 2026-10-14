# This formula is automatically updated by the release workflow when a new tag is pushed.
# To install, first add the tap:
#
#   brew tap pomali/structurizrx https://github.com/pomali/structurizrx
#   brew install pomali/structurizrx/structurizrx
class Structurizrx < Formula
  desc "Structurizr DSL toolchain - Rust implementation"
  homepage "https://github.com/pomali/structurizrx"
  version "0.3.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/pomali/structurizrx/releases/download/v0.3.0/structurizrx-aarch64-apple-darwin.tar.gz"
      sha256 "6555d5b85077281104869c532ad760ed186897cb4bfba06d7a446f26fb264ff9"
    else
      url "https://github.com/pomali/structurizrx/releases/download/v0.3.0/structurizrx-x86_64-apple-darwin.tar.gz"
      sha256 "6dfac90d2bd5d89b25d4e69140fdae70f7be959a56c097e5914efae7d9a23ee4"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/pomali/structurizrx/releases/download/v0.3.0/structurizrx-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "fc043c66b31a5d1c4fd9819683075a4c31b10ee9eea8aa25175aa3c40f454c2c"
    else
      url "https://github.com/pomali/structurizrx/releases/download/v0.3.0/structurizrx-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "a6d41206a42aa80e3f413a30661422cc34045336829242d6db9866c7bac4e9a3"
    end
  end

  def install
    bin.install "structurizrx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/structurizrx --version")
  end
end
