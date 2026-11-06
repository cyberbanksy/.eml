# frozen_string_literal: true

class Doteml < Formula
  desc "Convert .eml email files to PDF, PNG, JPG, or HTML"
  homepage "https://github.com/cyberbanksy/.eml"
  version "0.2.2"
  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.2/doteml-aarch64-apple-darwin.tar.xz"
      sha256 "3e8560eac8fecb1f92c80cb0257e8700b049908114ab8857da773df55812a1d9"
    end
    if Hardware::CPU.intel?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.2/doteml-x86_64-apple-darwin.tar.xz"
      sha256 "fe6c1bf03ec91cb5b084d65ff7867dc6775eead93e7ee898522fef8a78463c27"
    end
  end
  if OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.2/doteml-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "9695b3495e966c4b909abec25198f20c99cb9746a2cc47ab0d6fc78a700929b3"
    end
    if Hardware::CPU.intel?
      url "https://github.com/cyberbanksy/.eml/releases/download/v0.2.2/doteml-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "766e2693b1d1975ccbccf04ffb05a91db103f8db1b54e690496eeed37886c27e"
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
