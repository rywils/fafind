class Fafind < Formula
  desc "Fast parallel filesystem search by filename"
  homepage "https://github.com/rywils/fafind"
  version "1.0.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/rywils/fafind/releases/download/v#{version}/fafind-macos-x86_64-v#{version}.tar.gz"
      sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5"
    end

    on_arm do
      url "https://github.com/rywils/fafind/releases/download/v#{version}/fafind-macos-arm64-v#{version}.tar.gz"
      sha256 "0ad10e59911d3059cc5dfaf2da218adb945740d840617810008971f00f4c2ada"
    end
  end

  def install
    bin.install "fafind"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/fafind --version")
  end
end