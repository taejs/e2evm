import SwiftUI

struct AssistantPanel: View {
    @ObservedObject var model: WorkspaceModel
    let profile: MachineProfile
    @ObservedObject var session: ProfileSession
    @State private var showSettings = false
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "bubble.left.and.bubble.right").font(.system(size: 16)).foregroundStyle(HarborTheme.accent)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Assistant").font(.system(size: 14, weight: .semibold))
                    Text("LOCAL CODEX CLI · HOST").font(.system(size: 9, weight: .medium)).tracking(0.8).foregroundStyle(HarborTheme.muted)
                }
                Spacer()
                Menu {
                    Button("Configure Codex…") { model.errorMessage = nil; showSettings = true }
                    Button("Clear conversation") { session.messages = [] }.disabled(session.chatBusy || session.messages.isEmpty)
                } label: { Image(systemName: "ellipsis") }.menuStyle(.borderlessButton).frame(width: 20).help("Assistant options")
            }.padding(.horizontal, 19).padding(.vertical, 22)
            Divider()
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        if session.messages.isEmpty {
                            VStack(alignment: .leading, spacing: 14) {
                                Text("Think beside your machine.").font(.system(size: 17, weight: .medium))
                                Text("Ask about Linux, plan your setup, or work through a command before you run it.")
                                    .font(.system(size: 12)).foregroundStyle(HarborTheme.muted).lineSpacing(4)
                                Button { session.prompt = "Suggest a few read-only commands to inspect a fresh Ubuntu machine and explain what each does." } label: {
                                    HStack(alignment: .top) {
                                        Text("How do I inspect this machine?").multilineTextAlignment(.leading)
                                        Spacer(minLength: 4)
                                        Image(systemName: "arrow.down.left")
                                    }.font(.system(size: 11)).foregroundStyle(HarborTheme.accent).padding(12)
                                        .overlay(RoundedRectangle(cornerRadius: 6).stroke(HarborTheme.accent.opacity(0.22)))
                                }.buttonStyle(.plain)
                            }.padding(.top, 17)
                        }
                        ForEach(session.messages) { message in
                            VStack(alignment: .leading, spacing: 8) {
                                HStack(spacing: 6) {
                                    if message.role == "Codex" { Image(systemName: "sparkle").foregroundStyle(HarborTheme.accent) }
                                    Text(message.role.uppercased()).font(.system(size: 9, weight: .semibold)).tracking(1)
                                        .foregroundStyle(message.role == "Codex" ? HarborTheme.accent : HarborTheme.muted)
                                }
                                if message.text.isEmpty && session.chatBusy {
                                    HStack(spacing: 9) { ProgressView().controlSize(.mini); Text("Waiting for Codex…").foregroundStyle(HarborTheme.muted) }.font(.system(size: 12))
                                } else {
                                    Text(message.text).font(.system(size: 12)).lineSpacing(4).textSelection(.enabled)
                                        .foregroundStyle(message.failed ? Color.orange : Color(white: 0.85))
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                }
                            }
                        }
                        Color.clear.frame(height: 1).id("chat-end")
                    }.padding(20)
                }.onChange(of: session.messages.last?.text) { _, _ in proxy.scrollTo("chat-end", anchor: .bottom) }
                    .onChange(of: session.messages.count) { _, _ in proxy.scrollTo("chat-end", anchor: .bottom) }
            }
            VStack(alignment: .leading, spacing: 11) {
                Text("Suggestions only. Guest files and console output are not shared. Each request starts a new turn.")
                    .font(.system(size: 10)).foregroundStyle(HarborTheme.muted).lineSpacing(3)
                VStack(spacing: 0) {
                    TextEditor(text: $session.prompt).font(.system(size: 12)).scrollContentBackground(.hidden)
                        .padding(8).frame(minHeight: 78, maxHeight: 105).disabled(session.chatBusy)
                        .accessibilityLabel("Message the local Codex assistant")
                        .overlay(alignment: .topLeading) {
                            if session.prompt.isEmpty { Text("Ask Codex…").font(.system(size: 12)).foregroundStyle(HarborTheme.muted).padding(.horizontal, 13).padding(.top, 16).allowsHitTesting(false) }
                        }
                    HStack {
                        Text(session.chatBusy ? "Receiving messages…" : "\(profile.id)")
                            .font(.system(size: 9, design: .monospaced)).foregroundStyle(HarborTheme.muted).lineLimit(1)
                        Spacer()
                        if session.chatBusy {
                            Button { model.cancelChat(profile.id) } label: { Label("Stop", systemImage: "stop.fill") }
                                .controlSize(.small).help("Cancel this assistant turn and its child processes")
                        } else {
                            Button { model.ask(profile.id) } label: { Image(systemName: "arrow.up").font(.system(size: 11, weight: .semibold)) }
                                .buttonStyle(.borderedProminent).controlSize(.small).keyboardShortcut(.return, modifiers: .command)
                                .help("Send to Codex (⌘Return)")
                                .disabled(session.prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                    }.padding(.horizontal, 12).padding(.bottom, 10)
                }.background(HarborTheme.canvas).clipShape(RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.white.opacity(0.10)))
            }.padding(16)
        }.background(HarborTheme.rail)
        .sheet(isPresented: $showSettings) { AssistantSettings(model: model, profile: profile) }
    }
}

struct AssistantSettings: View {
    @ObservedObject var model: WorkspaceModel
    let profile: MachineProfile
    @Environment(\.dismiss) private var dismiss
    @State private var program = ""
    @State private var saving = false
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Text("Local Codex CLI").font(.title2.weight(.semibold))
            Text("HarborVM runs Codex on your Mac in read-only, ephemeral mode. Sign in using Codex in your terminal first. Your existing Codex account and usage limits apply.")
                .font(.callout).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 8) {
                Text("Executable for \(profile.name)").font(.callout.weight(.medium))
                TextField("codex or /absolute/path/to/codex", text: $program).textFieldStyle(.roundedBorder)
                Text("Use an absolute path if the desktop app cannot find Codex. No API key is stored by HarborVM.").font(.caption).foregroundStyle(.secondary)
            }
            if let error = model.errorMessage { Text(error).font(.callout).foregroundStyle(.orange).textSelection(.enabled) }
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction).disabled(saving)
                Button(saving ? "Saving…" : "Save") {
                    saving = true; model.errorMessage = nil
                    model.setAssistant(id: profile.id, program: program.trimmingCharacters(in: .whitespacesAndNewlines)) { success in saving = false; if success { dismiss() } }
                }.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction).disabled(saving || program.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }.padding(28).frame(width: 460).tint(HarborTheme.accent)
            .onAppear { program = profile.assistant_program ?? "codex" }
    }
}
