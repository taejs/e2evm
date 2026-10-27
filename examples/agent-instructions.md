# Repository instructions for E2E agents

Append the following to your application's `AGENTS.md` or `CLAUDE.md`. Replace the profile and absolute local skill path once; preserve existing project instructions. An installed skill includes the same references as the repository copy.

```markdown
## Browser E2E workspace

For this project's browser E2E tasks, use HarborVM profile `e2e`.
Read `/absolute/path/to/skills/guest-e2e/SKILL.md` and the linked reference relevant to the task.
Invoke the installed `harborvm` CLI from the host; run browsers, test scripts,
development servers, and missing project dependencies inside this guest.

Verify the target URL and source revision before testing. Host localhost and
uncommitted host files are not automatically available in the guest. If the
required target cannot be reached or transferred, report the setup dependency;
do not silently switch to a host browser or a different deployment.

Return assertions, the guest command result, and screenshots when useful.
Preserve unrelated browser tabs and leave the VM running. Use only test data
and accounts authorized for the task. Existing project test commands and
dependency versions still apply.
```

If the agent uses a nondefault `HARBORVM_HOME`, add that exact local directory to your private project instructions. Do not publish personal paths, credentials, or session data in a public repository.
