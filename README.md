# Glance

> See everything, Optimize Anything

Just made it out because I was bored, not many people will use it anyway.

## Description

Glance (formerly Linux Optimizer) is a modern system utility for Linux, built with **Tauri v2** (Rust) and **Svelte 5**. It combines system monitoring, cleaning, and optimization tools into a unified, beautiful interface with a glassmorphism design.

## Features

- **📊 Dashboard**: Real-time system statistics (CPU, RAM, Disk).
- **📈 Resource Monitor**: Live history charts for CPU, Memory, and Network traffic.
- **🧹 System Cleaner**: Clean trash, caches, logs, and old kernels safe and easily.
- **⚙️ Performance Tweaks**: Apply recommended system settings (BBR, Swappiness, etc.).
- **📦 Repository Manager**: Manage APT sources, PPAs, and find the fastest mirrors.
- **🌐 Hosts Editor**: Edit `/etc/hosts` and import ad-blocking lists.
- **🔧 Services**: Manage systemd services.
- **📦 Packages**: Search and remove installed packages.
- **🚀 Startup**: Manage autostart applications.

## Installation

### Prerequisites
- Ubuntu 24.04+ or Debian 12+
- `libwebkit2gtk-4.1-0`, `libgtk-3-0`, `curl`, `pkexec`

### Build from Source
```bash
git clone https://github.com/yourusername/glance.git
cd glance
npm install
npm run tauri build
```

The .deb package will be in `src-tauri/target/release/bundle/deb/`.

## License

MIT
