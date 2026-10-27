# Repository instructions for E2E agents

Append the following to your application's `AGENTS.md` or `CLAUDE.md`. Replace the profile and absolute local skill path once; preserve existing project instructions. An installed skill includes the same references as the repository copy.

```markdown
## Browser E2E workspace

For this project's browser E2E tasks, use E2EVM profile `e2e`.
Read `/absolute/path/to/skills/guest-e2e/SKILL.md` and the linked reference relevant to the task.
Invoke the installed `e2evm` CLI from the host; run browsers, test scripts,
development servers, and missing project dependencies inside this guest.

Verify the target URL and source revision before testing. Host localhost and
uncommitted host files are not automatically available in the guest. If the
required target cannot be reached or transferred, report the setup dependency;
do not silently switch to a host browser or a different deployment.

Always return assertions, the guest command result, and actual screenshots.
Capture the final state and any failure before closing the test tab. Transfer
images to the host, inspect them, and embed or attach them in the final reply;
paths alone are not a completed delivery. If capture or delivery fails, report
the test outcome separately and mark screenshot evidence blocked with its
reason. Do not report the overall task complete without viewable screenshots.
Preserve unrelated browser tabs and leave the VM running. Use only test data
and accounts authorized for the task. Existing project test commands and
dependency versions still apply.
```

If the agent uses a nondefault `E2EVM_HOME`, add that exact local directory to your private project instructions. Do not publish personal paths, credentials, or session data in a public repository.
