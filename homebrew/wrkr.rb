class Wrkr < Formula
  desc "Fast, scriptable load testing tool"
  homepage "https://github.com/nogcio/wrkr"
  version "__VERSION__"
  license "AGPL-3.0-only"

  on_macos do
    on_arm do
      url "https://github.com/nogcio/wrkr/releases/download/#{version}/wrkr-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "__SHA256_MAC_ARM64__"
    end

    on_intel do
      url "https://github.com/nogcio/wrkr/releases/download/#{version}/wrkr-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "__SHA256_MAC_X86_64__"
    end
  end

  def install
    bin.install "wrkr"
  end

  test do
    system "#{bin}/wrkr", "--help"
  end
end
