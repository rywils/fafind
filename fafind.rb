class Fafind < Formula
  desc "Fast parallel filesystem search by filename"
  homepage "https://github.com/rywils/fafind"
  version "1.0.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/rywils/fafind/releases/download/v#{version}/fafind-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_x86_64_apple_darwin"
    end

    on_arm do
      url "https://github.com/rywils/fafind/releases/download/v#{version}/fafind-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_aarch64_apple_darwin"
    end
  end

  def install
    bin.install "fafind"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/fafind --version")
  end
end
