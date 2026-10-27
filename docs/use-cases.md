# Your agent tests. You keep your desktop.

You ask an agent to check a change, switch to X or another project, and its test browser opens on the desktop you are using. E2EVM gives that browser a separate place to run: a local VM with its own desktop, tools, and browser profile.

Your existing coding agent sends commands into the guest through the CLI. The browser and test commands run there; you can open E2EVM to inspect them. Background `start` and profile selection preserve the current app's focus. Closing the workspace window keeps the guest running. See the [recorded checks](validation.md#real-guest-and-application-checks).

These recipes use the supplied Ubuntu browser environment on an Apple Silicon Mac running macOS 14+. Start with the [copyable setup prompt](../README.md#just-paste-this-prompt) and the [agent kit](agents.md). Replace bracketed details in the prompts. Each recipe uses your existing agent and the steps or tests for your application.

| When you need to… | Give the guest… | Get back… |
| --- | --- | --- |
| Keep working while a headed test runs | A reachable app and a specific flow | Assertions and actual screenshots |
| Check a bug fix end to end | The exact changed revision and regression steps | Evidence that the changed behavior works |
| Test an authenticated flow again | A dedicated guest login and test account | A repeatable account flow without importing your host session |
| Review a UI change | Named screens, viewports, and optional baselines | Captures you can inspect alongside the assertions |
| Repeat a release smoke test | An existing suite and explicit run request | A result for each requested run |
| Run a browser report or download task | Authorized pages, steps, and expected output | Checked guest files and explicitly transferred evidence |

## Keep using your Mac while the browser runs

For a developer who wants to keep coding, read a document, or catch up on X while an agent checks a user flow. The test can open tabs and navigate inside guest Chrome while you continue in your current Mac app.

**Already provided:** a visible guest browser, background VM startup, guest command execution, and instructions for returning inspected screenshots. **You supply:** the app URL reachable from the guest, its expected behavior, and a CLI-capable coding agent.

> Use $guest-e2e with profile [ID] to test [flow] at [guest-reachable URL]. Check [expected outcomes] in the visible guest browser while I keep working on my Mac. Return the assertion results and attach inspected screenshots before cleanup. Leave the VM running.

## Verify the code you just changed

For a developer fixing a validation bug, broken navigation, or another browser interaction. The goal is to exercise the changed app through its UI, then show the result with enough detail to reproduce a failure.

**Already provided:** guest tools and a skill that checks the target revision before testing. **You supply:** the source revision or an approved transfer of unpublished edits, the application's dependencies, and regression assertions. A fresh clone does not include uncommitted changes on your Mac. [Project transfer and dependency guidance](../skills/guest-e2e/references/environment.md) covers that setup.

> Use $guest-e2e with profile [ID] to verify [fix] from [revision or approved local changes] at [URL]. First confirm that source is running. Reproduce [original bug], assert [correct behavior], and check [nearby behavior]. Attach inspected screenshots and report any mismatch between the requested source and the tested app.

**Recorded example:** an OpenCut Classic project-name validation change passed 12 assertions covering project creation, empty-name rejection, the Enter-key guard, trimming, and persistence after reload. That verifies the project-management flow; it does not establish video rendering or export support. [Validation record](validation.md#real-guest-and-application-checks).

## Reuse a dedicated test login

For someone checking an account page, onboarding, or a settings flow across repeated runs. Guest Chrome has its own persistent profile, so an authorized test session can stay with that workspace rather than borrowing your personal Mac browser.

**Already provided:** persistent guest browser storage and a recipe that attaches Playwright to the existing Chrome context. Cookie persistence across a normal VM restart was verified. **You supply:** a dedicated test account and any sign-in or two-factor steps inside the guest. Services can still expire sessions; credentials and host cookies are not imported automatically. See [browser state and connection details](usage.md#browser-and-guest-commands).

> Use $guest-e2e with profile [ID] to test [account flow] at [URL] using its existing authorized test session. If sign-in is needed, pause so I can complete it inside the guest. Check [outcomes], attach inspected screenshots, and clean up only records this test creates.

## Bring back UI evidence

For a developer reviewing a responsive page, modal, or checkout layout. Ask for the exact views and interaction states that matter, then inspect the actual guest captures with the functional checks.

**Already provided:** browser screenshots and a [working capture-and-transfer recipe](../skills/guest-e2e/references/browser-smoke.md), including capture on assertion failure. **You supply:** the app, requested viewports and states, and any existing baseline or visual comparison tool. E2EVM does not include a baseline service or automatically decide whether a visual difference is acceptable. Inspect captures before relying on them; native cached-view capture has [rendering limits](validation.md#boundaries-of-this-evidence).

> Use $guest-e2e with profile [ID] to review [screens] at [URL] using [viewport sizes]. Exercise [states] and run [functional assertions]. Attach inspected screenshots with a caption for each state. Compare against [provided baselines] if available and identify differences for my review.

## Repeat a release smoke test in the same workspace

For a maintainer running the same essential checks before a release or after a deployment. Keep the browser tools and project setup in a named guest, then run the existing suite against the intended deployment each time.

**Already provided:** a persistent workspace, command results, and a [supervised guest-job recipe](../skills/guest-e2e/references/environment.md#jobs-longer-than-60-seconds) for suites that exceed the CLI's 60-second request limit. **You supply:** the suite, target deployment, and each run request. For scheduled runs, use a separately configured scheduler or agent automation; E2EVM has no built-in scheduler. The Mac must remain awake and the guest available.

> Use $guest-e2e with profile [ID] to run [existing smoke suite] against [deployment URL/revision] now. Supervise it inside the guest if it will exceed one command's timeout. Return its actual completion status, failures, and inspected screenshots. Reuse the named workspace for the next requested run.

## Let a browser reporting task run while you do something else

For someone gathering information from authorized pages, downloading a report, or checking a browser-generated file while using another Mac app. Give the agent the pages, fields, and output you want; its browser interactions happen inside the guest.

**Already provided:** a separate Chrome desktop, guest command execution, and Playwright for an existing agent to use. **You supply:** the authorized pages or report flow, any required login, the fields and file properties to check, and an approved transfer method when the result must reach the Mac. This is a recipe built from the guest tools; an agent writes or adapts the actual browser automation. Downloads stay in the guest. The [environment guide](../skills/guest-e2e/references/environment.md) explains access and transfer, and the small screenshot recipe is not a general large-file transport.

> Use the E2EVM CLI with profile [ID] to collect [fields or report] from [authorized URLs]. Read the guest environment guide, run browser automation inside the guest, and keep it in the background while I use my Mac. Check [expected output], attach an inspected screenshot, and transfer [requested files] through [approved method]. Identify any file that could not be delivered.

## Start with one real flow

Install `e2evm-setup` to prepare the environment, then give `guest-e2e` a narrow task with explicit outcomes. The setup smoke test proves the browser connection and evidence path; your application's assertions prove the application. The right-side assistant remains advisory chat. These prompts go to your existing coding agent. [Choose and install the skills](agents.md).

[Share a workflow you tried, or suggest one to explore](https://github.com/taejs/e2evm/issues/new?template=use-case.yml). Include the prompt, setup, and actual result so someone else can try it.
