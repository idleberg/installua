# Fixtures for the overlay examples

`makensis` reads the disk at compile time. `File "assets\icon.ico"` is not a
syntax check — it fails with *"File: no files found"* unless the file is there —
and that is the only thing standing between the overlay's example pairs and
tier 3.

So this directory is where `tests/overlay.rs` writes the combined script and
runs `makensis -WX`. Everything an example references relative to the script
lives here, and nothing else does.

**Adding an `exposed(…)` row whose example touches the disk?** Put the file
here. It should be a real file of its type rather than an empty placeholder: a
fixture that is not what it claims to be is a trap for the first row that uses
`Icon` or `File /r`, which read the bytes rather than just copying them.

Why this matters, and why the golden is not enough: a golden diff answers *"did
the output change?"* and only `makensis` answers *"is the output valid NSIS?"*.
Swap `CreateShortcut`'s link and target and the golden passes forever — it
records what the compiler emits and has no opinion on whether NSIS accepts it.
Phase 6 adds a row per command from a `-CMDHELP` line somebody read, which is
exactly the situation where that distinction earns its keep (§14).
