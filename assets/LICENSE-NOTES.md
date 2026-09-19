# Asset and license notes

This file records what this repository contains and what the available sources say about it, so that
users and rights holders can judge for themselves. It is not legal advice and it is not a license grant.

## 1. Original Mine Bombers game files

Locations: `res/minebomb/`, `res/minebomb.zip`, and the derived forms listed below.

**Owner:** Skitso Productions (Finland). Mine Bombers was shareware (1995-96); version 3.11 was later
released as freeware.

**What the original package itself says** (files inside `res/minebomb/`):

- `FILE_ID.DIZ`: "Mine Bombers 3.11 Registered version / FREEWARE VERSION! MAY BE DISTRIBUTED"
- `MINEENG.TXT`, title box: "REGISTERED FREEWARE"
- `HISTORIA.TXT`, entry dated 31.12.2001: "Rekisteröity versio julkaistu freewarena." (the registered
  version was released as freeware)
- `MINEENG.TXT`, section 1, contradicts the above: "This is the REGISTERED version of MB. So it's against
  the law to copy or distribute it." This looks like text left over from before the freeware release, but
  it is still in the package.

The package contains no license text, no source code and no statement about modification, conversion or
embedding.

**What this repository does with it:**

| Form | Where | Relationship to the original |
|---|---|---|
| Original files | `res/minebomb/` (125 of the 127 files are identical to `res/minebomb.zip`) | Distributed as received |
| Converted audio | `web/audio/*` (14 files) | Derived: sound effects VOC to WAV and music S3M to MP3, both by `scripts/import_classic_assets.cjs` |
| Embedded data | `web/pkg/mb_wasm.wasm` | Derived: original images and font (and any original maps the code embeds) are compiled in with `include_bytes!` in `crates/mb-wasm/src/lib.rs` |

The freeware notice says the package "may be distributed". As far as we can tell it says nothing about
the derived forms above, and the contradictory sentence in `MINEENG.TXT` leaves the distribution question
open even for the unmodified files. Treat the rights in all of this material as unresolved.

**If you are the rights holder or their representative**, please open an issue on this repository. The
material will be removed promptly on request.

### How comparable projects handle original game data

Checked in September 2026:

- `idubrov/mb-reloaded` (the Rust project this one builds on) does not include the game data. Users
  point it at their own copy of Mine Bombers 3.11.
- ScummVM and OpenRA do not bundle proprietary data either. OpenRA offers a stripped-down download or
  imports content the user already owns. ScummVM expects the user's own copy and offers some freeware
  games from its own site (in at least one case with the author's explicit permission).
- OpenTyrian keeps the Tyrian 2.1 data (released as freeware "under no specific license") out of its
  source repository; the data comes as a separate download.
- OpenTTD avoids the question by shipping replacement assets (OpenGFX, OpenSFX, OpenMSX).

Of these projects, none commits the original files, or conversions of them, into its source repository.

## 2. Source code

- `src/` and `crates/mb-core/` are derived from `idubrov/mb-reloaded` by Ivan Dubrov. That repository
  publishes no license (no `LICENSE` file, no license metadata), so by default all rights are reserved
  and this repository cannot relicense that code.
- The "MIT" badge that used to be in the README had no basis and was removed.
- No license has been chosen yet for the code that was written for this repository (`crates/mb-wasm`,
  `web/`, `scripts/`). A license for the whole project depends on what the upstream author allows.

## 3. Bundled third-party binaries

The DLLs and import libraries in the repository root and in `lib/` (SDL2, SDL2_mixer, libgme, libxmp,
libogg, libopus, libopusfile, libwavpack) are committed without their license texts. Their licenses have
not been reviewed yet.

## 4. Placeholder art

The PNG files in `assets/placeholders/` were procedurally generated specifically for this design bundle
from geometric primitives. No image file from Mine Bombers or another third-party game was used as a
source asset. They are development placeholders, not a statement about the licensing of the original
Mine Bombers game or any third-party property.
