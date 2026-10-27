# Contributing to HarborVM

Bug reports, documentation improvements, accessibility fixes, and focused code changes are welcome. Please read the [code of conduct](CODE_OF_CONDUCT.md) and [architecture](docs/architecture.md) first. Discuss a new platform backend or a major workflow change in an issue before building it.

## Development setup

Follow the [README](README.md) to build the native app. The Rust core and fixture tests also run on Linux; actual VM operation needs Apple Silicon and macOS 14 or newer. Use current stable Rust, Xcode Command Line Tools, Node.js 22.15+, and Python 3 for the full development workflow.

Use a separate runtime for experiments:

```sh
export HARBORVM_HOME="$PWD/.runtime"
cargo build --locked
cargo run -- profile create dev --name "Development VM"
sh scripts/build-macos.sh
./target/debug/harborvm prepare dev
./target/debug/harborvm start dev
```

Do not point development tests at an existing machine's disk or reuse someone else's runtime. Shut down test guests and wait for the stopped state before rebuilding or replacing their native app. Keep runtime directories, browser data, API keys, assistant transcripts, and private screenshots out of commits. `.runtime`, `target`, and `dist` are ignored.

## Verify a change

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/guest_worker.py

# On Apple Silicon macOS:
sh platform/macos/tests/run.sh
sh scripts/build-macos.sh
codesign --verify --strict dist/HarborVM.app
```

Rust tests use temporary profiles, executable fixtures, and loopback HTTP servers. They do not call a paid model service or require a real Codex/Ollama account. The Python tests exercise the guest worker using local fixtures. Do not run `guest/setup.sh` on your development host: it is a Linux guest provisioning script that changes system services and installs packages.

For lifecycle, provisioning, or native display changes, also verify a disposable guest locally. For UI changes, inspect the actual window at its minimum and default sizes, check keyboard focus and empty/error states, and attach a redacted screenshot. Record what was tested and what remains unverified. CI compiles and signs the app but does not boot a VM or prove interactive UI behavior.

## Send a pull request

1. Fork the repository and create a descriptive branch.
2. Keep the change focused, with clear English code, comments, and documentation.
3. Add a regression test when it protects meaningful behavior; avoid tests that merely repeat implementation details.
4. Run the relevant checks and inspect your staged diff for private files.
5. Explain the problem, resulting behavior, and validation in the pull request.

Preserve argument boundaries, bounded untrusted input, profile ownership, and guest-only input handling. Assistant text must remain advisory unless a separate design explicitly defines and reviews a user-approved execution flow. Never add global mouse/keyboard injection or automatic clipboard synchronization as a shortcut.

New dependencies should solve a concrete need. Document their purpose and license in [dependencies](docs/dependencies.md), update `Cargo.lock`, and include any required notices. Do not copy third-party UI assets or source without checking the license and preserving attribution.

Contributions are licensed under the repository's MIT license. Only submit material you have the right to contribute. Report vulnerabilities through the [security policy](SECURITY.md), not a public reproduction containing private data.
