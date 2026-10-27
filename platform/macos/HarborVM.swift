import AppKit
import SwiftUI

final class ApplicationDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var model: WorkspaceModel!
    var server: ControlServer?
    var window: NSWindow!
    private var terminationPending = false
    private var terminationReady = false
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
            window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1440, height: 920), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
            window.title = "HarborVM"; window.delegate = self; window.isReleasedWhenClosed = false
            window.minSize = NSSize(width: 1120, height: 762)
            window.titlebarAppearsTransparent = true
            window.backgroundColor = NSColor(red: 0.065, green: 0.073, blue: 0.080, alpha: 1)
            window.contentView = NSHostingView(rootView: WorkspaceView(model: model))
            window.center()
            if arguments.contains("--background") { window.orderBack(nil) }
            else { window.makeKeyAndOrderFront(nil) }
            installMenus()
            server?.start()
            if !arguments.contains("--background") { NSApp.activate(ignoringOtherApps: true) }
        } catch {
            fputs("HarborVM: \(error.localizedDescription)\n", stderr)
            NSApp.terminate(nil)
        }
    }
    private func installMenus() {
        let menu = NSMenu()
        func addMenu(_ title: String) -> NSMenu {
            let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
            let submenu = NSMenu(title: title); item.submenu = submenu; menu.addItem(item)
            return submenu
        }
        let application = addMenu("HarborVM")
        application.addItem(withTitle: "Hide HarborVM", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        application.addItem(.separator())
        application.addItem(withTitle: "Quit HarborVM", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        let edit = addMenu("Edit")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        let redo = edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        let windows = addMenu("Window")
        windows.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windows.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        NSApp.windowsMenu = windows
        NSApp.mainMenu = menu
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool { window?.makeKeyAndOrderFront(nil); return true }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let model else { return .terminateNow }
        if model.hasActiveMachines || model.preparing || model.creating {
            model.errorMessage = model.hasActiveMachines
                ? "Shut down or force stop your active machines before quitting HarborVM."
                : "Let machine preparation or profile creation finish before quitting HarborVM."
            window.makeKeyAndOrderFront(nil)
            return .terminateCancel
        }
        if terminationReady { return .terminateNow }
        if terminationPending { return .terminateCancel }
        terminationPending = true
        // Keep the normal event loop alive while Rust finishes subprocess-group cleanup.
        // terminateLater enters an AppKit modal loop that can starve these callbacks.
        model.finishProcesses { [weak self] in
            DispatchQueue.main.async {
                self?.terminationReady = true
                sender.terminate(nil)
            }
        }
        return .terminateCancel
    }
}

@main
struct HarborVMApplication {
    static func main() {
        if MacOSProvisioner.runIfRequested(arguments: CommandLine.arguments) {
            RunLoop.main.run()
            return
        }
        let application = NSApplication.shared
        let delegate = ApplicationDelegate()
        application.setActivationPolicy(.regular)
        application.delegate = delegate
        withExtendedLifetime(delegate) { application.run() }
    }
}
