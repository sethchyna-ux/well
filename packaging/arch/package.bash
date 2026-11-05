# Appended to the generated PKGBUILD after version and source checksums.
# The binary was built by the harness from this checkout with Cargo.lock.
pkgname=well
pkgrel=1
pkgdesc='Native GPU-rendered terminal workspace'
arch=('x86_64')
depends=('gcc-libs' 'glibc' 'fontconfig' 'ttf-dejavu' 'libx11' 'libxcursor' 'libxi' 'libxrandr' 'libxkbcommon' 'libxkbcommon-x11' 'wayland' 'vulkan-icd-loader')
optdepends=('vulkan-driver: hardware accelerated Vulkan rendering')
options=('!strip' '!debug')
PKGEXT='.pkg.tar.zst'
source=('well' 'org.well.Terminal.desktop' 'README.md' 'well-terminal.md' 'release-readiness.md' 'automation.md')
package() {
    install -Dm755 "$srcdir/well" "$pkgdir/usr/bin/well"
    install -Dm644 "$srcdir/org.well.Terminal.desktop" "$pkgdir/usr/share/applications/org.well.Terminal.desktop"
    install -Dm644 "$srcdir/README.md" "$pkgdir/usr/share/doc/well/README.md"
    install -Dm644 "$srcdir/well-terminal.md" "$pkgdir/usr/share/doc/well/docs/well-terminal.md"
    install -Dm644 "$srcdir/release-readiness.md" "$pkgdir/usr/share/doc/well/docs/release-readiness.md"
    install -Dm644 "$srcdir/automation.md" "$pkgdir/usr/share/doc/well/docs/automation.md"
    if [[ -f $srcdir/LICENSE ]]; then
        install -Dm644 "$srcdir/LICENSE" "$pkgdir/usr/share/licenses/well/LICENSE"
    fi
}
