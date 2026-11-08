# Guide for AI assistants

This repository holds two independent add-ons for Championship Manager 01/02 3.9.68, both in Rust,
both 32-bit Windows DLLs that patch the running game. Addresses in the docs and code are virtual
addresses in the 3.9.68 `cm0102.exe`. Paths below start at the repository root.

| Path | What |
|---|---|
| `ui/src/` | cmui: `lib.rs` (hooks, drawing), `assistant.rs`, `comp.rs`, `news.rs`, `scout.rs`, `remote.rs`, `prefetch.rs` |
| `ui/docs/INDEX.md` | **start here for cmui**: screen -> code -> how to check; network, testing, switches |
| `ui/FIXES.md` | every fix: symptom, cause, code, check. Grep it first when something looks like an old bug, and add each new fix |
| `ui/README.md` | how the hooks work, in depth |
| `ui/helper_*.md`, `ui/assistant_pick.md`, `ui/scout_stars.md` | reverse-engineering notes: record layouts and game functions |
| `ui/install.py`, `make_exe.py`, `make_patches.py`, `logos.py` | install into a game folder, make the loader exe, make the CM0102Patcher misc patches, make the crest pack |
| `net/src/` | CM Network: a `wsock32.dll` stand-in; `wsock32.def` comes from `make_def.py` |
| `cmui_package/dist/cmui`, `net/package` | what players use. Refresh after a change: build, then copy the DLL and the `ui` scripts in |

## Build

```
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

Run it in `ui` or `net`. It must stay a 32-bit build.

## Rules of the code

- **Check before patching.** Every hook compares the original bytes first and leaves the site alone
  when they differ, because players run exes changed by other patches. `install.py` holds the same
  list of sites: keep the two in step.
- **Each feature has a switch**: a byte in the exe and a name in five lists that must agree:
  `FEATURES` (with a `const NAME: usize = N;`) in `ui/src/lib.rs`, `FEATURES = N` in
  `ui/make_exe.py`, and the name in `ui/make_patches.py`, `ui/install.py` and
  `ui/cmui_config.ps1`; the test `every_switch_list_agrees` (`cargo test`) fails when one is
  missing. Add a row to `ui/docs/switches.md` too (documentation, not a list). A new feature
  gets its own switch and can be turned off alone.
- **Network games are part of every feature.** The host runs all screens and a client only sees what
  is sent, so whatever cmui draws from game memory needs a way to the client (`remote.rs`,
  `ui/docs/network.md`).
- **The game's behaviour stays as it was** unless the feature is meant to change it. Screens may
  look different; results of the game may not.
- **Keep CM Network out of cmui.** They are separate DLLs with separate packages.
- **Do not commit** game files, saves, logos or generated crest packs. Logos may not be shared
  (`cmui_package/LICENCES.md`).
- Do not add names, addresses or machine paths of people to the repository. Use `203.0.113.x` for
  an example address.

## Testing

The game must be closed before a DLL is replaced. Test on a copy of the game folder, not a save
that someone is playing. `ui/docs/testing.md` has the checks; `ui/drive.ps1` and `ui/run.ps1` let
cmui drive the game through its script file. The scripts of the original test rounds are not in
this repository, so the docs mention a few that you will not find here.

Some scripts and docs name drive letters from the machine they were written on (`E:\CM0102`).
Replace them with your own game folder.
