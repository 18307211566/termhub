# Contributing

Thanks for helping improve TermHub.

## Development Setup

Requirements:

- Rust stable, with `rustfmt` and `clippy`
- Node.js 18+
- npm 9+
- Tauri CLI v2

```bash
cargo install tauri-cli --version "^2"
cd crates/termhub-app/ui
npm ci
cd ../../..
cargo test --workspace
```

Run the desktop app during development:

```bash
cd crates/termhub-app
cargo tauri dev
```

## Pull Requests

- Keep changes focused.
- Run `cargo fmt --all`.
- Run `cargo clippy --workspace --all-targets -- -D warnings`.
- Run `cargo test --workspace`.
- For UI changes, run `npm run build --prefix crates/termhub-app/ui`.
- Do not commit local config, host keys, logs, build outputs, or IDE settings.

## Security

Do not include real passwords, private host keys, device addresses, or customer logs in issues or pull requests. See `SECURITY.md` for vulnerability reporting.
