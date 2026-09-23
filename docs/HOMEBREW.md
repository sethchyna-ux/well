# Homebrew distribution

Well packages its macOS app for Homebrew as a **Cask**, not a formula. A Cask
installs the prebuilt `Well.app` into the configured Applications directory and
also links its executable as `well`.

The generated release artifact is Apple-silicon (`arm64`) only and requires
macOS Monterey (12) or later.

## Build the release Cask

Preflight the local release machine before spending time on the notarized
build:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./scripts/preflight-macos-release.sh --notarize
```

Build the final, notarized archive and the Cask from the same artifact:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./package-app.sh --notarize --cask
```

This produces these related files:

```text
dist/Well-<version>-macOS-arm64.zip
dist/Well-<version>-macOS-arm64.zip.sha256
dist/Casks/well.rb
```

`well.rb` includes the exact archive SHA-256 and downloads only the immutable
GitHub release asset at `v<version>`. Verify it before promotion:

```bash
./scripts/verify-homebrew-cask.sh dist/Casks/well.rb
```

Do not publish a Cask generated from an ad-hoc local build. The public Cask
must reference the Developer ID-signed and notarized ZIP uploaded to GitHub
Releases.

## Publish through a tap

Create a tap named `sethchyna-ux/homebrew-well` (or another `homebrew-*` tap),
then copy the generated file to `Casks/well.rb` in that tap at the same commit
where the ZIP has been uploaded to the `v<version>` GitHub Release. Users can
then install it with:

```bash
brew tap sethchyna-ux/well
brew install --cask sethchyna-ux/well/well
```

From the root of the tap, run `brew style --cask Casks/well.rb` and
`brew audit --cask --new Casks/well.rb` before opening the promotion PR.

Once the Cask is accepted by the official `homebrew/cask` repository, users can
instead run:

```bash
brew install --cask well
```

The CI macOS packaging job generates and uploads the Cask next to the ZIP and
checksum, so release promotion does not need to reconstruct its SHA-256 by
hand.

## Removal

```bash
brew uninstall --cask well
brew uninstall --zap --cask well
```

`--zap` removes `~/.config/well` and `~/.well`, including generated image
cache. It deliberately does not remove API credentials from the macOS Keychain.
