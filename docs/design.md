# Workspace design

The workspace keeps four tasks visible: choosing a machine, using its desktop, running a guest command, and discussing a task with an assistant.

A compact machine list sits on the left. The actual virtual machine display occupies the center; the guest command console starts collapsed and opens below it with **Show console** or **⌘J**. The assistant occupies the right side. The first explicit opening enters native macOS full screen; **⌃⌘F** toggles it. Background agent starts do not enter full screen or activate the window. Graphite surfaces, restrained teal accents, and system typography keep the guest's web content visually distinct from host controls. Machine status and errors stay near the controls they affect.

The design uses original SwiftUI/AppKit layouts and an original local welcome page. No third-party screenshots, icons, fonts, or UI source were copied into the app. System symbols are rendered by macOS.

## References

Two open-source projects informed the workspace concept:

- [edvin/orca](https://github.com/edvin/orca): a Rust/Tauri container desktop, useful as a reference for a machine/resource workspace.
- [andrewmcoupe/orca](https://github.com/andrewmcoupe/orca): a local AI coding workspace, useful as a reference for conversation beside active work.

These projects are references, not dependencies or endorsements. E2EVM uses a native VM display and its own interaction model. [Ollama's API documentation](https://docs.ollama.com/api) informed the connection settings and streaming adapter; E2EVM does not embed or redistribute Ollama's UI.

## Interaction rules

- Show actual lifecycle state and explain failed actions.
- Keep command and conversation history associated with the selected profile.
- Make cancellation available while an assistant response is active.
- Treat assistant output as selectable text, never an automatic command.
- Keep host input separate from guest input. Programmatic clicks target the selected VM view, and no clipboard synchronization is enabled.
- Preserve running guests when the window closes and refuse an accidental app quit while they are active.

Visual and functional evidence belongs in [validation](validation.md). A polished screenshot alone does not establish guest operation or input isolation.
