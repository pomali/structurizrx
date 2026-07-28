# This formula is automatically updated by the release workflow when a new tag is pushed.
# To install, first add the tap:
#
#   brew tap pomali/structurizrx https://github.com/pomali/structurizrx
#   brew install structurizrx
class Structurizrx < Formula
  desc "Structurizr DSL toolchain - Rust implementation"
  homepage "https://github.com/pomali/structurizrx"
  version "0.1.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/pomali/structurizrx/releases/download/v0.1.0/structurizrx-aarch64-apple-darwin.tar.gz"
      sha256 "0a44531e94ba0678c45c4587f051ea121c6de1acc0a61057c48b4d8ece7a816f"
    else
      url "https://github.com/pomali/structurizrx/releases/download/v0.1.0/structurizrx-x86_64-apple-darwin.tar.gz"
      sha256 "70e137875a68d630d9b515764441307274510e87eed69b481ec237d3afb0c523"
    end
  end

  on_linux do
    url "https://github.com/pomali/structurizrx/releases/download/v0.1.0/structurizrx-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "1f9e7727f53066d079d8ab2b72b0c7bfe490250758f18338ecae5295db14b826"
  end

  def install
    bin.install "structurizrx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/structurizrx --version")
  end
end
