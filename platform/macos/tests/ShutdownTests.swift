import AppKit
import Foundation

// The model only needs the delegate's window for UI-only requests in this harness.
final class ApplicationDelegate: NSObject, NSApplicationDelegate { var window: NSWindow? }

@main
struct ShutdownTests {
    static func main() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("e2evm-shutdown-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let cli = root.appendingPathComponent("cli")
        try """
        #!/bin/sh
        case "$1" in
          profile) printf '[]' ;;
          run) sleep 0.3 ;;
          *) exit 2 ;;
        esac
        """.write(to: cli, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: cli.path)
        let model = WorkspaceModel(root: root.path, cli: cli.path)
        model.session("test").command = "fixture"
        model.runCommand("test")
        precondition(model.session("test").commandBusy)
        var finished = false
        model.finishProcesses { finished = true }
        precondition(!finished, "Cleanup must still have an outstanding job callback")
        for action in ["start", "refresh", "activate", "quit"] {
            var reply: [String: Any]?
            model.handle(["version": 1, "profile": "test", "action": action]) { reply = $0 }
            precondition(reply?["ok"] as? Bool == false)
            precondition((reply?["error"] as? String)?.contains("shutting down") == true)
        }
        model.perform("start", id: "test")
        precondition(model.errorMessage?.contains("shutting down") == true)
        var created: Bool?
        model.create(id: "late", name: "Late", cpus: 2, memory: 2048) { created = $0 }
        precondition(created == false && !model.creating)
        precondition(model.machines.isEmpty, "Rejected starts must never construct a VM")
        let deadline = Date().addingTimeInterval(5)
        while !finished && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
        precondition(finished, "Cancellation must complete without admitting new jobs")
        print("Native shutdown admission checks passed")
    }
}
