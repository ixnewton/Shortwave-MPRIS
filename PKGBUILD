# Maintainer: CxOrg <clx.org@cloud-org.uk>
pkgname=shortwave-mpris-git
pkgver=4.0.1.r181.g45bdd3a
pkgrel=2
pkgdesc="Internet radio player with access to over 50,000 stations (with MPRIS support)"
arch=('x86_64' 'aarch64')
url="https://github.com/ixnewton/Shortwave-MPRIS"
license=('GPL3')
depends=(
    'gtk4>=4.16.0'
    'libadwaita>=1.6.0'
    'libshumate>=1.3.0'
    'gstreamer>=1.16.0'
    'gst-plugins-base-libs>=1.16.0'
    'gst-plugins-bad>=1.16.0'
    'gst-plugins-good'
    'gst-libav'
    'ffmpeg>=4.0.0'
    'sqlite>=3.20.0'
    'openssl>=1.0.0'
    'dbus'
    'glib2>=2.66.0'
    'lcms2>=2.12.0'
    'libseccomp>=2.5.0'
)
makedepends=('git' 'rust' 'cargo' 'pkgconf' 'meson' 'ninja' 'blueprint-compiler' 'desktop-file-utils' 'appstream-glib')
provides=('shortwave' 'shortwave-mpris')
conflicts=('shortwave' 'shortwave-mpris')
options=('!lto')
source=("git+$url.git#branch=DLNA-Cast-FFmpeg-AUR")
sha256sums=('SKIP')

pkgver() {
  cd "Shortwave"
  git describe --long --tags | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g'
}

prepare() {
  cd "Shortwave"
  # Set up Rust toolchain
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
  cd "Shortwave"
  
  # Set up Rust environment
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  
  # Build with release profile
  arch-meson . build \
    --buildtype=release \
    -Dprofile=default
  
  ninja -C build
}

check() {
  cd "Shortwave"
  # Run tests if needed
  # meson test -C build --print-errorlogs
}

package() {
  cd "Shortwave"
  DESTDIR="$pkgdir" meson install -C build
  
  # Install license
  install -Dm644 COPYING.md "$pkgdir/usr/share/licenses/$pkgname/COPYING"
}
