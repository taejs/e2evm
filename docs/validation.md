# Validation record

Recorded on 2026-10-27 on Apple Silicon. This is functional verification of the tested configuration, not a security certification or a guarantee for every guest, provider, or website.

## Automated checks

- Rust formatting and all-target Clippy with warnings denied passed.
- 30 Rust tests passed: 9 preparation recovery cases, 6 local-assistant cases, 9 HTTP-provider cases, 6 profile/filesystem cases.
- 6 guest-worker tests passed, including a child that closes output but continues running past its deadline and a detached child retaining stdout. The detached-output regression failed before the fix at 4.041 seconds for a 1-second limit, then passed with a bounded drain and successful following command.
- Native ownership, safe paths, lifecycle transitions and keyboard mapping checks passed. The shutdown harness rejected new socket/UI work while an existing job completed cancellation; removing the admission guards reproduced the regression.
- Preparation recovery tests cover 18 injected interruption boundaries across Linux and macOS, metadata lock contention, foreign destinations, symlinks, changed files and incomplete installs without a journal.
- Stale desktop requests for API server A were rejected after switching the profile to B; a listening B endpoint received no connection, key or conversation. Matching connection snapshots retained the existing provider and Codex test behavior.
- Whole-app Swift typechecking with warnings treated as errors passed; signed native app build passed.
- macOS preparation helper rejected nine invalid/unsafe invocation cases. The guest bootstrap rejected execution on the physical host before mutation.
- Skill frontmatter, linked files, installer overwrite refusal, shell syntax and runnable Python example checks passed.

## Real guest and application checks

| Check | Observed result |
| --- | --- |
| Fresh Ubuntu boot | Actual native VM display and guest command worker available |
| Default browser setup | Chrome 156.0.8078.47, Python 3.14.4, Node 22.22.1, Python Playwright 1.64.1 installed |
| Guest command transport | Unicode argument returned correctly; failures report nonzero status |
| Browser persistence | A disposable cookie survived graceful VM stop and restart |
| Native input isolation | Click, Ctrl+A and ASCII text produced the exact expected DOM input value; host pointer coordinates and clipboard change count stayed identical |
| Background start and selection | Starting a stopped validation VM launched the native app behind the current app; starting and selecting preserved the foreground app PID/bundle, pointer coordinates and clipboard change count |
| Lifecycle | Pause/resume, pending graceful shutdown, actual stop, refusal to quit with an active VM, and close/reopen while retaining the VM verified |
| Stopped-app quit | Native process exited successfully in 0.02 seconds after the termination-loop fix |
| Minimal Linux setup | Fresh `--without all` guest completed bootstrap; desktop, Chrome, Python development packages, Node/npm and venv absent; core command transport worked |
| macOS installation | Fresh 64 GiB disk installed from a cached public Apple 25G83 restore image; native state running and Setup Assistant visibly rendered |
| Agent skill smoke | Two assertions passed against a local HTML fixture; actual JPEG returned through the bounded guest result channel and inspected |
| Real OpenCut Classic flow | On revision `cf5e79e919144200294fb9fed22a222592a0aeea`, an actual project-name validation change passed 12 assertions: new project, empty-name rejection, Enter guard, trimmed valid name, and persistence after reload. Two repeat runs and a third run during native-window recording passed. |
| Supervised guest job | Launch returned in 0.112 seconds; 3-second job subsequently reported successful completion and expected journal output |
| Codex connection | Existing local CLI sign-in produced a real assistant response |

## Boundaries of this evidence

macOS was verified through its visible Setup Assistant. A new guest account and its Python/Homebrew connection bootstrap have not been completed in the test image. Linux paths and package commands must not be applied to macOS. The user must complete first-run setup inside the guest.

Ollama and OpenAI-compatible discovery/streaming were tested against deterministic local HTTP fixtures, including cancellation, credential redaction, redirects and size limits. No live Ollama model or paid compatible service was used. The assistant displays replies; it does not execute them.

The browser smoke fixture verifies the guest integration, not an application's business flow. Actual application tests need their own assertions and evidence. GPU-heavy applications can need additional graphics support: OpenCut Classic's editor rendered with an experimental software WebGPU configuration, but its text preview encountered an upstream graphics error. Do not interpret project-management test success as video-rendering or export support.

Native cached-view screenshots successfully captured the actual Linux browser and macOS Setup Assistant in the tested app. Rendering may differ by guest or GPU-backed application; inspect results and fail visibly if a capture is incomplete. Keyboard typing uses the supported US-layout scan codes; Unicode belongs in the guest command channel.

Public screenshots and recordings use disposable test state. No existing production VM disk, account, or worker was used as the test machine.

## Release review

Review `20261027-release-6ac815b0` covered correctness, tests, maintainability, agent integration, security, API contracts, reliability, Swift and adversarial cases. Five validated findings were fixed and received focused follow-up inspection. A different-provider review CLI was unavailable, so the adversarial pass used the available agent route. Hosted CI and final public release identity are still pending.
