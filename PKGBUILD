# Maintainer: Your Name <you@example.com>
pkgname=fafind-bin
pkgver=1.0.0
pkgrel=1
pkgdesc="Fast parallel filesystem search by filename"
arch=('x86_64' 'aarch64')
url="https://github.com/rywils/fafind"
license=('MIT')
provides=('fafind')
conflicts=('fafind')
source_x86_64=("fafind-x86_64-unknown-linux-gnu-${pkgver}.tar.gz::https://github.com/rywils/fafind/releases/download/v${pkgver}/fafind-x86_64-unknown-linux-gnu.tar.gz")
source_aarch64=("fafind-aarch64-unknown-linux-gnu-${pkgver}.tar.gz::https://github.com/rywils/fafind/releases/download/v${pkgver}/fafind-aarch64-unknown-linux-gnu.tar.gz")
# Update these after each release:
#   curl -sL <url> | sha256sum
sha256sums_x86_64=('SKIP')
sha256sums_aarch64=('SKIP')

package() {
    install -Dm755 fafind "${pkgdir}/usr/bin/fafind"
}
