class Wrkr < Formula
  desc "Fast, scriptable load testing tool"
  homepage "https://github.com/nogcio/wrkr"
  version "v0.0.1"
  on_macos do
    on_arm do
      url "https://github.com/nogcio/wrkr/releases/download/#{version}/wrkr-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "5c37eeee68b5651166cb49d754683ef6d1941b1ca9bcc7bfa920b9f5e9ddac04"
    end

    on_intel do
      url "https://github.com/nogcio/wrkr/releases/download/#{version}/wrkr-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "dcf66476d0571be9c95489aa0127d6d6ddf2c4a1ba2ff1d7ed5e6f51e4fd90ea"
    end
  end

  def install
    bin.install "wrkr"
  end

  test do
    system "#{bin}/wrkr", "--help"
  end
end
