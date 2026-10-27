# Prompts for your existing agent

Install the needed skill using the [bundled installer](../docs/agents.md), or provide its local `SKILL.md` path in your project instructions. Replace bracketed details before use. `e2evm-setup` handles the first VM setup; the `guest-e2e` prompts use an existing profile.

Every prompt requires actual screenshots in the agent's reply. Capture final and failure states before cleanup, transfer them to the host, inspect them, and embed or attach the images with short captions. Paths alone do not complete delivery. If images cannot be captured or delivered, report the test outcome and the screenshot-evidence blocker separately; do not claim the task is complete.

## First setup

> Use $e2evm-setup to prepare E2EVM from [checkout path] for browser testing. Inspect prerequisites, reuse my dedicated Linux profile [ID] if suitable, and preserve disabled components and other VMs. Run the bundled browser verification, attach its inspected screenshot, and give me the exact CLI path and profile to use next. Leave the VM running.

## Quick smoke

> Use $guest-e2e with profile e2e. Test [URL] in the visible guest browser. Assert [expected title] and [primary visible element]. Return the guest command result and attach an inspected screenshot of the actual result. Keep the VM running.

## Complete user flow

> Use $guest-e2e with profile e2e to test [flow] at [guest-reachable URL], using [authorized test account or disposable fixture]. Follow this project's test instructions. Check [expected outcomes], attach inspected screenshots of the final state and any failure, and report failures with reproduction steps. Capture before cleanup. Clean up only the disposable records created by this run. Keep unrelated tabs and the VM running.

## Missing dependencies

> Use $guest-e2e with profile e2e. Inspect the project at [guest project path] and its lockfiles, then run [test command]. Install the missing dependencies needed for this test inside the guest using its existing package tools. Preserve my disabled setup choices; explain a required conflict before changing them. Use a supervised guest job if an operation will exceed the command timeout. Report installed packages, exact test results, and attach inspected result screenshots. If setup blocks the browser, explain the blocker and whether a diagnostic image could be captured; do not claim the test is complete.

## Verify a local change

> Use $guest-e2e with profile e2e to verify [change]. The source to test is [revision or approved transfer of local changes], and the app should run at [guest-reachable URL]. First confirm that this exact source is running in the guest target. Then check [assertions] and attach inspected screenshots of the actual result and any failure. If local changes are not available in the guest, tell me what transfer or deployment is needed instead of testing the remote default branch.

Agents without `$skill` syntax can replace “Use $guest-e2e” or “Use $e2evm-setup” with “Read and follow [local path to the chosen skill's SKILL.md]”. The rest of the request stays the same.
