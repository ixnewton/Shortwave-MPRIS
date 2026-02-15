# Shortwave MPRIS v5.1.0

Binary release of Shortwave with extended MPRIS support, DLNA/UPnP device support, and Google Cast integration.

## Features

- **Extended MPRIS Support**: Full media player control via D-Bus
- **DLNA/UPnP Device Support**: Stream to compatible network devices
- **Google Cast Integration**: Cast to Chromecast and compatible devices
- **FFmpeg Proxy**: Automatic transcoding for incompatible streams

## Installation

### Arch Linux (AUR)

```bash
yay -S shortwave-mpris-bin
```

### Manual Installation

Download and extract the tarball, then see `INSTALL.md` for detailed installation instructions.

## What's Included

- Pre-built x86_64 binary (stripped, 19MB)
- Desktop integration files
- GSettings schemas
- Application icons
- GResource bundle
- License and documentation

## Dependencies

- GTK4 >= 4.18.0
- libadwaita >= 1.8.0
- GStreamer >= 1.24.0 with plugins
- FFmpeg >= 4.0.0
- See INSTALL.md for complete list

## Checksums

SHA256 for `shortwave-mpris-5.1.0-linux-amd64.tar.gz`:
```
c3e7a957cd3c023e354307c2ec531744f4d8beecd46d90d8550a1c32ab90f597
```

All files include SHA256SUMS for verification.

## Changes from Previous Versions

- Built with release optimizations
- Complete binary distribution package
- Includes all necessary runtime resources
- Tested installation on Arch Linux

## Support

- GitHub Issues: https://github.com/ixnewton/Shortwave-MPRIS/issues
- Original Project: https://gitlab.gnome.org/World/Shortwave
