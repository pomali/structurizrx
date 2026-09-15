# This formula is automatically updated by the release workflow when a new tag is pushed.
# To install, first add the tap:
#
#   brew tap pomali/structurizrx https://github.com/pomali/structurizrx
#   brew install pomali/structurizrx/structurizrx
class Structurizrx < Formula
  desc "Structurizr DSL toolchain - Rust implementation"
  homepage "https://github.com/pomali/structurizrx"
  version "0.2.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/pomali/structurizrx/releases/download/v0.2.0/structurizrx-aarch64-apple-darwin.tar.gz"
      sha256 "312019941935480c670b0f199cecb561b778bad737e53461b3a1f0c8924149b9"
    else
      url "https://github.com/pomali/structurizrx/releases/download/v0.2.0/structurizrx-x86_64-apple-darwin.tar.gz"
      sha256 "e79b1b8627ad4ee4c9c9ced204cffd33514122833f71a1e3eba41402545d4658"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/pomali/structurizrx/releases/download/v0.2.0/structurizrx-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "9d049bbebf2520278eef76a03dfcc1bb03451041c014a8e6fb3e095b50ea37b3"
    else
      url "https://github.com/pomali/structurizrx/releases/download/v0.2.0/structurizrx-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "1cf6155a3ad8bf391a41e67aff7656766cc16015ec6f4bdc4361b121b489231c"
    end
  end

  def install
    bin.install "structurizrx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/structurizrx --version")
  end
end
