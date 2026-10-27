import AppKit
import Virtualization

final class NativeMachine: NSObject, VZVirtualMachineDelegate {
    let profile: MachineProfile
    let root: String
    let view = VZVirtualMachineView()
    var vm: VZVirtualMachine?
    private var profileLease: FileLease?
    private var diskLease: FileLease?
    private var serial: FileHandle?
    private var shutdownRequested = false
    var lastError: String?
    var onChange: (() -> Void)?
    private(set) var lastPointer: [String: Double]?
    init(profile: MachineProfile, root: String) {
        self.profile = profile; self.root = root
        super.init()
        view.frame = NSRect(x: 0, y: 0, width: 1440, height: 900)
        view.capturesSystemKeys = false
        view.automaticallyReconfiguresDisplay = false
    }
    var state: String {
        guard let vm else { return "stopped" }
        if shutdownRequested && vm.state == .running { return "stopping" }
        switch vm.state {
        case .stopped: return "stopped"
        case .running: return "running"
        case .paused: return "paused"
        case .starting: return "starting"
        case .pausing: return "pausing"
        case .resuming: return "resuming"
        case .stopping: return "stopping"
        case .error: return "error"
        case .saving: return "saving"
        case .restoring: return "restoring"
        @unknown default: return "unknown"
        }
    }
    var status: [String: Any] {
        var result: [String: Any] = ["ok": true, "profile": profile.id, "name": profile.name, "state": state, "ready": profile.ready, "input_scope": "guest-view", "clipboard_sync": false]
        if let lastError { result["last_error"] = lastError }
        if let lastPointer { result["pointer"] = lastPointer }
        return result
    }
    private func configure() throws {
        guard let directory = profile.directory else { throw HarborError("Missing profile directory") }
        try requirePrivateDirectory(root)
        try requirePrivateDirectory(root + "/profiles")
        try requirePrivateDirectory(directory)
        let lease = try FileLease(directory + "/profile.lock")
        for file in ["disk.img", "kernel", "initrd", "seed.iso"] { try requireRegularFile(directory + "/" + file) }
        // VZ acquires its own image lock when starting. Holding flock on the image
        // ourselves makes the VZ storage service reject it. Pin identity in a sidecar.
        let leaseDirectory = root + "/disk-leases"
        try FileManager.default.createDirectory(atPath: leaseDirectory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try requirePrivateDirectory(leaseDirectory)
        var diskInfo = stat()
        guard lstat(directory + "/disk.img", &diskInfo) == 0 else { throw HarborError("Cannot identify disk") }
        let disk = try FileLease(leaseDirectory + "/\(diskInfo.st_dev)-\(diskInfo.st_ino).lock")
        try requirePrivateDirectory(directory + "/mailbox")
        let config = VZVirtualMachineConfiguration()
        config.cpuCount = profile.cpus
        config.memorySize = UInt64(profile.memory_mib) * 1024 * 1024
        let boot = VZLinuxBootLoader(kernelURL: URL(fileURLWithPath: directory + "/kernel"))
        boot.initialRamdiskURL = URL(fileURLWithPath: directory + "/initrd")
        boot.commandLine = "root=/dev/vda rw rootwait console=hvc0 console=tty0"
        config.bootLoader = boot
        config.platform = VZGenericPlatformConfiguration()
        let mainDisk = try VZDiskImageStorageDeviceAttachment(url: URL(fileURLWithPath: directory + "/disk.img"), readOnly: false, cachingMode: .cached, synchronizationMode: .fsync)
        let seedDisk = try VZDiskImageStorageDeviceAttachment(url: URL(fileURLWithPath: directory + "/seed.iso"), readOnly: true, cachingMode: .cached, synchronizationMode: .fsync)
        config.storageDevices = [VZVirtioBlockDeviceConfiguration(attachment: mainDisk), VZVirtioBlockDeviceConfiguration(attachment: seedDisk)]
        let network = VZVirtioNetworkDeviceConfiguration(); network.attachment = VZNATNetworkDeviceAttachment()
        config.networkDevices = [network]
        config.entropyDevices = [VZVirtioEntropyDeviceConfiguration()]
        config.memoryBalloonDevices = [VZVirtioTraditionalMemoryBalloonDeviceConfiguration()]
        let graphics = VZVirtioGraphicsDeviceConfiguration()
        graphics.scanouts = [VZVirtioGraphicsScanoutConfiguration(widthInPixels: 1440, heightInPixels: 900)]
        config.graphicsDevices = [graphics]
        config.keyboards = [VZUSBKeyboardConfiguration()]
        config.pointingDevices = [VZUSBScreenCoordinatePointingDeviceConfiguration()]
        let mailbox = VZVirtioFileSystemDeviceConfiguration(tag: "HarborVM")
        mailbox.share = VZSingleDirectoryShare(directory: VZSharedDirectory(url: URL(fileURLWithPath: directory + "/mailbox"), readOnly: false))
        let kit = VZVirtioFileSystemDeviceConfiguration(tag: "HarborKit")
        let kitURL = Bundle.main.resourceURL!.appendingPathComponent("guest")
        guard FileManager.default.fileExists(atPath: kitURL.appendingPathComponent("worker.py").path) else { throw HarborError("Packaged guest kit is missing; rebuild HarborVM.app") }
        kit.share = VZSingleDirectoryShare(directory: VZSharedDirectory(url: kitURL, readOnly: true))
        config.directorySharingDevices = [mailbox, kit]
        let serialFD = open(directory + "/serial.log", O_WRONLY | O_CREAT | O_APPEND | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard serialFD >= 0 else { throw HarborError("Cannot open serial log") }
        let serialHandle = FileHandle(fileDescriptor: serialFD, closeOnDealloc: true)
        let port = VZVirtioConsoleDeviceSerialPortConfiguration()
        port.attachment = VZFileHandleSerialPortAttachment(fileHandleForReading: nil, fileHandleForWriting: serialHandle)
        config.serialPorts = [port]
        try config.validate()
        let machine = VZVirtualMachine(configuration: config)
        machine.delegate = self
        profileLease = lease; diskLease = disk; serial = serialHandle
        vm = machine; view.virtualMachine = machine
    }
    func perform(_ action: String, completion: @escaping (Result<Void, Error>) -> Void) throws {
        try requireTransition(action, state: state)
        if action == "start" {
            guard profile.ready else { throw HarborError("Prepare this profile before starting it") }
            try configure()
        }
        guard let vm else { throw HarborError("Machine is not configured") }
        let done: (Result<Void, Error>) -> Void = { [weak self] result in
            if case .failure(let error) = result { self?.lastError = error.localizedDescription }
            if self?.vm?.state == .stopped || self?.vm?.state == .error { self?.release() }
            self?.onChange?(); completion(result)
        }
        switch action {
        case "start": lastError = nil; shutdownRequested = false; vm.start(completionHandler: done)
        case "pause": vm.pause(completionHandler: done)
        case "resume": vm.resume(completionHandler: done)
        case "stop": try vm.requestStop(); shutdownRequested = true; done(.success(()))
        case "poweroff": vm.stop { error in
            if let error { done(.failure(error)) } else { done(.success(())) }
        }
        default: throw HarborError("Unknown machine action")
        }
        onChange?()
    }
    private func release() {
        view.virtualMachine = nil; vm = nil; serial = nil
        diskLease = nil; profileLease = nil; shutdownRequested = false
    }
    func guestDidStop(_ virtualMachine: VZVirtualMachine) { release(); onChange?() }
    func virtualMachine(_ virtualMachine: VZVirtualMachine, didStopWithError error: Error) { lastError = error.localizedDescription; release(); onChange?() }
    func screenshot() throws -> Data {
        guard vm != nil, view.window?.isVisible == true, view.bounds.width > 0, let image = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { throw HarborError("No guest display is available") }
        view.cacheDisplay(in: view.bounds, to: image)
        guard let data = image.representation(using: .png, properties: [:]) else { throw HarborError("Could not encode guest screenshot") }
        return data
    }
    func input(_ action: String, payload: [String: Any]) throws {
        guard state == "running", view.window?.isVisible == true else { throw HarborError("Guest input requires a running machine selected in a visible workspace") }
        if action == "click" {
            guard let x = payload["x"] as? Double, let y = payload["y"] as? Double,
                  x.isFinite, y.isFinite, (0...1).contains(x), (0...1).contains(y) else { throw HarborError("Click coordinates must be normalized numbers from 0 to 1") }
            let frame = view.bounds
            // VZ scales a fixed-size scanout to fit; translate through its letterbox.
            let scale = min(frame.width / 1440, frame.height / 900)
            let local = NSPoint(x: (frame.width - 1440 * scale) / 2 + x * 1440 * scale, y: (frame.height - 900 * scale) / 2 + (view.isFlipped ? y : 1 - y) * 900 * scale)
            let point = view.convert(local, to: nil)
            for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                guard let event = NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: view.window?.windowNumber ?? 0, context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0) else { throw HarborError("Could not construct guest pointer event") }
                if type == .leftMouseDown { view.mouseDown(with: event) } else { view.mouseUp(with: event) }
            }
            lastPointer = ["x": x, "y": y]
        } else {
            let strokes: [(UInt16, String, NSEvent.ModifierFlags)]
            if action == "text" {
                guard let value = payload["text"] as? String, value.utf8.count <= 16384 else { throw HarborError("Text must contain at most 16384 UTF-8 bytes") }
                // Virtual keyboards consume scan codes, not arbitrary Unicode strings.
                // The guest command channel supports full Unicode without keyboard-layout assumptions.
                strokes = try value.map { try GuestKeyboard.character($0) }
            } else {
                guard let name = payload["key"] as? String else { throw HarborError("A key name is required") }
                strokes = [try GuestKeyboard.named(name)]
            }
            for (key, characters, flags) in strokes {
                let modifiers: [(NSEvent.ModifierFlags, UInt16)] = [(.control, 59), (.option, 58), (.shift, 56), (.command, 55)]
                var held: [(NSEvent.ModifierFlags, UInt16)] = []
                defer { for (flag, code) in held.reversed() { try? modifierEvent(code: code, flags: flag, down: false) } }
                for (flag, code) in modifiers where flags.contains(flag) {
                    try modifierEvent(code: code, flags: flag, down: true)
                    held.append((flag, code))
                }
                for type in [NSEvent.EventType.keyDown, .keyUp] {
                    guard let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: view.window?.windowNumber ?? 0, context: nil, characters: characters, charactersIgnoringModifiers: characters.lowercased(), isARepeat: false, keyCode: key) else { throw HarborError("Could not construct guest key event") }
                    if type == .keyDown { view.keyDown(with: event) } else { view.keyUp(with: event) }
                }
            }
        }
    }
    private func modifierEvent(code: UInt16, flags: NSEvent.ModifierFlags, down: Bool) throws {
        guard let event = NSEvent.keyEvent(with: down ? .keyDown : .keyUp, location: .zero, modifierFlags: down ? flags : [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: view.window?.windowNumber ?? 0, context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: code) else { throw HarborError("Could not construct guest modifier event") }
        if down { view.keyDown(with: event) } else { view.keyUp(with: event) }
    }
}
