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
                    Text(profile.connection.detail.uppercased()).font(.system(size: 9, weight: .medium)).tracking(0.8).foregroundStyle(HarborTheme.muted)
                }
                Spacer()
                Menu {
                    Button("Configure connection…") { model.errorMessage = nil; showSettings = true }.disabled(session.chatBusy)
                    Button("Clear conversation") { model.clearConversation(profile.id) }.disabled(session.chatBusy || session.messages.isEmpty)
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
                                    if message.role == "assistant" { Image(systemName: "sparkle").foregroundStyle(HarborTheme.accent) }
                                    Text(message.role == "user" ? "YOU" : profile.connection.title.uppercased()).font(.system(size: 9, weight: .semibold)).tracking(1)
                                        .foregroundStyle(message.role == "assistant" ? HarborTheme.accent : HarborTheme.muted)
                                }
                                if message.text.isEmpty && session.chatBusy {
                                    HStack(spacing: 9) { ProgressView().controlSize(.mini); Text("Waiting for response…").foregroundStyle(HarborTheme.muted) }.font(.system(size: 12))
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
                Text("Suggestions only. This conversation is sent to your selected provider. Guest files and console output are not shared.")
                    .font(.system(size: 10)).foregroundStyle(HarborTheme.muted).lineSpacing(3)
                VStack(spacing: 0) {
                    TextEditor(text: $session.prompt).font(.system(size: 12)).scrollContentBackground(.hidden)
                        .padding(8).frame(minHeight: 78, maxHeight: 105).disabled(session.chatBusy)
                        .accessibilityLabel("Message the assistant")
                        .overlay(alignment: .topLeading) {
                            if session.prompt.isEmpty { Text("Ask \(profile.connection.title)…").font(.system(size: 12)).foregroundStyle(HarborTheme.muted).padding(.horizontal, 13).padding(.top, 16).allowsHitTesting(false) }
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
                                .help("Send message (⌘Return)")
                                .disabled(session.prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                    }.padding(.horizontal, 12).padding(.bottom, 10)
                }.background(HarborTheme.canvas).clipShape(RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.white.opacity(0.10)))
            }.padding(16)
        }.background(HarborTheme.rail)
        .sheet(isPresented: $showSettings) { AssistantSettings(model: model, profile: profile, session: session) }
    }
}

struct AssistantSettings: View {
    @ObservedObject var model: WorkspaceModel
    let profile: MachineProfile
    @ObservedObject var session: ProfileSession
    @Environment(\.dismiss) private var dismiss
    @State private var provider: AssistantConnection.Provider = .codex
    @State private var program = ""
    @State private var endpoint = ""
    @State private var modelName = ""
    @State private var apiKey = ""
    @State private var keyEnvironment = ""
    @State private var saving = false
    @State private var models: [String] = []
    @State private var modelNotice = ""
    init(model: WorkspaceModel, profile: MachineProfile, session: ProfileSession) {
        self.model = model; self.profile = profile; self.session = session
        _provider = State(initialValue: profile.connection.provider)
        _program = State(initialValue: profile.assistant_program ?? "codex")
        _endpoint = State(initialValue: profile.connection.endpoint ?? profile.connection.provider.defaultEndpoint)
        _modelName = State(initialValue: profile.connection.model ?? "")
        _keyEnvironment = State(initialValue: profile.connection.key_env ?? "")
        _apiKey = State(initialValue: session.apiKey)
    }
    private var connection: AssistantConnection {
        AssistantConnection(provider: provider,
                            endpoint: provider == .codex ? nil : cleaned(endpoint),
                            model: provider == .codex ? nil : cleaned(modelName),
                            key_env: provider == .codex ? nil : cleaned(keyEnvironment))
    }
    private func cleaned(_ text: String) -> String? {
        let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }
    private func save(fetchModels: Bool) {
        saving = true; model.errorMessage = nil; modelNotice = ""
        model.setConnection(id: profile.id, connection: connection,
                            program: cleaned(program) ?? "codex", apiKey: provider == .codex ? "" : apiKey) { success in
            guard success else { saving = false; return }
            if fetchModels {
                model.fetchModels(profile.id) { result in
                    saving = false
                    if let result {
                        models = result
                        modelNotice = result.isEmpty ? "No models were returned. You can enter a model name manually." : "\(result.count) models available. Choose one below, then save."
                    }
                }
            } else { saving = false; dismiss() }
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Assistant connection").font(.title2.weight(.semibold))
            Text("Settings for \(profile.name)").font(.callout).foregroundStyle(.secondary)
            Picker("Provider", selection: $provider) {
                ForEach(AssistantConnection.Provider.allCases) { item in Text(item.title).tag(item) }
            }.onChange(of: provider) { _, selected in
                endpoint = selected.defaultEndpoint; modelName = ""; models = []; modelNotice = ""; apiKey = ""; keyEnvironment = ""
            }
            if provider == .codex {
                Text("Uses Codex on your Mac in read-only, ephemeral mode. Sign in using Codex in your terminal first. Your existing account and usage limits apply.")
                    .font(.callout).foregroundStyle(.secondary)
                TextField("codex or /absolute/path/to/codex", text: $program).textFieldStyle(.roundedBorder)
                    .accessibilityLabel("Codex executable")
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Server URL").font(.callout.weight(.medium))
                    TextField(provider == .ollama ? "http://127.0.0.1:11434" : "https://your-server.example/v1", text: $endpoint)
                        .textFieldStyle(.roundedBorder).accessibilityLabel("Assistant server URL")
                        .onChange(of: endpoint) { _, _ in apiKey = ""; keyEnvironment = ""; models = []; modelNotice = "" }
                    Text(provider == .ollama ? "Ollama's native chat API. The server must already be running." : "Requires OpenAI-compatible chat completions. Include the server's API base path, such as /v1.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                VStack(alignment: .leading, spacing: 8) {
                    Text("Model").font(.callout.weight(.medium))
                    HStack {
                        TextField("Enter a model name", text: $modelName).textFieldStyle(.roundedBorder)
                        if !models.isEmpty {
                            Menu("Choose") {
                                ForEach(models, id: \.self) { name in Button(name) { modelName = name } }
                            }.fixedSize()
                        }
                    }
                    Button("Save connection & fetch models") { save(fetchModels: true) }.disabled(saving || cleaned(endpoint) == nil)
                    if !modelNotice.isEmpty { Text(modelNotice).font(.caption).foregroundStyle(.secondary) }
                }
                VStack(alignment: .leading, spacing: 8) {
                    Text("Authentication · optional").font(.callout.weight(.medium))
                    SecureField("API key for this app session", text: $apiKey).textFieldStyle(.roundedBorder)
                    Text("Kept in memory until you quit; never written to your profile. Changing the server clears these credentials. A typed key overrides the environment variable.")
                        .font(.caption).foregroundStyle(.secondary)
                    TextField("Environment variable name, e.g. MY_PROVIDER_KEY", text: $keyEnvironment).textFieldStyle(.roundedBorder)
                        .accessibilityLabel("API key environment variable name")
                    Text("Only the variable name is saved. The variable must be available to E2EVM when the app starts.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
            Text("Changing the provider, server, model, or Codex executable clears this conversation. Messages remain in memory only.")
                .font(.caption).foregroundStyle(.secondary)
            if let error = model.errorMessage { Text(error).font(.callout).foregroundStyle(.orange).textSelection(.enabled) }
            HStack {
                if saving { ProgressView().controlSize(.small) }
                Spacer()
                Button("Close") { dismiss() }.keyboardShortcut(.cancelAction).disabled(saving)
                Button(saving ? "Saving…" : "Save") { save(fetchModels: false) }
                    .buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
                    .disabled(saving || (provider != .codex && (cleaned(endpoint) == nil || cleaned(modelName) == nil)))
            }
        }.padding(28).frame(width: 480).tint(HarborTheme.accent)
            .disabled(saving)
            .interactiveDismissDisabled(saving)

    }
}
