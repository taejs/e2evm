import AppKit
import SwiftUI

enum HarborTheme {
    static let canvas = Color(red: 0.065, green: 0.073, blue: 0.080)
    static let rail = Color(red: 0.084, green: 0.094, blue: 0.104)
    static let panel = Color(red: 0.105, green: 0.116, blue: 0.127)
    static let accent = Color(red: 0.47, green: 0.79, blue: 0.73)
    static let muted = Color(red: 0.58, green: 0.63, blue: 0.66)
}

struct GuestDisplay: NSViewRepresentable {
    let machine: NativeMachine
    func makeNSView(context: Context) -> NSView { machine.view }
    func updateNSView(_ nsView: NSView, context: Context) {}
}

struct WorkspaceView: View {
    @ObservedObject var model: WorkspaceModel
    @State private var showCreate = false
    var body: some View {
        VStack(spacing: 0) {
            HSplitView {
                library.frame(minWidth: 200, idealWidth: 220, maxWidth: 260)
                if let profile = model.profiles.first(where: { $0.id == model.selectedID }) {
                    MachineWorkspace(model: model, profile: profile, session: model.session(profile.id))
                        .id(profile.id).frame(minWidth: 500, maxWidth: .infinity, maxHeight: .infinity)
                    AssistantPanel(model: model, profile: profile, session: model.session(profile.id))
                        .id("assistant-" + profile.id).frame(minWidth: 300, idealWidth: 330, maxWidth: 480)
                } else {
                    emptyWorkspace.frame(minWidth: 700, maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            if let error = model.errorMessage {
                Divider()
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "exclamationmark.circle.fill").foregroundStyle(.orange)
                    Text(error).font(.callout).textSelection(.enabled).lineLimit(5)
                    Spacer(minLength: 12)
                    Button { model.errorMessage = nil } label: { Image(systemName: "xmark") }
                        .buttonStyle(.plain).help("Dismiss error")
                }.padding(12).background(HarborTheme.panel)
            }
        }
        .background(HarborTheme.canvas).tint(HarborTheme.accent)
        .frame(minWidth: 1120, minHeight: 740).preferredColorScheme(.dark)
        .sheet(isPresented: $showCreate) { CreateProfileSheet(model: model) }
        .disabled(model.shuttingDown)
        .onChange(of: model.selectedID) { _, _ in model.refreshStatus() }
    }

    private var library: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "square.stack.3d.up.fill").font(.system(size: 21)).foregroundStyle(HarborTheme.accent)
                VStack(alignment: .leading, spacing: 3) {
                    Text("HarborVM").font(.system(size: 16, weight: .semibold))
                    Text("LOCAL WORKSPACE").font(.system(size: 9, weight: .medium)).tracking(1.5).foregroundStyle(HarborTheme.muted)
                }
            }.padding(.horizontal, 20).padding(.top, 25).padding(.bottom, 32)
            HStack {
                Text("MACHINES").font(.system(size: 10, weight: .semibold)).tracking(1.4).foregroundStyle(HarborTheme.muted)
                Spacer()
                Button { model.reload(); model.refreshStatus() } label: { Image(systemName: "arrow.clockwise").font(.system(size: 11)) }
                    .buttonStyle(.plain).help("Refresh machines (⌘R)").keyboardShortcut("r", modifiers: .command)
            }.padding(.horizontal, 20).padding(.bottom, 10)
            ScrollView {
                VStack(spacing: 5) {
                    ForEach(model.profiles) { profile in
                        Button { model.selectedID = profile.id } label: {
                            HStack(spacing: 10) {
                                Image(systemName: "desktopcomputer").font(.system(size: 17)).foregroundStyle(model.selectedID == profile.id ? HarborTheme.accent : HarborTheme.muted)
                                VStack(alignment: .leading, spacing: 5) {
                                    Text(profile.name).font(.system(size: 13, weight: .medium)).lineLimit(1).foregroundStyle(.primary)
                                    Text(libraryState(profile)).font(.system(size: 11)).foregroundStyle(HarborTheme.muted)
                                }
                                Spacer(minLength: 0)
                                if model.machines[profile.id]?.state == "running" { Circle().fill(HarborTheme.accent).frame(width: 5, height: 5) }
                            }
                            .padding(.horizontal, 12).padding(.vertical, 13)
                            .background(model.selectedID == profile.id ? HarborTheme.accent.opacity(0.09) : Color.clear)
                            .clipShape(RoundedRectangle(cornerRadius: 7))
                            .overlay(alignment: .leading) {
                                if model.selectedID == profile.id { RoundedRectangle(cornerRadius: 2).fill(HarborTheme.accent).frame(width: 2, height: 23) }
                            }
                            .contentShape(Rectangle())
                        }.buttonStyle(.plain).accessibilityLabel("Select \(profile.name)")
                    }
                    if model.profiles.isEmpty {
                        Text("No machines yet").font(.callout).foregroundStyle(HarborTheme.muted).padding(.vertical, 20)
                    }
                }.padding(.horizontal, 9)
            }
            Spacer(minLength: 16)
            Button { model.errorMessage = nil; showCreate = true } label: {
                Label("New machine", systemImage: "plus").font(.system(size: 12, weight: .medium)).frame(maxWidth: .infinity, alignment: .leading).padding(10)
            }.buttonStyle(.plain).background(HarborTheme.panel).clipShape(RoundedRectangle(cornerRadius: 6))
                .padding(.horizontal, 15).padding(.bottom, 18).keyboardShortcut("n", modifiers: .command)
            Divider().padding(.horizontal, 16)
            HStack(spacing: 6) {
                Image(systemName: "laptopcomputer")
                Text("On this Mac")
                Spacer()
                Text("ARM64").font(.system(size: 9, weight: .medium, design: .monospaced))
            }.font(.system(size: 10)).foregroundStyle(HarborTheme.muted).padding(18)
        }.background(HarborTheme.rail)
    }

    private func libraryState(_ profile: MachineProfile) -> String {
        let state = model.machines[profile.id]?.state ?? "stopped"
        return state == "stopped" ? (profile.ready ? "Ready · \(profile.guestOS == "macos" ? "macOS" : "Ubuntu")" : "Not prepared") : state.capitalized
    }

    private var emptyWorkspace: some View {
        VStack(alignment: .leading, spacing: 20) {
            Image(systemName: "desktopcomputer").font(.system(size: 40, weight: .ultraLight)).foregroundStyle(HarborTheme.accent)
            VStack(alignment: .leading, spacing: 10) {
                Text("A place for your next machine.").font(.system(size: 25, weight: .medium))
                Text("Create a machine, prepare its disk, then open\nan isolated desktop alongside your command console.")
                    .font(.system(size: 13)).lineSpacing(5).foregroundStyle(HarborTheme.muted)
            }
            Button("Create a machine") { showCreate = true }.buttonStyle(.borderedProminent).controlSize(.large)
            Text("Apple Silicon · Linux and macOS · Local storage").font(.system(size: 10)).foregroundStyle(HarborTheme.muted)
        }.frame(maxWidth: .infinity, maxHeight: .infinity).background(HarborTheme.canvas)
    }
}

struct MachineWorkspace: View {
    @ObservedObject var model: WorkspaceModel
    let profile: MachineProfile
    @ObservedObject var session: ProfileSession
    @State private var confirmPoweroff = false
    var state: String { model.machines[profile.id]?.state ?? "stopped" }
    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            if session.consoleVisible {
                VSplitView {
                    display.frame(minHeight: 230, maxHeight: .infinity)
                    GuestConsole(model: model, profile: profile, session: session, running: state == "running")
                        .frame(minHeight: 160, idealHeight: 230, maxHeight: 430)
                }
            } else { display.frame(maxHeight: .infinity) }
            Divider()
            HStack(spacing: 7) {
                Image(systemName: "cursorarrow").font(.system(size: 10))
                Text("Input stays in the guest view")
                Spacer()
                Button { session.consoleVisible.toggle() } label: { Label(session.consoleVisible ? "Hide console" : "Show console", systemImage: "terminal") }
                    .buttonStyle(.plain).keyboardShortcut("j", modifiers: .command)
            }.font(.system(size: 10)).foregroundStyle(HarborTheme.muted).padding(.horizontal, 16).padding(.vertical, 9).background(HarborTheme.panel)
        }
        .confirmationDialog("Force stop \(profile.name)?", isPresented: $confirmPoweroff, titleVisibility: .visible) {
            Button("Force stop", role: .destructive) { model.request("poweroff", id: profile.id) }
            Button("Cancel", role: .cancel) {}
        } message: { Text("Unsaved work inside this machine may be lost.") }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 15) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(profile.name).font(.system(size: 20, weight: .semibold)).lineLimit(1)
                    Text("\(profile.cpus) vCPU  /  \(String(format: "%g", Double(profile.memory_mib) / 1024)) GiB memory  /  \(profile.disk_gib) GiB disk")
                        .font(.system(size: 11)).foregroundStyle(HarborTheme.muted)
                }
                Spacer(minLength: 8)
                HStack(spacing: 6) {
                    Circle().fill(state == "running" ? HarborTheme.accent : HarborTheme.muted).frame(width: 6, height: 6)
                    Text(state.capitalized).font(.system(size: 11, weight: .medium))
                }.padding(.horizontal, 10).padding(.vertical, 7).background(HarborTheme.panel).clipShape(Capsule())
            }
            HStack(spacing: 8) {
                if profile.ready {
                    if state == "running" {
                        Button { model.request("pause", id: profile.id) } label: { Label("Pause", systemImage: "pause") }
                        Button { model.request("stop", id: profile.id) } label: { Label("Shut down", systemImage: "power") }
                    } else {
                        Button { model.request(state == "paused" ? "resume" : "start", id: profile.id) } label: {
                            Label(state == "paused" ? "Resume" : "Start machine", systemImage: "play.fill")
                        }.buttonStyle(.borderedProminent).disabled(state != "stopped" && state != "paused")
                    }
                    Menu {
                        Button("Force stop…", role: .destructive) { confirmPoweroff = true }.disabled(state == "stopped")
                    } label: { Image(systemName: "ellipsis") }.menuStyle(.borderlessButton).frame(width: 22)
                } else {
                    Button { model.request("prepare", id: profile.id) } label: { Label(profile.guestOS == "macos" ? "Install macOS" : "Prepare Linux", systemImage: "arrow.down.circle") }
                        .buttonStyle(.borderedProminent)
                }
                if session.action != nil { ProgressView().controlSize(.small).scaleEffect(0.7).frame(width: 16, height: 16) }
                Spacer(minLength: 2)
                Text(state == "running" ? (session.guestOnline ? "Guest online" : "Guest worker offline") : profile.ready ? "Disk prepared" : "Disk not prepared").font(.system(size: 10)).foregroundStyle(HarborTheme.muted)
            }.controlSize(.small).disabled(session.action != nil)
        }.padding(20).background(HarborTheme.canvas)
    }

    private var display: some View {
        ZStack {
            Color.black
            if let machine = model.machines[profile.id], machine.vm != nil {
                GuestDisplay(machine: machine).id(profile.id).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                VStack(spacing: 18) {
                    Image(systemName: session.action == "prepare" ? "arrow.down.circle" : "desktopcomputer")
                        .font(.system(size: 38, weight: .ultraLight)).foregroundStyle(HarborTheme.muted)
                    VStack(spacing: 8) {
                        Text(session.action == "prepare" ? "Preparing your machine" : profile.ready ? "Your desktop is ready" : "Start with a fresh disk")
                            .font(.system(size: 18, weight: .medium))
                        Text(session.action == "prepare" ? (profile.guestOS == "macos" ? "Downloading from Apple and installing macOS. This can take a while." : "Downloading and verifying Ubuntu. This can take several minutes.") : profile.ready ? "Start this machine to open its isolated desktop." : (profile.guestOS == "macos" ? "Download an Apple restore image into a private 64 GiB disk. Complete guest setup after installation." : "Prepare downloads Ubuntu ARM64 and creates a private 24 GiB disk.\nMemory is allocated only when you start the machine."))
                            .font(.system(size: 12)).foregroundStyle(HarborTheme.muted).multilineTextAlignment(.center).lineSpacing(4)
                    }
                    if session.action == "prepare" {
                        ProgressView().controlSize(.small)
                        ScrollView {
                            Text(session.progress.isEmpty ? "Waiting for preparation output…" : session.progress)
                                .font(.system(size: 10, design: .monospaced)).foregroundStyle(HarborTheme.muted).textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }.frame(maxWidth: 440, maxHeight: 100)
                    }
                    if let error = model.machines[profile.id]?.lastError {
                        Text(error).font(.callout).foregroundStyle(.orange).textSelection(.enabled).padding(.horizontal, 24)
                    }
                }.padding(24).frame(maxWidth: .infinity, maxHeight: .infinity).background(HarborTheme.canvas)
            }
        }.clipped()
    }
}

struct CreateProfileSheet: View {
    @ObservedObject var model: WorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var name = "Ubuntu workspace"
    @State private var identifier = "ubuntu"
    @State private var cpus = 2
    @State private var memory = 2048
    @State private var guestOS = "linux"
    @State private var desktop = true
    @State private var browser = true
    @State private var python = true
    @State private var node = true
    @State private var playwright = true
    private var without: [String] {
        [("desktop", desktop), ("browser", browser), ("python", python), ("node", node), ("playwright", playwright)].compactMap { $0.1 ? nil : $0.0 }
    }
    private var valid: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !identifier.isEmpty && identifier.count <= 40 && identifier.first != "-" && identifier.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || $0 == 45 }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            VStack(alignment: .leading, spacing: 8) {
                Text("New machine").font(.title2.weight(.semibold))
                Text("Create a profile first. Prepare and start it when you are ready.").foregroundStyle(.secondary).font(.callout)
            }
            Form {
                Picker("Operating system", selection: $guestOS) {
                    Text("Ubuntu Linux").tag("linux")
                    Text("macOS").tag("macos")
                }.onChange(of: guestOS) { _, value in
                    if value == "macos" {
                        if name == "Ubuntu workspace" { name = "macOS workspace" }
                        if identifier == "ubuntu" { identifier = "macos" }
                        memory = max(memory, 4096); cpus = max(cpus, 2); desktop = true
                    } else {
                        if name == "macOS workspace" { name = "Ubuntu workspace" }
                        if identifier == "macos" { identifier = "ubuntu" }
                    }
                }
                TextField("Name", text: $name)
                TextField("Profile ID", text: $identifier)
                Text("Use lowercase letters, numbers and hyphens.").font(.caption).foregroundStyle(.secondary)
                Stepper("CPU cores: \(cpus)", value: $cpus, in: (guestOS == "macos" ? 2 : 1)...min(32, ProcessInfo.processInfo.activeProcessorCount))
                Picker("Memory", selection: $memory) {
                    ForEach([1024, 2048, 4096, 8192, 16384].filter { guestOS != "macos" || $0 >= 4096 }, id: \.self) { Text("\($0 / 1024) GB").tag($0) }
                }
                Text(guestOS == "macos" ? "64 GiB sparse disk · Apple restore image" : "24 GiB sparse disk · Ubuntu ARM64").font(.caption).foregroundStyle(.secondary)
                Section("Optional setup") {
                    Text("Start with everything selected. Turn off what you do not need.").font(.caption).foregroundStyle(.secondary)
                    if guestOS == "linux" {
                        Toggle("Desktop", isOn: $desktop).onChange(of: desktop) { _, enabled in
                            if !enabled { browser = false; playwright = false }
                        }
                    }
                    Toggle("Chrome browser", isOn: $browser).disabled(!desktop).onChange(of: browser) { _, enabled in
                        if !enabled { playwright = false }
                    }
                    Toggle("Python development tools", isOn: $python).onChange(of: python) { _, enabled in
                        if !enabled { playwright = false }
                    }
                    Toggle("Node.js and npm", isOn: $node)
                    Toggle("Playwright for Python", isOn: $playwright).disabled(!browser || !python)
                    Text("Chrome needs a desktop. Playwright needs Chrome and Python tools. Turning these off also turns off their dependents.").font(.caption).foregroundStyle(.secondary)
                    Text("The guest connection runtime is required for commands. These choices apply to optional tools.").font(.caption).foregroundStyle(.secondary)
                    if guestOS == "macos" {
                        Text("After installation, complete Apple's setup in the VM and run the guest connection setup in its Terminal.").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }.formStyle(.grouped)
            if let error = model.errorMessage { Text(error).font(.callout).foregroundStyle(.orange).textSelection(.enabled) }
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction).disabled(model.creating)
                Button(model.creating ? "Creating…" : "Create profile") {
                    model.errorMessage = nil
                    model.create(id: identifier, name: name, cpus: cpus, memory: memory, guestOS: guestOS, without: without) { success in if success { dismiss() } }
                }.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction).disabled(!valid || model.creating)
            }
        }.padding(28).frame(width: 520, height: 690).tint(HarborTheme.accent)
    }
}
