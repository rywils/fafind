# Maintainer: Ryan Wilson <ryan@ryanwilson.io>
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
sha256sums_x86_64=('a4221717c6f9ef1debcefecf52dd4288a2183e206dce41dfb6906aa7685e4072')
sha256sums_aarch64=('54c1d01b215745dc1692611878acf2743b94e272ba4b1b5bee29904bcae41135')

package() {
    install -Dm755 fafind "${pkgdir}/usr/bin/fafind"
}
