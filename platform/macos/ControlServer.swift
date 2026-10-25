import Foundation
import Darwin

final class ControlServer {
    private let socketFD: Int32
    private let lease: FileLease
    private let path: String
    private let queue = DispatchQueue(label: "HarborVM.control", qos: .userInitiated)
    private weak var model: WorkspaceModel?
    init(root: String, model: WorkspaceModel) throws {
        try requirePrivateDirectory(root)
        lease = try FileLease(root + "/app.lock")
        path = root + "/control.sock"
        self.model = model
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard path.utf8.count < capacity else { throw HarborError("Data directory is too long for the control socket") }
        // Only the process holding app.lock can remove its stale socket.
        var old = stat()
        if lstat(path, &old) == 0 {
            guard old.st_mode & S_IFMT == S_IFSOCK, old.st_uid == getuid() else { throw HarborError("Unsafe existing control socket") }
            guard unlink(path) == 0 else { throw HarborError("Cannot remove stale control socket") }
        }
        socketFD = socket(AF_UNIX, SOCK_STREAM, 0)
        guard socketFD >= 0 else { throw HarborError("Cannot create control socket") }
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { destination in
                _ = path.withCString { strlcpy(destination, $0, capacity) }
            }
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        let bound = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(socketFD, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard bound == 0, chmod(path, 0o600) == 0, listen(socketFD, 16) == 0 else { close(socketFD); throw HarborError("Cannot bind private control socket") }
    }
    func start() {
        queue.async { [self] in
            while true {
                let client = accept(socketFD, nil, nil)
                if client < 0 { if errno == EINTR { continue }; return }
                // A dedicated queue per connection keeps one waiting VM start from blocking status.
                DispatchQueue.global(qos: .userInitiated).async { [self] in serve(client) }
            }
        }
    }
    private func serve(_ client: Int32) {
        defer { close(client) }
        var uid: uid_t = 0, gid: gid_t = 0
        guard getpeereid(client, &uid, &gid) == 0, uid == getuid() else { return }
        var timeout = timeval(tv_sec: 10, tv_usec: 0)
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(client, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        var noSignal: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, socklen_t(MemoryLayout<Int32>.size))
        var data = Data(), buffer = [UInt8](repeating: 0, count: 4096)
        while !data.contains(10) {
            let count = read(client, &buffer, buffer.count)
            guard count > 0 else { return }
            data.append(contentsOf: buffer[0..<count])
            guard data.count <= 65536 else { reply(client, ["ok": false, "error": "Request exceeds 64 KiB"]); return }
        }
        guard let newline = data.firstIndex(of: 10),
              let request = (try? JSONSerialization.jsonObject(with: data.prefix(upTo: newline))) as? [String: Any],
              let requestID = request["id"] as? String, !requestID.isEmpty, requestID.utf8.count <= 128 else {
            reply(client, ["ok": false, "error": "Invalid JSON request or request ID"]); return
        }
        let completion = DispatchSemaphore(value: 0)
        let box = ResponseBox()
        DispatchQueue.main.async { [weak model] in
            guard let model else { box.value = ["ok": false, "error": "Workspace unavailable"]; completion.signal(); return }
            model.handle(request) { value in box.value = value; completion.signal() }
        }
        if completion.wait(timeout: .now() + 110) == .timedOut {
            reply(client, ["ok": false, "id": requestID, "error": "Operation outcome unknown; query status before retrying"])
        } else {
            var response = box.value
            response["id"] = requestID
            reply(client, response)
        }
    }
    private func reply(_ client: Int32, _ value: [String: Any]) {
        guard var data = try? JSONSerialization.data(withJSONObject: value), data.count <= 32 * 1024 * 1024 else { return }
        data.append(10)
        data.withUnsafeBytes { bytes in
            var offset = 0
            while offset < bytes.count {
                let count = write(client, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
                if count <= 0 { break }; offset += count
            }
        }
    }
}
private final class ResponseBox: @unchecked Sendable {
    var value: [String: Any] = [:]
}
