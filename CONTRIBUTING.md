# Contributing

Thanks for considering a contribution!

## Getting started

```
rustup show        # stable toolchain, rustfmt + clippy come via rust-toolchain.toml
cargo build
cargo test
```

The project targets Windows 10/11; develop and test on Windows, since the
tray, event-trigger and adb integration are Windows-specific.

## Code quality gate

CI runs these on every PR — run them locally first:

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

## Project layout

| Path | Purpose |
|---|---|
| `src/main.rs` | entry point, mode dispatch (CLI / tray) |
| `src/config.rs` | PS-compatible `config.json` load/save/autocreate |
| `src/adb.rs` | adb.exe discovery + devices/connect/pair/mdns wrappers |
| `src/scanner.rs` | async ephemeral-range TCP port scanner |
| `src/tray.rs` | tray icon, menu, message loop |

## Commits & PRs

- Short imperative subject line (`Stage 2: adb layer + port scanner`).
- Keep PRs small and focused; describe what and why.
- New behavior ships with tests; parser and selection logic is unit-tested,
  live-connection flows are verified against a real device.
- Update README/CHANGELOG when user-visible behavior changes.

## Reporting bugs

Use the issue templates — include Windows build, phone model and adb version.
Security issues: see [SECURITY.md](SECURITY.md), never public issues.
