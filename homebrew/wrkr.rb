class Wrkr < Formula
  desc "Fast, scriptable load testing tool"
  homepage "https://github.com/nogcio/wrkr"
  version "v0.0.2"
  on_macos do
    on_arm do
      url "https://github.com/nogcio/wrkr/releases/download/#{version}/wrkr-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "265b880a5d0b58fcfe5a7c1af24c12777d4fd3fd91766d581be98efd21a2a243"
    end

    on_intel do
      url "https://github.com/nogcio/wrkr/releases/download/#{version}/wrkr-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "29b2d8200d8faabbb1b7ddc51a7cfddedc820c551544f2901430a4853788fe82"
    end
  end

  depends_on "luajit"
  depends_on "protobuf"

  def install
    bin.install "wrkr"
  end

  test do
    system "#{bin}/wrkr", "--help"
  end
end
