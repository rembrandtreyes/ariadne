class Ariadne < Formula
  desc "The thread through the labyrinth — universal dependency graph for AI agents"
  homepage "https://github.com/loremllc/ariadne"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/loremllc/ariadne/releases/download/v#{version}/ariadne-aarch64-apple-darwin.tar.gz"
      sha256 "TBD"
    else
      url "https://github.com/loremllc/ariadne/releases/download/v#{version}/ariadne-x86_64-apple-darwin.tar.gz"
      sha256 "TBD"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/loremllc/ariadne/releases/download/v#{version}/ariadne-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "TBD"
    else
      url "https://github.com/loremllc/ariadne/releases/download/v#{version}/ariadne-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "TBD"
    end
  end

  def install
    bin.install "ariadne"
  end

  test do
    assert_match "ariadne", shell_output("#{bin}/ariadne --version")
  end
end
