import Foundation
import Darwin

@main
struct SafetyTests {
    static func expectFailure(_ label: String, _ operation: () throws -> Void) throws {
        do { try operation() } catch { return }
        throw HarborError("Expected failure: \(label)")
    }
    static func main() throws {
        let selectAll = try GuestKeyboard.named("ctrl+a")
        assert(selectAll.0 == 0 && selectAll.2.contains(.control))
        let uppercase = try GuestKeyboard.character("H")
        assert(uppercase.0 == 4 && uppercase.2.contains(.shift))
        let punctuation = try GuestKeyboard.character("!")
        assert(punctuation.0 == 18 && punctuation.2.contains(.shift))
        try expectFailure("unknown modifier") { _ = try GuestKeyboard.named("super+enter") }
        try expectFailure("non-ASCII scan code") { _ = try GuestKeyboard.character("한") }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("e2evm-lock-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        defer { try? FileManager.default.removeItem(at: root) }
        try requirePrivateDirectory(root.path)
        let file = root.appendingPathComponent("disk.img").path
        FileManager.default.createFile(atPath: file, contents: Data(), attributes: [.posixPermissions: 0o600])
        var held: FileLease? = try FileLease(file, create: false)
        try expectFailure("second disk owner") { _ = try FileLease(file, create: false) }
        withExtendedLifetime(held) {}
        held = nil
        let reopened = try FileLease(file, create: false)
        withExtendedLifetime(reopened) {}
        let profileLock = try FileLease(root.appendingPathComponent("profile.lock").path)
        try expectFailure("second profile owner") { _ = try FileLease(root.appendingPathComponent("profile.lock").path) }
        withExtendedLifetime(profileLock) {}
        let appLock = try FileLease(root.appendingPathComponent("app.lock").path)
        try expectFailure("second app owner") { _ = try FileLease(root.appendingPathComponent("app.lock").path) }
        withExtendedLifetime(appLock) {}
        let link = root.appendingPathComponent("link").path
        try FileManager.default.createSymbolicLink(atPath: link, withDestinationPath: file)
        try expectFailure("symlink lock") { _ = try FileLease(link) }
        try expectFailure("symlink disk") { try requireRegularFile(link) }
        try requireTransition("start", state: "stopped")
        try requireTransition("pause", state: "running")
        try requireTransition("resume", state: "paused")
        try requireTransition("stop", state: "running")
        try requireTransition("poweroff", state: "paused")
        try expectFailure("start active guest") { try requireTransition("start", state: "running") }
        try expectFailure("pause stopped guest") { try requireTransition("pause", state: "stopped") }
        try expectFailure("resume running guest") { try requireTransition("resume", state: "running") }
        try expectFailure("unknown action") { try requireTransition("delete", state: "stopped") }
        print("PASS: app/profile/disk exclusivity, safe paths, and lifecycle transitions")
    }
}
