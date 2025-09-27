# Maintainer: Your Name <your.email@example.com>
pkgname=shortwave-mpris-git
pkgver=5.0.0.r0.g1ff632d
pkgrel=1
pkgdesc="Internet radio player with access to over 50,000 stations (with MPRIS support)"
arch=('x86_64' 'aarch64')
url="https://gitlab.gnome.org/cxorg/Shortwave"
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
source=("git+$url.git#branch=aur_kde_plasma6")
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
    -Dprofile=default \
    -Doffline=false \
    -Dtests=false
  
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
  install -Dm644 COPYING "$pkgdir/usr/share/licenses/$pkgname/COPYING"
  
  # Install desktop file with MPRIS suffix
  install -Dm644 data/de.haeckerfelix.Shortwave.desktop \
    "$pkgdir/usr/share/applications/de.haeckerfelix.Shortwave.mpris.desktop"
  
  # Install appstream file with MPRIS suffix
  install -Dm644 data/de.haeckerfelix.Shortwave.metainfo.xml \
    "$pkgdir/usr/share/metainfo/de.haeckerfelix.Shortwave.mpris.metainfo.xml"
  
  # Update desktop file to use MPRIS variant
  sed -i 's/Name=Shortwave/Name=Shortwave (MPRIS)/' \
    "$pkgdir/usr/share/applications/de.haeckerfelix.Shortwave.mpris.desktop"
}
