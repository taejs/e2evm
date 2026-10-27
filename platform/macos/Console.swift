import SwiftUI

struct GuestConsole: View {
    @ObservedObject var model: WorkspaceModel
    let profile: MachineProfile
    @ObservedObject var session: ProfileSession
    let running: Bool
    @FocusState private var commandFocused: Bool
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "terminal").foregroundStyle(HarborTheme.accent)
                Text("Guest console").font(.system(size: 11, weight: .semibold))
                Text("/bin/sh · single command").font(.system(size: 10)).foregroundStyle(HarborTheme.muted)
                Spacer(minLength: 4)
                Menu {
                    ForEach(Array(session.commands.enumerated().reversed()), id: \.offset) { _, command in
                        Button(String(command.prefix(100))) { session.command = command; commandFocused = true }
                    }
                } label: { Image(systemName: "clock.arrow.circlepath") }
                    .menuStyle(.borderlessButton).frame(width: 18).disabled(session.commands.isEmpty).help("Command history")
                Button { session.console = "" } label: { Image(systemName: "clear") }
                    .buttonStyle(.plain).disabled(session.commandBusy || session.console.isEmpty).help("Clear console output")
            }.padding(.horizontal, 15).padding(.vertical, 11).background(HarborTheme.panel)
            Divider()
            ScrollViewReader { proxy in
                ScrollView([.horizontal, .vertical]) {
                    VStack(alignment: .leading, spacing: 12) {
                        if session.console.isEmpty {
                            if running && profile.guestOS == "macos" && !session.guestOnline {
                                Text("Finish macOS setup, then run in the guest's Terminal:\nsudo mkdir -p /Volumes/HarborKit\nsudo /sbin/mount_virtiofs -r -u \"$(id -u)\" -g \"$(id -g)\" HarborKit /Volumes/HarborKit\n/bin/sh /Volumes/HarborKit/setup-macos.sh\n\nThe installer explains any guest-only Python or Homebrew prerequisites.")
                                    .foregroundStyle(HarborTheme.muted).textSelection(.enabled)
                            } else {
                                Text("Commands run inside \(profile.name).\nEach command starts a fresh shell; this is not an interactive terminal.")
                                .foregroundStyle(HarborTheme.muted)
                            }
                        } else { Text(session.console).foregroundStyle(Color(white: 0.84)).textSelection(.enabled) }
                        if session.commandBusy {
                            HStack(spacing: 8) {
                                ProgressView().controlSize(.mini)
                                Text("Running in guest… · 60 s limit").foregroundStyle(HarborTheme.muted)
                            }
                        }
                        Color.clear.frame(height: 1).id("console-end")
                    }.font(.system(size: 11, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading).padding(15)
                }.onChange(of: session.console) { _, _ in proxy.scrollTo("console-end", anchor: .bottom) }
            }
            Divider()
            HStack(spacing: 10) {
                Text("$").font(.system(size: 13, weight: .medium, design: .monospaced)).foregroundStyle(HarborTheme.accent)
                TextField(running ? (session.guestOnline ? "Run a guest command…" : "Waiting for guest worker…") : "Start the machine to run commands", text: $session.command)
                    .textFieldStyle(.plain).font(.system(size: 12, design: .monospaced)).focused($commandFocused)
                    .onSubmit { if running && session.guestOnline { model.runCommand(profile.id) } }
                    .disabled(!running || !session.guestOnline || session.commandBusy)
                Button { model.runCommand(profile.id) } label: { Image(systemName: "arrow.turn.down.left") }
                    .buttonStyle(.plain).foregroundStyle(HarborTheme.accent).help("Run command in guest")
                    .disabled(!running || !session.guestOnline || session.commandBusy || session.command.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }.padding(.horizontal, 15).padding(.vertical, 12)
        }.background(HarborTheme.canvas)
    }
}
