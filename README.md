# device-shell

Tauri v2 desktop wrapper for the local POS device service (printers, scales,
RFID readers). The Rust backend exposes the same `localhost:3333` HTTP API the
TypeScript service did, plus a tray-resident GUI.

## Layout

```
apps/device-shell/
├── src/                      # React status panel (small webview)
├── src-tauri/
│   ├── src/lib.rs            # tray, plugins, lifecycle
│   ├── src/device_service/   # axum server (printers/scales/rfid)
│   ├── tauri.conf.json       # bundle + plugins config
│   └── capabilities/         # permission allowlist
└── package.json
```

The single binary serves three subsystems on `:3333`:

| Routes | Subsystem |
|---|---|
| `GET /printers`, `POST /print` | Printers (escpos, zpl, sbpl, system) |
| `GET /scales`, `GET /scales/:n/weight`, `GET /scales/:n/stream` | Scales (nci, cas, toledo, avery, hid-pos, mock) |
| `GET /rfid`, `GET /rfid/:n/stream`, `POST /rfid/:n/write` | RFID (llrp, hid-keyboard, mock) |
| `GET /devices`, `GET /health` | Aggregate / observability |

## Install

### Linux

**Ubuntu / Debian (.deb):**
```bash
sudo apt install ./POS\ Device\ Shell_0.1.0_amd64.deb
# or, with manual dep resolution:
sudo dpkg -i 'POS Device Shell_0.1.0_amd64.deb'
sudo apt-get install -f
```

**Fedora / RHEL (.rpm):**
```bash
sudo dnf install ./POS\ Device\ Shell-0.1.0-1.x86_64.rpm
```

**Portable (.AppImage):**
```bash
chmod +x 'POS Device Shell_0.1.0_amd64.AppImage'
./POS\ Device\ Shell_0.1.0_amd64.AppImage
```

The `.deb` and `.rpm` install:
- `/usr/bin/device-shell` — Tauri tray app (main entry)
- `/usr/bin/standalone` — headless service (for systemd / containers)
- `/usr/share/applications/POS Device Shell.desktop` — launcher entry
- Hicolor icons in `/usr/share/icons/hicolor/`

Required system libraries (auto-pulled by `apt`/`dnf`):
`libwebkit2gtk-4.1-0`, `libgtk-3-0`, `libayatana-appindicator3-1`.

### Windows

Run the `.msi` (or `.exe` NSIS) installer produced by CI on `windows-latest`.
WebView2 runtime is pulled by the bundler if missing.

### macOS

Drag the `.app` from the `.dmg` into `/Applications`. First launch may require
**System Settings → Privacy & Security → Open Anyway** until the build is
notarised.

## Run

After install, launch from your app menu (`POS Device Shell`) or:

```bash
device-shell             # foreground, window visible
device-shell --minimized # straight to tray (used by autostart entry)
```

The service becomes available at `http://localhost:3333` shortly after the
tray icon appears.

**Headless / systemd:** use the bundled `standalone` binary — it runs the same
service without the Tauri webview.

```ini
# /etc/systemd/system/pos-device-service.service
[Unit]
Description=POS Device Service
After=network.target

[Service]
ExecStart=/usr/bin/standalone
Restart=on-failure
User=pos
WorkingDirectory=/etc/pos-device

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now pos-device-service
```

## Configuration

The service looks for `config.json` in its current working directory.

| Bundle | Default cwd |
|---|---|
| `device-shell` (tray) | `~` (user home) |
| `standalone` | wherever you launch from (use `WorkingDirectory=` in systemd) |

Example:

```json
{
  "port": 3333,
  "printers": {
    "EPSON TM-U220": {
      "protocol": "escpos",
      "connection": "system"
    }
  },
  "scales": {
    "checkout-1": {
      "protocol": "nci",
      "transport": "serial",
      "path": "/dev/ttyUSB0",
      "baud": 9600,
      "unit": "kg"
    }
  },
  "rfid": {
    "tag-station": {
      "protocol": "llrp",
      "transport": "tcp",
      "host": "192.168.1.50",
      "port": 5084
    }
  }
}
```

## Verify install

```bash
curl http://localhost:3333/health
# → {"status":"ok","service":"pos-device-service",...}

curl http://localhost:3333/devices
# → {"printers":[…],"scales":[…],"rfid":[…]}
```

## Autostart

Toggle "Launch on system startup" in the tray status panel. Implemented via
`tauri-plugin-autostart`:

- Linux: writes `~/.config/autostart/device-shell.desktop`
- macOS: registers a LaunchAgent in `~/Library/LaunchAgents/`
- Windows: writes the `Run` registry key under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

Autostart entries pass `--minimized` so boot-time launches go straight to the
tray.

## Uninstall

```bash
# Debian/Ubuntu
sudo apt remove pos-device-shell

# Fedora/RHEL
sudo dnf remove pos-device-shell

# AppImage / macOS — just delete the app file/bundle
```

To also drop autostart entry: open the tray panel, untick "Launch on system
startup" before uninstalling, or remove the file/registry entry listed above.

## Develop

Prereqs:
- Rust 1.77+ (`rustup default stable`)
- Bun 1.3+
- Linux dev libs: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev`
- macOS: Xcode CLT
- Windows: WebView2 SDK (auto via tauri build)

```bash
cd apps/device-shell
bun install
bun run tauri dev    # window + service, hot reload on edits
```

The headless dev binary skips the Tauri runtime:

```bash
cargo run --bin standalone
```

## Build installers

```bash
bun run tauri build              # all targets the OS supports
bun run tauri build --bundles deb,rpm,appimage   # subset
```

Cross-compilation is not supported by Tauri — run on each target OS, typically
via the GitHub Actions matrix in `.github/workflows/device-shell-release.yml`.
