# Contributing to E2EVM

Thanks for helping agents test without interrupting someone's desktop. Bug reports, documentation, accessibility improvements, and focused code changes are all welcome. You can contribute to the Rust core and guest worker from Linux; you do not need a Mac for every change.

## AI-assisted contributions are welcome

Use the editor, AI assistant, or coding agent that works for you. We do not require an AI-use declaration, a model name, or prompt transcripts. We review the resulting change and its evidence.

A human contributor remains responsible for the submission: read the full diff, understand the decisions, run the relevant checks, and be able to discuss review feedback. An agent's claim that something works is not a test result. Describe what actually ran and any gaps. Translation and writing assistance are welcome too; keep descriptions short, specific, and accurate.

## Pick a change

Search [issues](https://github.com/taejs/e2evm/issues) and [open pull requests](https://github.com/taejs/e2evm/pulls) before starting. Small bug fixes and documentation improvements can go straight to a PR. Discuss a new platform backend, a major workflow change, or automatic agent execution in an issue first so we can agree on the scope.

For a bug report, include the revision, host and guest versions, steps to reproduce, and expected versus actual behavior. Redacted logs or a short recording help. You do not need to have a fix ready. Follow the [security policy](SECURITY.md) for vulnerabilities and the [code of conduct](CODE_OF_CONDUCT.md) when participating.

## Development setup

Read the [architecture](docs/architecture.md) for the Rust, native app, and guest boundaries. For the portable core and worker tests, install current stable Rust and Python 3. Building and running the native app also requires Apple Silicon, macOS 14 or newer, Xcode Command Line Tools, and Node.js 22.15+. The [README](README.md) explains image downloads and first boot.

Use a separate runtime for experiments:

```sh
export E2EVM_HOME="$PWD/.runtime"
export E2EVM_APP="$PWD/dist/E2EVM.app"
cargo build --locked
cargo run -- profile create dev --name "Development VM"
sh scripts/build-macos.sh
./target/debug/e2evm prepare dev
./target/debug/e2evm start dev
```

Use fresh, disposable profiles. Do not test against a personal or production VM, an existing disk, or someone else's runtime. Shut down test guests and confirm the stopped state before rebuilding or replacing their native app. Keep runtime directories, browser data, API keys, assistant transcripts, and private screenshots out of commits. `.runtime`, `target`, and `dist` are ignored.

## Verify a change

For Rust or guest-worker changes, these are the portable CI checks:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/guest_worker.py
```

For native app changes, also run these on Apple Silicon macOS:

```sh
sh platform/macos/tests/run.sh
sh scripts/build-macos.sh
codesign --verify --strict dist/E2EVM.app
```

Rust tests use temporary profiles, executable fixtures, and loopback HTTP servers. They do not call a paid model service or require a real Codex/Ollama account. The Python tests exercise the guest worker using local fixtures. CI also checks shell syntax; for a changed shell script, run `sh -n path/to/script.sh`. Do not run `guest/setup.sh` on your development host: it provisions Linux guests, changes system services, and installs packages.

Match verification to the change. Add a regression test when it protects meaningful behavior; avoid tests that merely repeat the implementation. Documentation-only changes need accurate commands and working links, not a new test suite.

Lifecycle, provisioning, display, and input changes need a disposable guest check as well as automated tests. For UI changes, inspect the actual window at its minimum and default sizes, keyboard focus, and empty/error states; attach a redacted screenshot or recording. For assistant changes, distinguish local fixtures from a live provider test. State what remains unverified if you do not have the needed hardware or service; you can still open a PR for review. CI builds and signs the app but does not boot a VM or prove interactive behavior. See [recorded validation](docs/validation.md) for current coverage and limits.

## Send a pull request

Fork the repository and create a descriptive branch from its current default branch. Keep the PR about one problem and leave unrelated cleanup for another change. Follow nearby code conventions and use clear English for code, comments, and documentation.

Use a title that describes the result, such as `fix: keep guest input inside the VM window`. In the [PR template](.github/PULL_REQUEST_TEMPLATE.md), explain the problem, resulting behavior, and checks you actually ran. Link an issue when one exists; screenshots, commands, or a small reproduction are more useful than a long generated summary. Remove template guidance and sections that do not apply. Review the final diff for private data and unrelated files before submitting.

## Preserve the workspace boundaries

Preserve argument boundaries, bounded untrusted input, profile ownership, and guest-only input handling. Assistant text must remain advisory unless a separate design explicitly defines and reviews a user-approved execution flow. Never add global mouse/keyboard injection or automatic clipboard synchronization as a shortcut.

New dependencies should solve a concrete need. Document their purpose and license in [dependencies](docs/dependencies.md), update `Cargo.lock`, and include any required notices. Do not copy third-party UI assets or source without checking the license and preserving attribution.

Contributions are licensed under the repository's MIT license. Only submit material you have the right to contribute. Report vulnerabilities through the [security policy](SECURITY.md), not a public reproduction containing private data.
