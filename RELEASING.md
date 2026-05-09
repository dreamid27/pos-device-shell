# Releasing device-shell

This document covers the release pipeline: GitHub Actions matrix CI, the Tauri
updater (signing keys + manifest), and Windows code-signing.

## TL;DR — cutting a release

```bash
# 1. Bump version in all three places (must match):
#    package.json                "version"
#    src-tauri/tauri.conf.json   "version"
#    src-tauri/Cargo.toml        "version"

# 2. Commit, tag, push:
git commit -am "release: 0.2.0"
git tag v0.2.0
git push origin master --tags
```

The `release` workflow runs `tauri-action` on each runner in the matrix,
attaches installers + `.sig` files to the GitHub release (auto-published),
then a final job synthesises `latest.json` from the signature files and
uploads it.

## CI workflow

`.github/workflows/release.yml`

| Runner | Outputs |
|---|---|
| `ubuntu-22.04` | `.deb`, `.rpm`, `.AppImage` (+ `.AppImage.sig`) |
| `windows-latest` | `.msi`, `.exe` NSIS (+ `.sig`) |

Signature files (`*.sig`) are produced because the updater plugin is active —
see "Updater" below.

### Required GitHub repo secrets

| Secret | Used by | Source |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | All runners — signs updater bundles | `~/.tauri/device-shell.key` (base64-encoded contents) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | All runners | Passphrase you chose at keygen (empty for no-password key) |
| `WINDOWS_CERTIFICATE_BASE64` | Windows runner | `base64 -w0 your-cert.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | Windows runner | `.pfx` password |
| `APPLE_CERTIFICATE` | macOS runners | `base64 -w0 DeveloperIDApplication.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | macOS runners | `.p12` password |
| `APPLE_SIGNING_IDENTITY` | macOS runners | `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | macOS notarisation | Apple Developer account email |
| `APPLE_PASSWORD` | macOS notarisation | App-specific password from appleid.apple.com |
| `APPLE_TEAM_ID` | macOS notarisation | 10-char team id |

Add a secret: GitHub → repo → Settings → Secrets and variables → Actions → New.

## Updater

### Architecture

```
Build runner → signs bundle → uploads .AppImage/.msi/.app.tar.gz + .sig
                                              ↓
                            publish-latest-json job synthesises latest.json
                                              ↓
              `https://github.com/<repo>/releases/latest/download/latest.json`
                                              ↓
              installed app polls endpoint, verifies sig with embedded pubkey
                                              ↓
              downloads new installer, swaps binary, restarts
```

### Keypair (one-time setup)

```bash
bunx @tauri-apps/cli signer generate --write-keys ~/.tauri/device-shell.key
# → device-shell.key      (PRIVATE — never commit)
# → device-shell.key.pub  (public — embedded in tauri.conf.json)
```

The public key is already pasted into `src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`). To rotate:

1. Generate a new keypair.
2. Replace `pubkey` in `tauri.conf.json` with the new public-key contents.
3. Update GitHub secret `TAURI_SIGNING_PRIVATE_KEY` with the new private key.
4. Existing installs **cannot** be updated to a build signed with the new key
   — they verify against the old pubkey baked in. Plan a key rotation only
   alongside a re-install campaign.

### Setting the private key as a GitHub secret

tauri build expects the key as a single-line base64 string (the whole `.key`
file contents, base64-encoded). Paste the output of:

```bash
base64 -w0 ~/.tauri/device-shell.key
```

into the `TAURI_SIGNING_PRIVATE_KEY` secret on GitHub.

Set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to the passphrase (empty string is
fine for passwordless dev keys).

> Common gotcha: tauri v2 also requires `bundle.createUpdaterArtifacts: true`
> in `tauri.conf.json`. Without it the build skips `.sig` generation entirely
> even with the signing key set, and the latest.json job finds zero `.sig`
> files.

### Updater endpoint

`tauri.conf.json`:
```json
"plugins": {
  "updater": {
    "active": true,
    "endpoints": [
      "https://github.com/posstack/pos-s1/releases/latest/download/latest.json"
    ],
    "dialog": true,
    "pubkey": "<base64 minisign pubkey>"
  }
}
```

Replace the URL if you mirror releases elsewhere (S3, Cloudflare R2, your own
CDN). The default GitHub URL works because `publish-latest-json` uploads
`latest.json` as a release asset.

### `latest.json` schema

```json
{
  "version": "0.2.0",
  "notes": "See release notes on GitHub.",
  "pub_date": "2026-05-09T12:34:56Z",
  "platforms": {
    "linux-x86_64":  { "url": "...AppImage",   "signature": "<base64>" },
    "darwin-x86_64": { "url": "...app.tar.gz", "signature": "<base64>" },
    "darwin-aarch64":{ "url": "...app.tar.gz", "signature": "<base64>" },
    "windows-x86_64":{ "url": "...setup.exe",  "signature": "<base64>" }
  }
}
```

The CI workflow assembles this from the `.sig` files attached to the release.

### In-app update

The status panel exposes a **Check Update** button. Programmatically:

```ts
import { check } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"

const update = await check()
if (update) {
  await update.downloadAndInstall()
  await relaunch()
}
```

`dialog: true` in the plugin config means Tauri also shows a native prompt
when an update is found at startup — change to `false` if you want full
control of the UX from React.

## Windows code-signing (Authenticode)

### Why sign

Without an Authenticode signature, Windows SmartScreen blocks the installer
("Microsoft Defender SmartScreen prevented an unrecognised app from starting").
Users must click "More info → Run anyway" the first time, which most won't
discover. Even with a self-signed cert, SmartScreen reputation only builds up
over time as users run it.

### Cert acquisition

| Type | Cost | SmartScreen reputation | Notes |
|---|---|---|---|
| **OV (standard) Code-Signing** | ~$200–400/yr | Slow build-up (weeks/months of installs) | DigiCert, Sectigo, GlobalSign, SSL.com |
| **EV (Extended Validation)** | ~$400–600/yr | Instant (no SmartScreen warning from day one) | Issued on USB hardware token (or cloud HSM) — slightly more setup |
| **Self-signed** | $0 | Never | Only useful for internal testing — users still see SmartScreen warnings forever |

For a commercial POS deployment, **EV is the practical choice** — instant trust
+ no rep-building period. SSL.com and Sectigo offer cloud-HSM EV that works in
CI without shipping a USB token to GitHub Actions.

### Setup (file-based cert, e.g. OV)

1. Buy cert → vendor delivers `.pfx` (or `.p12`) password-protected.
2. Encode + add as repo secret:
   ```bash
   base64 -w0 cert.pfx | pbcopy   # paste into WINDOWS_CERTIFICATE_BASE64
   ```
3. Add `WINDOWS_CERTIFICATE_PASSWORD` with the `.pfx` password.
4. Push a tag — the Windows runner imports the cert into a temp store, signs
   `device-shell.exe` and the installer with `signtool`, then verifies.

`tauri-action` handles the import/sign internally; no extra workflow steps.

### Setup (EV with cloud HSM)

EV typically requires `signtool` to call a CSP that talks to the HSM. Vendor
provides a CSP DLL; install it on the runner, then point `tauri.bundle.windows`
config at it. The cleanest path on GH Actions is:

1. SSL.com / DigiCert / Sectigo cloud-HSM service.
2. Vendor provides a Windows-side `signtool.exe` wrapper or `eSigner` CLI.
3. Replace `tauri-action` signing env vars with vendor-specific ones — see
   their docs for the exact env-var names; the conf.json `windows` section
   stays the same.

### Verification

After a signed build, on a Windows machine:

```powershell
Get-AuthenticodeSignature 'POS Device Shell_0.2.0_x64-setup.exe'
# Status: Valid  SignerCertificate: <subject>
# TimeStamperCertificate: ...

signtool verify /pa /v 'POS Device Shell_0.2.0_x64-setup.exe'
```

`tauri.conf.json` already configures the timestamp URL
(`http://timestamp.digicert.com`) so signatures remain valid after the cert
expires.

### What NOT to do

- **Do not** commit the `.pfx` to git. Only the base64 in GitHub secrets.
- **Do not** sign with `--no-verify` flags. If verification fails, the bundle
  is broken.
- **Do not** use the same cert across multiple unrelated apps if you can
  avoid it — losing reputation on one drags down the others.

## macOS code-signing + notarisation

Required for any download outside the Mac App Store. Apple Gatekeeper blocks
unsigned apps after macOS 10.15+.

1. Apple Developer Program enrollment (~$99/yr).
2. Create **Developer ID Application** cert in developer.apple.com →
   Certificates → +. Download as `.cer`, import into Keychain, then export as
   `.p12` (with password).
3. Encode: `base64 -i DeveloperIDApplication.p12 | pbcopy` →
   `APPLE_CERTIFICATE` secret.
4. Identity string: `Developer ID Application: Your Name (TEAMID)` →
   `APPLE_SIGNING_IDENTITY`.
5. Generate app-specific password at appleid.apple.com → Sign-In and Security
   → App-Specific Passwords. → `APPLE_PASSWORD`.
6. `APPLE_ID` = your Apple Developer account email. `APPLE_TEAM_ID` is the
   10-char id from developer.apple.com → Membership.

`tauri-action` then:
- Signs `.app` with `codesign`
- Builds `.dmg`, signs it
- Submits to Apple notary service via `notarytool`
- Staples the notarisation ticket so Gatekeeper accepts offline installs

First notarisation can take 5–15 min. Subsequent builds are typically <2 min.

## Smoke testing the pipeline

Without a real cert/key:

1. Set `plugins.updater.active = false` in `tauri.conf.json` temporarily.
2. Push a tag → CI builds installers without signing.
3. Confirm artefacts appear in the draft GitHub release.
4. Re-enable updater + add secrets → tag again.

Always test the auto-update path on a non-production machine first:
1. Install version `N`.
2. Cut release `N+1`, publish.
3. Click **Check Update** in the tray panel.
4. Confirm the app downloads, swaps binary, restarts on the new version.

## Troubleshooting

**`signature verification failed`** at update time → public key in
`tauri.conf.json` doesn't match the private key used to sign. Re-sign with
the correct key, or rebuild with the new pubkey.

**`could not parse latest.json`** → `publish-latest-json` job didn't run, or
URL in `endpoints` is wrong. Check the release page has `latest.json`.

**SmartScreen warning persists with EV cert** → cert is enrolled for OV not EV,
or `signtool` didn't apply it correctly. Verify with
`Get-AuthenticodeSignature` — the cert subject must include `(EV)`.

**Notarisation failed: "The signature of the binary is invalid"** → some
embedded binary (e.g. `standalone`) wasn't signed. Add it to
`bundle.macOS.providerShortName` config or sign manually before bundling.
