import AppKit
import SwiftUI
import Virtualization

struct MachineProfile: Decodable, Identifiable {
    let version: Int
    let id: String
    let name: String
    let cpus: Int
    let memory_mib: Int
    let disk_gib: Int
    let ready: Bool
    let assistant_program: String?
    var directory: String?
}

final class WorkspaceModel: ObservableObject {
    let root: String
    let cli: String
    @Published var profiles: [MachineProfile] = []
    @Published var selectedID: String?
    @Published var errorMessage: String?
    @Published var revision = 0
    var machines: [String: NativeMachine] = [:]
    var timer: Timer?
    init(root: String, cli: String) {
        self.root = root; self.cli = cli
        reload()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.revision += 1 }
    }
    deinit { timer?.invalidate() }
    func runCLI(_ arguments: [String]) throws -> Data {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: cli)
        process.arguments = arguments
        var environment = ProcessInfo.processInfo.environment
        environment["HARBORVM_HOME"] = root
        process.environment = environment
        let output = Pipe(), errors = Pipe()
        process.standardOutput = output; process.standardError = errors
        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        let failure = errors.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw HarborError(String(data: failure, encoding: .utf8) ?? "CLI failed")
        }
        return data
    }
    func reload() {
        do {
            profiles = try JSONDecoder().decode([MachineProfile].self, from: runCLI(["profile", "list"]))
            if selectedID == nil { selectedID = profiles.first?.id }
        } catch { errorMessage = error.localizedDescription }
    }
    func machine(_ id: String, refreshStopped: Bool = false) throws -> NativeMachine {
        if let machine = machines[id], !refreshStopped || machine.state != "stopped" { return machine }
        guard !id.isEmpty, id.count <= 40, id.first != "-", id.utf8.allSatisfy({ (97...122).contains($0) || (48...57).contains($0) || $0 == 45 }) else { throw HarborError("Invalid profile ID") }
        let profile = try JSONDecoder().decode(MachineProfile.self, from: runCLI(["profile", "show", id]))
        guard profile.id == id, profile.version == 1,
              profile.directory == URL(fileURLWithPath: root).appendingPathComponent("profiles/\(id)").path else { throw HarborError("Invalid profile descriptor") }
        let machine = NativeMachine(profile: profile, root: root)
        machine.onChange = { [weak self] in self?.revision += 1 }
        machines[id] = machine
        return machine
    }
    var hasActiveMachines: Bool { machines.values.contains { $0.state != "stopped" } }
    func perform(_ action: String, id: String) {
        do { try machine(id, refreshStopped: action == "start").perform(action) { [weak self] result in
            if case .failure(let error) = result { self?.errorMessage = error.localizedDescription }
            self?.revision += 1
        } } catch { errorMessage = error.localizedDescription }
    }
    func handle(_ request: [String: Any], completion: @escaping ([String: Any]) -> Void) {
        do {
            guard request["version"] as? Int == 1, let id = request["profile"] as? String,
                  let action = request["action"] as? String else { throw HarborError("Expected version 1, profile and action") }
            let payload = request["payload"] as? [String: Any] ?? [:]
            if action == "activate" {
                reload()
                (NSApp.delegate as? ApplicationDelegate)?.window?.makeKeyAndOrderFront(nil)
                NSApp.activate(ignoringOtherApps: true)
                completion(["ok": true]); return
            }
            if action == "refresh" { reload(); completion(["ok": true]); return }
            if action == "app-screenshot" {
                guard let view = NSApp.windows.first(where: { $0.isVisible })?.contentView,
                      let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { throw HarborError("No visible workspace window") }
                view.cacheDisplay(in: view.bounds, to: bitmap)
                completion(["ok": true, "png_base64": bitmap.representation(using: .png, properties: [:])!.base64EncodedString()]); return
            }
            let machine = try machine(id, refreshStopped: action == "start")
            switch action {
            case "status": completion(machine.status)
            case "select": selectedID = id; NSApp.activate(ignoringOtherApps: true); completion(machine.status)
            case "screenshot": completion(["ok": true, "png_base64": try machine.screenshot().base64EncodedString()])
            case "click", "key", "text": try machine.input(action, payload: payload); completion(machine.status)
            default: try machine.perform(action) { result in
                switch result {
                case .success: completion(machine.status)
                case .failure(let error): completion(["ok": false, "error": error.localizedDescription])
                }
            }
            }
            revision += 1
        } catch { completion(["ok": false, "error": error.localizedDescription]) }
    }
}
