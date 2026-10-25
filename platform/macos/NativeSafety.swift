import Foundation
import Darwin

struct HarborError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

final class FileLease {
    private var descriptor: Int32 = -1
    init(_ path: String, create: Bool = true) throws {
        let flags = O_RDWR | O_NOFOLLOW | O_CLOEXEC | (create ? O_CREAT : 0)
        descriptor = open(path, flags, 0o600)
        guard descriptor >= 0 else { throw HarborError("Cannot open lock: \(URL(fileURLWithPath: path).lastPathComponent)") }
        var metadata = stat()
        guard fstat(descriptor, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG,
              metadata.st_uid == getuid() else {
            close(descriptor); descriptor = -1
            throw HarborError("Resource must be a regular file owned by this user")
        }
        guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
            close(descriptor); descriptor = -1
            throw HarborError("Resource is already owned by another process")
        }
    }
    deinit { if descriptor >= 0 { flock(descriptor, LOCK_UN); close(descriptor) } }
}

func requirePrivateDirectory(_ path: String) throws {
    var metadata = stat()
    guard lstat(path, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFDIR,
          metadata.st_uid == getuid(), metadata.st_mode & 0o077 == 0 else {
        throw HarborError("Expected a private directory owned by this user: \(path)")
    }
}

func requireRegularFile(_ path: String) throws {
    var metadata = stat()
    guard lstat(path, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG,
          metadata.st_uid == getuid() else {
        throw HarborError("Expected a regular file owned by this user: \(URL(fileURLWithPath: path).lastPathComponent)")
    }
}

func requireTransition(_ action: String, state: String) throws {
    let allowed: [String: Set<String>] = ["start": ["stopped"], "pause": ["running"], "resume": ["paused"], "stop": ["running"], "poweroff": ["running", "paused", "stopping"]]
    guard allowed[action]?.contains(state) == true else {
        throw HarborError("Cannot \(action) a machine in state \(state)")
    }
}
