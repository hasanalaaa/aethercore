# winget manifests for `aetherctl`

## Status: authored, NOT publishable, and deliberately so

Every field that would make this real is a `PENDING_` placeholder:
`InstallerUrl`, `InstallerSha256`, `ReleaseDate`, `PublisherUrl`.

A winget manifest is a pointer to a **hosted archive** plus its SHA-256. Two of
the things it needs are human-gated in this project and cannot be produced by
an agent:

- a production download endpoint to host the archive at, and
- an Authenticode certificate, because winget's `portable` installer type puts
  an unsigned binary on the user's PATH and SmartScreen will treat it
  accordingly.

So the manifest is written and kept in the repo, and release automation fills
the four placeholders from `out/cli/SHA256SUMS.txt` at publish time. Publishing
it before those two exist would be shipping a broken package.

## Why `zip` + `portable`, and not an MSI

`aetherctl` installs no service, writes nothing outside its own folder and needs
no elevation. A portable zip is exactly that shape, and winget's portable
handling gives the PATH alias for free. Authoring a second MSI purely to carry
one executable would add an install transaction, an ARP entry and an uninstall
path that all have to be qualified, in exchange for nothing the archive does not
already do.

## The archive itself does not need winget

`scripts/build-cli-archive.ps1` produces a self-contained zip. Expanding it and
putting the folder on PATH is the documented one-command install and works on a
machine with no package manager at all. winget is discoverability, not a
dependency — see `docs/CLI_DISTRIBUTION.md`.
