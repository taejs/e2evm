import AppKit
import SwiftUI

struct GuestDisplay: NSViewRepresentable {
    let machine: NativeMachine
    func makeNSView(context: Context) -> NSView { machine.view }
    func updateNSView(_ nsView: NSView, context: Context) {}
}

struct WorkspaceView: View {
    @ObservedObject var model: WorkspaceModel
    var body: some View {
        NavigationSplitView {
            List(model.profiles, selection: $model.selectedID) { profile in
                VStack(alignment: .leading, spacing: 5) {
                    Label(profile.name, systemImage: "desktopcomputer")
                    Text(model.machines[profile.id]?.state ?? "stopped").font(.caption).foregroundStyle(.secondary)
                }.padding(.vertical, 5).tag(profile.id)
            }
            .navigationTitle("HarborVM")
            .navigationSplitViewColumnWidth(min: 190, ideal: 220)
        } detail: {
            VStack(spacing: 0) {
                if let id = model.selectedID, let profile = model.profiles.first(where: { $0.id == id }) {
                    HStack {
                        VStack(alignment: .leading) {
                            Text(profile.name).font(.title2.bold())
                            Text("\(profile.cpus) CPUs · \(profile.memory_mib) MiB").font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        let state = model.machines[id]?.state ?? "stopped"
                        Text(state.capitalized).foregroundStyle(.secondary)
                        Button(state == "paused" ? "Resume" : "Start") { model.perform(state == "paused" ? "resume" : "start", id: id) }.disabled(state != "stopped" && state != "paused")
                        Button("Pause") { model.perform("pause", id: id) }.disabled(state != "running")
                        Button("Shut down") { model.perform("stop", id: id) }.disabled(state != "running")
                        Menu { Button("Force stop", role: .destructive) { model.perform("poweroff", id: id) } } label: { Image(systemName: "ellipsis") }.disabled(state == "stopped")
                    }.padding(20)
                    Divider()
                    if let machine = model.machines[id], machine.vm != nil {
                        GuestDisplay(machine: machine).id(id).frame(maxWidth: .infinity, maxHeight: .infinity)
                    } else {
                        ContentUnavailableView(profile.ready ? "Ready to start" : "Prepare your machine", systemImage: "desktopcomputer", description: Text(profile.ready ? "Start this machine to open its isolated desktop." : "Use the CLI to prepare a Linux disk for this profile.")).frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                } else {
                    ContentUnavailableView("Your local workspace", systemImage: "square.stack.3d.up", description: Text("Create a profile with the HarborVM CLI to get started.")).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
                if let error = model.errorMessage {
                    HStack { Image(systemName: "exclamationmark.triangle"); Text(error).textSelection(.enabled); Spacer(); Button("Dismiss") { model.errorMessage = nil } }.padding(12).background(.red.opacity(0.12))
                }
            }
        }.frame(minWidth: 900, minHeight: 600).preferredColorScheme(.dark)
    }
}
