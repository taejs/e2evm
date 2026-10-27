import Foundation
import Darwin
import Virtualization

/// The CLI launches this mode in an isolated process while holding the profile lease.
/// Installation writes only fresh artifacts in its private staging directory.
final class MacOSProvisioner: NSObject, URLSessionDownloadDelegate {
    private static var active: MacOSProvisioner?
    private let directory: String
    private let cpus: Int
    private let memory: UInt64
    private let diskBytes: Int64
    private var session: URLSession?
    private var installer: VZMacOSInstaller?
    private var machine: VZVirtualMachine?
    private var timer: Timer?
    private var downloadedImage: URL?
    private var lastDownloadPercent = -1

    private init(directory: String, cpus: Int, memory: UInt64, diskBytes: Int64) {
        self.directory = directory; self.cpus = cpus; self.memory = memory; self.diskBytes = diskBytes
    }

    static func runIfRequested(arguments: [String]) -> Bool {
        guard arguments.dropFirst().first == "--prepare-macos" else { return false }
        do {
            guard arguments.count >= 3 else { throw HarborError("A private macOS staging directory is required") }
            let directory = arguments[2]
            guard directory.hasPrefix("/") else { throw HarborError("macOS staging directory must be absolute") }
            try requirePrivateDirectory(directory)
            var options: [String: String] = [:]
            let allowed = Set(["--cpus", "--memory-mib", "--disk-gib", "--restore-image"])
            var index = 3
            while index < arguments.count {
                let key = arguments[index]
                guard allowed.contains(key), options[key] == nil, index + 1 < arguments.count else {
                    throw HarborError("Invalid macOS preparation arguments")
                }
                options[key] = arguments[index + 1]; index += 2
            }
            guard let cpus = Int(options["--cpus"] ?? "2"), (2...32).contains(cpus),
                  let memoryMiB = Int(options["--memory-mib"] ?? "4096"), (4096...65536).contains(memoryMiB),
                  let diskGiB = Int(options["--disk-gib"] ?? "64"), (64...512).contains(diskGiB) else {
                throw HarborError("macOS requires 2–32 CPUs, 4096–65536 MiB RAM, and a 64–512 GiB disk")
            }
            for name in ["disk.img", "hardware-model.bin", "machine-identifier.bin", "auxiliary-storage", "restore.ipsw"] {
                var info = stat()
                guard lstat(directory + "/" + name, &info) != 0, errno == ENOENT else {
                    throw HarborError("macOS preparation requires fresh artifacts; \(name) already exists")
                }
            }
            guard VZVirtualMachine.isSupported else { throw HarborError("Virtualization is unavailable on this Mac") }
            let provisioner = MacOSProvisioner(directory: directory, cpus: cpus, memory: UInt64(memoryMiB) * 1024 * 1024, diskBytes: Int64(diskGiB) * 1024 * 1024 * 1024)
            active = provisioner
            DispatchQueue.main.async { provisioner.begin(localImage: options["--restore-image"]) }
        } catch { fail(error) }
        return true
    }

    private static func report(_ text: String) {
        FileHandle.standardError.write(Data((text + "\n").utf8))
    }
    private static func fail(_ error: Error) -> Never {
        report("macOS preparation failed: " + error.localizedDescription)
        exit(1)
    }

    private func begin(localImage: String?) {
        if let localImage {
            do {
                guard localImage.hasPrefix("/") else { throw HarborError("Restore image path must be absolute") }
                try requireRegularFile(localImage)
                loadImage(URL(fileURLWithPath: localImage))
            } catch { Self.fail(error) }
        } else {
            Self.report("Finding the latest macOS restore image supported by this Mac…")
            VZMacOSRestoreImage.fetchLatestSupported { result in
                DispatchQueue.main.async {
                    switch result {
                    case .success(let image): self.download(image.url)
                    case .failure(let error): Self.fail(error)
                    }
                }
            }
        }
    }

    private static func isAppleDownload(_ url: URL) -> Bool {
        guard url.scheme == "https", url.user == nil, url.password == nil,
              let host = url.host?.lowercased() else { return false }
        return host == "apple.com" || host.hasSuffix(".apple.com") || host.hasSuffix(".cdn-apple.com")
    }

    private func download(_ url: URL) {
        guard Self.isAppleDownload(url) else { Self.fail(HarborError("Apple returned an unexpected restore-image URL")) }
        Self.report("Downloading macOS directly from Apple. This is a large download; installation follows automatically.")
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 120
        configuration.timeoutIntervalForResource = 4 * 60 * 60
        configuration.httpCookieStorage = nil
        configuration.urlCredentialStorage = nil
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
        self.session = session
        session.downloadTask(with: url).resume()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(request.url.map(Self.isAppleDownload) == true ? request : nil)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        guard totalBytesExpectedToWrite > 0 else { return }
        let percent = Int(Double(totalBytesWritten) / Double(totalBytesExpectedToWrite) * 100)
        if percent / 5 > lastDownloadPercent / 5 || lastDownloadPercent < 0 {
            lastDownloadPercent = percent
            Self.report("Downloading macOS: \(percent)%")
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        do {
            guard let response = downloadTask.response as? HTTPURLResponse, response.statusCode == 200,
                  response.url.map(Self.isAppleDownload) == true else { throw HarborError("Apple restore-image download did not succeed") }
            let destination = URL(fileURLWithPath: directory + "/restore.ipsw")
            // copyItem refuses to replace an existing destination, including a symlink.
            try FileManager.default.copyItem(at: location, to: destination)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: destination.path)
            DispatchQueue.main.async {
                self.downloadedImage = destination
                self.loadImage(destination)
            }
        } catch { Self.fail(error) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if error != nil { Self.fail(HarborError("The macOS download failed. Check the connection and retry with a fresh staging directory.")) }
    }

    private func loadImage(_ url: URL) {
        Self.report("Checking macOS restore image compatibility…")
        VZMacOSRestoreImage.load(from: url) { result in
            DispatchQueue.main.async {
                do {
                    let image = try result.get()
                    try self.install(image: image, url: url)
                } catch { Self.fail(error) }
            }
        }
    }

    private func install(image: VZMacOSRestoreImage, url: URL) throws {
        guard let requirements = image.mostFeaturefulSupportedConfiguration,
              requirements.hardwareModel.isSupported else { throw HarborError("This macOS restore image is not supported by this Mac") }
        guard cpus >= requirements.minimumSupportedCPUCount, memory >= requirements.minimumSupportedMemorySize else {
            throw HarborError("This restore image requires at least \(requirements.minimumSupportedCPUCount) CPUs and \(requirements.minimumSupportedMemorySize / 1024 / 1024) MiB RAM; increase the profile resources")
        }
        let diskPath = directory + "/disk.img"
        let descriptor = open(diskPath, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard descriptor >= 0 else { throw HarborError("Cannot create a fresh macOS disk") }
        defer { close(descriptor) }
        guard ftruncate(descriptor, diskBytes) == 0, fsync(descriptor) == 0 else { throw HarborError("Cannot allocate the macOS disk") }
        let hardware = requirements.hardwareModel
        let identity = VZMacMachineIdentifier()
        try Self.writeNew(hardware.dataRepresentation, path: directory + "/hardware-model.bin")
        try Self.writeNew(identity.dataRepresentation, path: directory + "/machine-identifier.bin")
        let auxiliary = try VZMacAuxiliaryStorage(creatingStorageAt: URL(fileURLWithPath: directory + "/auxiliary-storage"), hardwareModel: hardware, options: [])
        let platform = VZMacPlatformConfiguration()
        platform.hardwareModel = hardware; platform.machineIdentifier = identity; platform.auxiliaryStorage = auxiliary
        let configuration = VZVirtualMachineConfiguration()
        configuration.platform = platform
        configuration.bootLoader = VZMacOSBootLoader()
        configuration.cpuCount = cpus; configuration.memorySize = memory
        let attachment = try VZDiskImageStorageDeviceAttachment(url: URL(fileURLWithPath: diskPath), readOnly: false, cachingMode: .cached, synchronizationMode: .fsync)
        configuration.storageDevices = [VZVirtioBlockDeviceConfiguration(attachment: attachment)]
        configuration.graphicsDevices = [Self.graphics()]
        configuration.keyboards = [VZMacKeyboardConfiguration()]
        configuration.pointingDevices = [VZUSBScreenCoordinatePointingDeviceConfiguration()]
        let network = VZVirtioNetworkDeviceConfiguration()
        network.attachment = VZNATNetworkDeviceAttachment()
        configuration.networkDevices = [network]
        try configuration.validate()
        let machine = VZVirtualMachine(configuration: configuration)
        self.machine = machine
        let installer = VZMacOSInstaller(virtualMachine: machine, restoringFromImageAt: url)
        self.installer = installer
        Self.report("Installing macOS into the new guest disk…")
        timer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
            guard let installer = self?.installer else { return }
            Self.report("Installing macOS: \(Int(installer.progress.fractionCompleted * 100))%")
        }
        installer.install { result in
            self.timer?.invalidate()
            switch result {
            case .success:
                if let image = self.downloadedImage { try? FileManager.default.removeItem(at: image) }
                Self.report("macOS installed. Start the guest and complete Apple's Setup Assistant, then install the guest connection kit.")
                exit(0)
            case .failure(let error): Self.fail(error)
            }
        }
    }

    private static func writeNew(_ data: Data, path: String) throws {
        let descriptor = open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard descriptor >= 0 else { throw HarborError("Cannot create a fresh macOS identity file") }
        let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        try handle.write(contentsOf: data)
        try handle.synchronize()
        try handle.close()
    }

    private static func readIdentity(_ path: String) throws -> Data {
        let descriptor = open(path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        guard descriptor >= 0 else { throw HarborError("Cannot read macOS guest identity") }
        let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        defer { try? handle.close() }
        var info = stat()
        guard fstat(descriptor, &info) == 0, info.st_mode & S_IFMT == S_IFREG,
              info.st_uid == getuid(), info.st_size > 0, info.st_size <= 1024 * 1024 else {
            throw HarborError("Invalid macOS guest identity file")
        }
        guard let data = try handle.read(upToCount: 1024 * 1024 + 1), data.count <= 1024 * 1024 else {
            throw HarborError("macOS guest identity exceeds the size limit")
        }
        return data
    }

    static func loadPlatform(directory: String) throws -> VZMacPlatformConfiguration {
        for name in ["hardware-model.bin", "machine-identifier.bin", "auxiliary-storage"] {
            try requireRegularFile(directory + "/" + name)
        }
        guard let hardware = VZMacHardwareModel(dataRepresentation: try readIdentity(directory + "/hardware-model.bin")), hardware.isSupported,
              let identity = VZMacMachineIdentifier(dataRepresentation: try readIdentity(directory + "/machine-identifier.bin")) else {
            throw HarborError("macOS guest identity is invalid or unsupported by this Mac")
        }
        let platform = VZMacPlatformConfiguration()
        platform.hardwareModel = hardware; platform.machineIdentifier = identity
        platform.auxiliaryStorage = VZMacAuxiliaryStorage(url: URL(fileURLWithPath: directory + "/auxiliary-storage"))
        return platform
    }

    static func graphics() -> VZMacGraphicsDeviceConfiguration {
        let graphics = VZMacGraphicsDeviceConfiguration()
        graphics.displays = [VZMacGraphicsDisplayConfiguration(widthInPixels: 1440, heightInPixels: 900, pixelsPerInch: 80)]
        return graphics
    }
}
