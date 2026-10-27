# Prompts for your existing agent

These prompts assume the `guest-e2e` skill is installed or its path has been added to your project instructions. Replace bracketed target details before use. The profile must already exist; the skill explains how to check readiness and gather evidence.

## Quick smoke

> Use $guest-e2e with profile e2e. Test [URL] in the visible guest browser. Assert [expected title] and [primary visible element]. Return the guest command result and a screenshot. Keep the VM running.

## Complete user flow

> Use $guest-e2e with profile e2e to test [flow] at [guest-reachable URL], using [authorized test account or disposable fixture]. Follow this project's test instructions. Check [expected outcomes], capture evidence at the final state, and report failures with reproduction steps. Clean up only the disposable records created by this run. Keep unrelated tabs and the VM running.

## Missing dependencies

> Use $guest-e2e with profile e2e. Inspect the project at [guest project path] and its lockfiles, then run [test command]. Install the missing dependencies needed for this test inside the guest using its existing package tools. Preserve my disabled setup choices; explain a required conflict before changing them. Use a supervised guest job if an operation will exceed the command timeout. Report installed packages, exact test results, and artifact locations.

## Verify a local change

> Use $guest-e2e with profile e2e to verify [change]. The source to test is [revision or approved transfer of local changes], and the app should run at [guest-reachable URL]. First confirm that this exact source is running in the guest target. Then check [assertions] and return evidence. If local changes are not available in the guest, tell me what transfer or deployment is needed instead of testing the remote default branch.

Agents without `$skill` syntax can replace “Use $guest-e2e” with “Read and follow [local path to skills/guest-e2e/SKILL.md]”. The rest of the request stays the same.
