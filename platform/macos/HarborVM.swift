import AppKit
import SwiftUI

final class ApplicationDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var model: WorkspaceModel!
    var server: ControlServer?
    var window: NSWindow!
    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            let arguments = CommandLine.arguments
            func value(_ flag: String) -> String? {
                guard let index = arguments.firstIndex(of: flag), index + 1 < arguments.count else { return nil }
                return arguments[index + 1]
            }
            let root = value("--home") ?? ProcessInfo.processInfo.environment["HARBORVM_HOME"] ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/HarborVM").path
            let cli = value("--cli") ?? Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/harborvm").path
            // Rust owns directory creation and durable profile validation.
            let setup = Process(); setup.executableURL = URL(fileURLWithPath: cli); setup.arguments = ["profile", "list"]
            var environment = ProcessInfo.processInfo.environment; environment["HARBORVM_HOME"] = root; setup.environment = environment
            setup.standardOutput = FileHandle.nullDevice
            try setup.run(); setup.waitUntilExit()
            guard setup.terminationStatus == 0 else { throw HarborError("Cannot initialize the profile store") }
            model = WorkspaceModel(root: root, cli: cli)
            server = try ControlServer(root: root, model: model)
            window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1280, height: 820), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
            window.title = "HarborVM"; window.delegate = self; window.isReleasedWhenClosed = false
            window.contentView = NSHostingView(rootView: WorkspaceView(model: model))
            window.center(); window.makeKeyAndOrderFront(nil)
            let menu = NSMenu(), application = NSMenu(), item = NSMenuItem()
            application.addItem(withTitle: "Quit HarborVM", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
            item.submenu = application; menu.addItem(item); NSApp.mainMenu = menu
            server?.start()
            NSApp.activate(ignoringOtherApps: true)
        } catch {
            fputs("HarborVM: \(error.localizedDescription)\n", stderr)
            NSApp.terminate(nil)
        }
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool { window?.makeKeyAndOrderFront(nil); return true }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard model?.hasActiveMachines == true else { return .terminateNow }
        model.errorMessage = "Shut down or force stop your active machines before quitting HarborVM."
        window.makeKeyAndOrderFront(nil)
        return .terminateCancel
    }
}

@main
struct HarborVMApplication {
    static func main() {
        let application = NSApplication.shared
        let delegate = ApplicationDelegate()
        application.setActivationPolicy(.regular)
        application.delegate = delegate
        withExtendedLifetime(delegate) { application.run() }
    }
}
