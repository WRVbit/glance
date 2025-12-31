Name:           glance
Version:        0.1.0
Release:        1%{?dist}
Summary:        A beautiful, modern Linux system optimizer

License:        GPLv3
URL:            https://github.com/WRVbit/glance
Source0:        https://github.com/WRVbit/glance/archive/refs/tags/v%{version}.tar.gz

BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  nodejs
BuildRequires:  npm
BuildRequires:  gtk3-devel
BuildRequires:  webkit2gtk3-devel
BuildRequires:  libappindicator-gtk3-devel

Requires:       gtk3
Requires:       webkit2gtk3
Requires:       libappindicator-gtk3

%description
Glance is a modern Linux system optimizer that combines monitoring, cleaning, and optimization into one beautiful application. Built with Tauri v2 (Rust) and Svelte 5.

%prep
%autosetup

%build
# Set cargo home to local dir to avoid permission issues in mock/copr
export CARGO_HOME="$(pwd)/.cargo"
npm ci
npm run tauri build -- --bundles rpm

%install
install -Dm755 src-tauri/target/release/glance-linuxoptimizer %{buildroot}%{_bindir}/glance
install -Dm644 com.github.WRVbit.glance.desktop %{buildroot}%{_datadir}/applications/com.github.WRVbit.glance.desktop
install -Dm644 src-tauri/icons/icon.png %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/com.github.WRVbit.glance.png

%files
%{_bindir}/glance
%{_datadir}/applications/com.github.WRVbit.glance.desktop
%{_datadir}/icons/hicolor/512x512/apps/com.github.WRVbit.glance.png

%changelog
* Mon Dec 31 2024 WRVbit <your-email@example.com> - 0.1.0-1
- Initial release
