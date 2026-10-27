# Dependencies and licensing

E2EVM's original source and documentation are covered by the [MIT license](../LICENSE). That license does not relicense guest operating systems, browsers, external model services, or platform frameworks.

## Rust dependencies

`Cargo.lock` records the resolved dependency graph. The direct dependencies in the initial source release are:

| Crate | Purpose | Declared license |
| --- | --- | --- |
| `anyhow` | Contextual errors | MIT OR Apache-2.0 |
| `libc` | File descriptors, locks, signals, and process primitives | MIT OR Apache-2.0 |
| `serde`, `serde_json` | Typed profiles and protocol messages | MIT OR Apache-2.0 |
| `sha2` | Ubuntu artifact SHA-256 verification | MIT OR Apache-2.0 |
| `reqwest` | HTTP model discovery and streaming with rustls TLS | MIT OR Apache-2.0 |
| `tokio` | Async network execution and deadlines | MIT |
| `tempfile` (development) | Isolated test fixtures | MIT OR Apache-2.0 |

The Apple Silicon target's resolved metadata was inspected during documentation preparation. Transitive packages declare permissive licenses including MIT, Apache-2.0, ISC, BSD-3-Clause, Unicode-3.0, Zlib, BSL-1.0, Unlicense, CDLA-Permissive-2.0, and the LLVM exception. Some expressions require combined notices, such as ring's `Apache-2.0 AND ISC`. No package in that inspected graph lacked a license declaration. Metadata inspection is not a substitute for preserving the actual license texts and notices when distributing compiled binaries.

Review the graph after dependency changes:

```sh
cargo metadata --locked --format-version 1 --filter-platform aarch64-apple-darwin
cargo tree --locked
```

Cargo registry packages include their upstream source and license files. A binary release should collect the applicable notices for its exact lockfile and target before publication. This source release does not vendor those dependencies.

## Host requirements

- Rust and Cargo compile the core and CLI.
- Apple's SDK, Swift compiler, AppKit, SwiftUI, and Virtualization.framework build and run the native app under Apple's applicable terms.
- Host utilities `curl`, `tar`, `gzip`, `hdiutil`, and `base64` handle downloads, image preparation, and screenshot decoding.
- Node.js 22.15+ supplies zstd decompression for the Ubuntu kernel. It is required during preparation, not embedded in the app.
- Codex CLI and Ollama are optional, separately installed products. E2EVM neither bundles their executables nor downloads model weights.

## Guest software

Ubuntu ARM64 cloud artifacts are fetched from [Ubuntu cloud images](https://cloud-images.ubuntu.com/resolute/20261016/) and checked against upstream SHA-256 manifests. The guest's apt packages, including the desktop, Python, and Node.js, retain their individual licenses. Package versions can change as Ubuntu repositories receive updates.

Google Chrome is downloaded separately from Google's HTTPS distribution during first-boot setup. Chrome is proprietary software governed by [Google's Chrome terms](https://www.google.com/chrome/terms/); it is not included in this repository or licensed under MIT. Users should review those terms before preparing the browser workstation. The Chrome package is not pinned to one version.

Python Playwright is installed in a guest virtual environment with a version pinned in `guest/setup.sh`. Its upstream project is [Microsoft Playwright for Python](https://github.com/microsoft/playwright-python), licensed Apache-2.0. The setup uses the guest's installed Chrome and does not bundle Playwright browser archives in the repository.

macOS restore images are obtained from Apple or an explicitly selected local Apple IPSW. Apple software is not included in this repository. The [macOS guide](macos.md) describes the applicable local development/testing limits and first-run setup.

## Design references

The [design document](design.md) records the Orca and Ollama references. No reference-project code, branding, or UI assets are redistributed. E2EVM's local welcome page is original. Repository screenshots should show only a disposable guest and non-sensitive demonstration content.

## Demonstration content

The workspace screenshot shows [OpenCut Classic](https://github.com/OpenCut-app/opencut-classic) at revision `cf5e79e919144200294fb9fed22a222592a0aeea`, with a local project-name validation fix, licensed [MIT](licenses/OpenCut-MIT.txt). It records actual Playwright interactions in a disposable Linux guest. It demonstrates project-name validation, saving, and reload persistence; it does not demonstrate timeline rendering or video export. No OpenCut executable is bundled.

Earlier spreadsheet validation used [Wafflebase](https://github.com/wafflebase/wafflebase) at revision `225b258c1025a86b1466d84b3e13915c0ce4173c` under [Apache-2.0](licenses/Wafflebase-Apache-2.0.txt). Those checks used its local in-memory development harness without the collaboration backend.
