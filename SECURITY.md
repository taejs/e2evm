# Security policy

HarborVM is an early local-development tool. Security fixes target the current development branch; no older release has a promised maintenance window.

## Reporting

Use **Security → Report a vulnerability** on the [GitHub repository](https://github.com/taejs/harborvm/security) when private reporting is available. If it is unavailable, open an issue asking the maintainer for a private reporting channel without including exploit details, credentials, or personal data. No response-time guarantee is offered.

Include the affected revision, host and guest versions, a minimal reproduction using a disposable profile, expected impact, and any proposed fix. Do not upload VM images, browser profiles, access tokens, raw assistant logs, or unrelated host files.

## Trust boundaries

- The guest has a dedicated writable mailbox and a read-only packaged setup kit. Your home directory is not shared with the guest.
- The Linux guest's `harbor` user has passwordless sudo and desktop auto-login. Treat all code run in that guest as able to control the guest and its mailbox.
- The private native control socket checks the host user. Restrictive file permissions and advisory locks reduce accidental cross-profile access; they do not isolate HarborVM from malicious software running as the same host user or root.
- Guest responses and assistant output are untrusted. Guest command transport bounds data and rejects unsafe file paths; responses never become host commands.
- View-scoped input does not post global host events. Clipboard synchronization is disabled.
- Chrome's debugging endpoint listens on guest loopback port 9222. Guest programs can use it to control Chrome and access its browser session. Do not publish or forward that port to an untrusted network.
- VM networking uses Apple's NAT attachment. HarborVM does not add a firewall that blocks access to your LAN or internet.

## Assistant data and credentials

API conversations leave the Mac for the endpoint you select. Localhost endpoints refer to the Mac. Only explicitly submitted conversation content and profile identity are added to requests; the app does not automatically attach guest files, console output, or screenshots.

The desktop holds an entered API key in memory for the current app session. Profiles save connection metadata and, optionally, the name of an environment variable containing a key. The key itself is not saved in profile JSON. API requests require HTTPS except for loopback, reject URL credentials and redirects, and use Bearer authentication when a key is supplied. Review the selected service's own retention and billing policies.

Codex runs as your host user in an ephemeral, read-only session. Its sandbox restricts changes; it is not a VM boundary or a guarantee that host files cannot be read. Select only trusted executables and use Codex's own authentication controls. See [assistant connections](docs/assistant.md) for the exact adapter contract.

## Software and disk safety

Ubuntu boot artifacts are checked against SHA-256 manifests fetched from Ubuntu over HTTPS. This detects corrupt downloads; HarborVM does not independently verify a signed manifest. Guest packages and Chrome are installed from their upstream distributions. The application is locally ad-hoc signed, not notarized or independently security-audited.

Preparation refuses to overwrite an existing profile disk. Profile and inode-based sidecar locks prevent cooperating HarborVM processes from concurrently opening a disk, and Virtualization.framework applies its own image ownership checks. Do not open the same writable disk in another VM tool. A force stop can lose unsaved guest data. Back up important guest data after a clean shutdown.

macOS guest commands run as the signed-in guest account after manual bootstrap. Its share mount service maps ownership to that account; it does not share the host home directory. macOS password prompts cannot be answered through the noninteractive command channel.
