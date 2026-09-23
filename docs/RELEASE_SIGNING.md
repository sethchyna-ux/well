# Release signing and notarization

`package-app.sh` is the canonical macOS release entrypoint. It performs a locked Cargo build, stages a fresh `Well.app`, signs it, and runs bundle verification before publishing the result to `dist/`. A failed build, signature, or verification does not publish a partial application bundle.

The script does not install Well into `/Applications` unless `--install` is explicitly supplied.

## Local developer build

Build an ad-hoc-signed application for local testing:

```bash
./package-app.sh
open dist/Well.app
```

Create the local application plus a versioned, architecture-specific release ZIP and SHA-256 checksum:

```bash
./package-app.sh --archive
```

Generate a Homebrew Cask from the same archive and checksum:

```bash
./package-app.sh --archive --cask
```

The generated Cask is written to `dist/Casks/well.rb`. A Cask intended for
public distribution must be generated from the final Developer ID-signed and
notarized archive; see [Homebrew distribution](HOMEBREW.md).

The release files follow this naming scheme:

```text
dist/Well.app
dist/Well-<version>-macOS-<architecture>.zip
dist/Well-<version>-macOS-<architecture>.zip.sha256
```

For example, a version `0.1.0` build on Apple silicon produces `Well-0.1.0-macOS-arm64.zip` and its adjacent `.sha256` file.

To install the verified bundle locally, opt in explicitly:

```bash
./package-app.sh --install
```

## Bundle metadata

The application version comes from the root Cargo package. Release automation may customize the numeric build number and minimum supported macOS version:

```bash
WELL_BUILD_NUMBER=42 \
WELL_MIN_MACOS_VERSION=13.0 \
./package-app.sh --archive
```

- `WELL_BUILD_NUMBER` sets `CFBundleVersion` and defaults to `1`.
- `WELL_MIN_MACOS_VERSION` sets `LSMinimumSystemVersion` and defaults to the project's supported baseline.

Use the same values for every rebuild of a particular release candidate.

## Developer ID release

Set `WELL_SIGN_IDENTITY` to an installed Apple Developer ID Application certificate. The package script enables hardened runtime and secure timestamping for a non-ad-hoc identity.

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_BUILD_NUMBER=42 \
./package-app.sh --archive
```

Confirm the identity is available before building:

```bash
security find-identity -v -p codesigning
```

Do not use an Apple Development certificate for a release distributed outside the Mac App Store.

## Notarized release

Store notarization credentials once in the local keychain:

```bash
xcrun notarytool store-credentials well-notary \
  --apple-id "developer@example.com" \
  --team-id "TEAMID" \
  --password "app-specific-password"
```

Then provide that keychain profile and a Developer ID identity to the package script:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./package-app.sh --notarize --cask
```

`--notarize` first runs `scripts/preflight-macos-release.sh --notarize` so missing tools, packaging assets, invalid version/build metadata, absent Developer ID certificates, or unusable notarization keychain profiles fail before compilation. It then creates the versioned ZIP, submits it with `notarytool`, waits for acceptance, staples the ticket to `Well.app`, validates the staple, verifies Gatekeeper acceptance, and refreshes the ZIP and checksum so the published archive contains the stapled bundle.

To install the same notarized build after verification:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./package-app.sh --notarize --install
```

Do not commit Apple IDs, app-specific passwords, certificates, provisioning profiles, keychain contents, or exported `.p12` files.

## CI artifacts

The macOS packaging job runs the locked packaging path and publishes the versioned architecture-specific `.zip` together with its `.sha256` file. `Well.app` is contained in the ZIP so its macOS metadata and signature are preserved.

CI packaging verifies archive creation; it does not imply notarization unless the job is explicitly supplied with a Developer ID certificate and notarization credentials.

## Release checklist

1. Start from the intended release commit with a clean worktree and confirm `Cargo.lock` is current and committed.
2. Run the locked quality gates:

   ```bash
   cargo fmt --all -- --check
   cargo check --workspace --locked
   cargo test --workspace --locked
   cargo clippy --workspace --all-targets --locked -- -D warnings
   ./scripts/check-repo-hygiene.sh
   ```

3. Confirm the Developer ID identity and notarization profile are available:

   ```bash
   WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
   WELL_NOTARY_PROFILE=well-notary \
   WELL_BUILD_NUMBER=42 \
   ./scripts/preflight-macos-release.sh --notarize
   ```

4. Build the final notarized artifacts with the release build number:

   ```bash
   WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
   WELL_NOTARY_PROFILE=well-notary \
   WELL_BUILD_NUMBER=42 \
   ./package-app.sh --notarize
   ```

5. Verify the published bundle and checksum:

   ```bash
   ./scripts/verify-macos-bundle.sh dist/Well.app
   (cd dist && shasum -a 256 -c Well-<version>-macOS-<architecture>.zip.sha256)
   xcrun stapler validate dist/Well.app
   spctl --assess --type execute --verbose=4 dist/Well.app
   ```

6. Extract the ZIP on a clean supported Mac, launch Well, open an interactive shell, and exercise input, resize, copy/paste, and clean exit.
7. Publish the `.zip` and matching `.zip.sha256` from `dist/`; keep the checksum beside the release download.
8. Verify and promote `dist/Casks/well.rb` to the Homebrew tap only after the matching notarized ZIP is available at the immutable GitHub Release URL:

   ```bash
   ./scripts/verify-homebrew-cask.sh dist/Casks/well.rb
   ```
