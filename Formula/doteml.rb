# frozen_string_literal: true

class Doteml < Formula
  desc "Convert .eml email files to PDF, PNG, JPG, or HTML"
  homepage "https://github.com/cyberbanksy/.eml"
  version "0.2.3"
  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.3/doteml-aarch64-apple-darwin.tar.xz"
      sha256 "2dfeeb2ddff4f2b5602f30b8c337574328a919c8e1f27ed11b10d50efa88dfc8"
    end
    if Hardware::CPU.intel?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.3/doteml-x86_64-apple-darwin.tar.xz"
      sha256 "e87af05616074ce2917ec10a8181c5f4eb4be9334b5ce4ba89bd3e8cfdca9d1b"
    end
  end
  if OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.3/doteml-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "4e721931637ddb883ab80ca2f836b14878f928a35adf759b6567221d2e2640b7"
    end
    if Hardware::CPU.intel?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.3/doteml-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "a00d099f09680c1f1139a070fde7158295c877d525f68e704d9bd62f2e4a1e6b"
    end
  end
  license "MIT"

  BINARY_ALIASES = {
    "aarch64-apple-darwin":      {},
    "aarch64-unknown-linux-gnu": {},
    "x86_64-apple-darwin":       {},
    "x86_64-pc-windows-gnu":     {},
    "x86_64-unknown-linux-gnu":  {},
  }.freeze

  def target_triple
    cpu = Hardware::CPU.arm? ? "aarch64" : "x86_64"
    os = OS.mac? ? "apple-darwin" : "unknown-linux-gnu"

    "#{cpu}-#{os}"
  end

  def install_binary_aliases!
    BINARY_ALIASES[target_triple.to_sym].each do |source, dests|
      dests.each do |dest|
        bin.install_symlink bin/source.to_s => dest
      end
    end
  end

  def install
    if OS.mac? && Hardware::CPU.arm?
      bin.install "doteml", "eml2html", "eml2jpg", "eml2pdf", "eml2png"
    end
    if OS.mac? && Hardware::CPU.intel?
      bin.install "doteml", "eml2html", "eml2jpg", "eml2pdf", "eml2png"
    end
    if OS.linux? && Hardware::CPU.arm?
      bin.install "doteml", "eml2html", "eml2jpg", "eml2pdf", "eml2png"
    end
    if OS.linux? && Hardware::CPU.intel?
      bin.install "doteml", "eml2html", "eml2jpg", "eml2pdf", "eml2png"
    end

    install_binary_aliases!

    # Homebrew will automatically install these, so we don't need to do that
    doc_files = Dir["README.*", "readme.*", "LICENSE", "LICENSE.*", "CHANGELOG.*"]
    leftover_contents = Dir["*"] - doc_files

    # Install any leftover files in pkgshare; these are probably config or
    # sample files.
    pkgshare.install(*leftover_contents) unless leftover_contents.empty?
  end
end
