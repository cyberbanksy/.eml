# frozen_string_literal: true

class Doteml < Formula
  desc "Convert .eml email files to PDF, PNG, JPG, or HTML"
  homepage "https://github.com/cyberbanksy/.eml"
  version "0.2.0"
  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.0/doteml-aarch64-apple-darwin.tar.xz"
      sha256 "9c60482d1950ac415eac18f86479b05d7ba49a4a195c341db2fc47782e9879fa"
    end
    if Hardware::CPU.intel?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.0/doteml-x86_64-apple-darwin.tar.xz"
      sha256 "0ecc7cb7148249504d4c5e66efbd5f751ab827d6cf22d836b987995040c18469"
    end
  end
  if OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.0/doteml-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "5ab583a32140ba787c3c6ddb51e2a108fc2e865e242b9c191d67b5d5e034601f"
    end
    if Hardware::CPU.intel?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.0/doteml-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "bcf46b24a2d103d397c19391c8efdac7da2f9ab0f53e8ac30f71170f08421b03"
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
