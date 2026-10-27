import AppKit
import Darwin
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
    let guest_os: String?
    var guestOS: String { guest_os ?? "linux" }
    let assistant_program: String?
    let assistant_connection: AssistantConnection?
    var connection: AssistantConnection { assistant_connection ?? .codex }
    var directory: String?
}

struct ConversationMessage: Identifiable {
    let id = UUID()
    let role: String
    var text: String
    var failed = false
}

final class ProfileSession: ObservableObject {
    @Published var command = ""
    @Published var console = ""
    @Published var commandBusy = false
    @Published var commands: [String] = []
    @Published var prompt = ""
    @Published var messages: [ConversationMessage] = []
    @Published var chatBusy = false
    // Credentials and conversation exist only for this app session, never in profile JSON.
    @Published var apiKey = ""
    var history: [[String: String]] = []
    @Published var action: String?
    @Published var progress = ""
    @Published var guestOnline = false
    @Published var consoleVisible = true
}

/// Each pipe has its own reader. Child processes must never block the AppKit queue.
/// Captured bytes are bounded independently of the producer's output volume.
private final class CLIBytes {
    private let lock = NSLock()
    private var data = Data()
    private var clipped = false
    private var deliveryPending = false
    func append(_ chunk: Data) {
        lock.lock(); defer { lock.unlock() }
        let capacity = 8 * 1024 * 1024
        if data.count + chunk.count > capacity { clipped = true }
        data.append(chunk.prefix(max(0, capacity - data.count)))
    }
    func deliver(_ update: @escaping (String) -> Void) {
        lock.lock()
        guard !deliveryPending else { lock.unlock(); return }
        deliveryPending = true; lock.unlock()
        DispatchQueue.main.async { [self] in
            lock.lock()
            let text = String(decoding: data.suffix(256 * 1024), as: UTF8.self)
            deliveryPending = false; lock.unlock()
            update(text)
        }
    }
    func result() -> (Data, Bool) {
        lock.lock(); defer { lock.unlock() }
        return (data, clipped)
    }
}

private final class CLIJob {
    let process = Process()
    var cancelled = false
    func cancel() {
        cancelled = true
        if process.isRunning { process.terminate() }
    }
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
    private var sessions: [String: ProfileSession] = [:]
    private var jobs: [String: CLIJob] = [:]
    private var polling = false
    private var terminationCompletion: (() -> Void)?
    @Published var creating = false
    @Published private(set) var shuttingDown = false

    func session(_ id: String) -> ProfileSession {
        if let existing = sessions[id] { return existing }
        let value = ProfileSession(); sessions[id] = value; return value
    }

    /// Only the conversation the user sees is shared. Guest state is never attached.
    func ask(_ id: String) {
        let state = session(id), prompt = session(id).prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !state.chatBusy, !prompt.isEmpty else { return }
        let userMessage = ["role": "user", "content": prompt]
        guard let expected = expectedAssistant(id) else { errorMessage = "Refresh assistant settings before sending."; return }
        var request: [String: Any] = ["messages": state.history + [userMessage], "expected": expected]
        if !state.apiKey.isEmpty { request["api_key"] = state.apiKey }
        guard let input = try? JSONSerialization.data(withJSONObject: request), input.count <= 64 * 1024 else {
            errorMessage = "This conversation exceeds 64 KiB. Clear the conversation or shorten your message."; return
        }
        state.prompt = ""; state.chatBusy = true
        state.messages.append(ConversationMessage(role: "user", text: prompt))
        state.messages.append(ConversationMessage(role: "assistant", text: ""))
        let messageID = state.messages.last!.id
        launch(["chat-json", id], key: "chat:\(id)", input: input, output: { text in
            if let index = state.messages.firstIndex(where: { $0.id == messageID }) { state.messages[index].text = text }
        }) { [weak self] result in
            state.chatBusy = false
            if let index = state.messages.firstIndex(where: { $0.id == messageID }) {
                switch result {
                case .success(let data):
                    let text = String(decoding: data, as: UTF8.self)
                    state.messages[index].text = String(text.suffix(256 * 1024))
                    state.history += [userMessage, ["role": "assistant", "content": text]]
                case .failure(let error):
                    state.messages[index].text += (state.messages[index].text.isEmpty ? "" : "\n\n") + error.localizedDescription
                    state.messages[index].failed = true
                    self?.errorMessage = error.localizedDescription
                    self?.reload()
                }
            }
        }
    }

    private func expectedAssistant(_ id: String) -> [String: Any]? {
        guard let profile = profiles.first(where: { $0.id == id }),
              let data = try? JSONEncoder().encode(profile.connection),
              let connection = try? JSONSerialization.jsonObject(with: data) else { return nil }
        return ["connection": connection, "program": profile.assistant_program as Any? ?? NSNull()]
    }

    func clearConversation(_ id: String) {
        let state = session(id)
        guard !state.chatBusy else { return }
        state.messages = []; state.history = []
    }

    func cancelChat(_ id: String) { jobs["chat:\(id)"]?.cancel() }

    func runCommand(_ id: String) {
        let state = session(id), command = session(id).command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !state.commandBusy, !command.isEmpty else { return }
        state.command = ""; state.commandBusy = true
        state.commands.append(command)
        if state.commands.count > 50 { state.commands.removeFirst() }
        state.console += "\n$ \(command)\n"
        launch(["run", id, command], key: "command:\(id)", acceptFailureJSON: true) { result in
            state.commandBusy = false
            switch result {
            case .success(let data):
                if let value = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] {
                    state.console += (value["output"] as? String ?? "")
                    if let error = value["error"] as? String { state.console += "\n\(error)" }
                    if let code = value["exit_code"] as? Int { state.console += "\n[exit \(code)]\n" }
                } else { state.console += "Invalid response from guest command channel.\n" }
            case .failure(let error): state.console += "\(error.localizedDescription)\n"
            }
            state.console = String(state.console.suffix(256 * 1024))
        }
    }

    func request(_ action: String, id: String) {
        let state = session(id)
        guard state.action == nil else { return }
        state.action = action; state.progress = ""
        launch([action, id], key: "action:\(id)", diagnostic: { state.progress = $0 }) { [weak self] result in
            state.action = nil
            if case .failure(let error) = result { self?.errorMessage = error.localizedDescription }
            self?.reload(); self?.refreshStatus()
        }
    }

    func create(id: String, name: String, cpus: Int, memory: Int, guestOS: String = "linux", without: [String] = [], completion: @escaping (Bool) -> Void) {
        guard !creating else { return }; creating = true
        var arguments = ["profile", "create", id, "--name", name, "--cpus", String(cpus), "--memory", String(memory), "--os", guestOS]
        if !without.isEmpty { arguments += ["--without", without.joined(separator: ",")] }
        launch(arguments, key: "create") { [weak self] result in
            self?.creating = false
            switch result {
            case .success: self?.reload(); self?.selectedID = id; completion(true)
            case .failure(let error): self?.errorMessage = error.localizedDescription; completion(false)
            }
        }
    }

    func setConnection(id: String, connection: AssistantConnection, program: String, apiKey: String,
                       completion: @escaping (Bool) -> Void) {
        guard !session(id).chatBusy else { errorMessage = "Stop the current response before changing its connection."; completion(false); return }
        let previous = profiles.first { $0.id == id }
        var arguments = ["profile", "connection", id, connection.provider.rawValue]
        if let endpoint = connection.endpoint, !endpoint.isEmpty { arguments += ["--endpoint", endpoint] }
        if let model = connection.model, !model.isEmpty { arguments += ["--model", model] }
        if let keyEnv = connection.key_env, !keyEnv.isEmpty { arguments += ["--key-env", keyEnv] }
        let saveConnection = { [weak self] in
            guard let self else { completion(false); return }
            self.launch(arguments, key: "settings:\(id)") { [weak self] result in
                guard let self else { completion(false); return }
                switch result {
                case .success:
                    if previous?.connection != connection || (connection.provider == .codex && previous?.assistant_program != program) {
                        self.clearConversation(id)
                    }
                    self.reload()
                    guard let current = self.profiles.first(where: { $0.id == id }),
                          current.connection == connection,
                          connection.provider != .codex || current.assistant_program == program else {
                        self.session(id).apiKey = ""
                        self.clearConversation(id)
                        self.errorMessage = "Assistant connection changed while saving. Review its settings before sending."
                        completion(false); return
                    }
                    self.session(id).apiKey = apiKey
                    completion(true)
                case .failure(let error): self.errorMessage = error.localizedDescription; completion(false)
                }
            }
        }
        if connection.provider == .codex {
            launch(["profile", "assistant", id, program], key: "program:\(id)") { [weak self] result in
                switch result {
                case .success: saveConnection()
                case .failure(let error): self?.errorMessage = error.localizedDescription; completion(false)
                }
            }
        } else { saveConnection() }
    }

    func fetchModels(_ id: String, completion: @escaping ([String]?) -> Void) {
        guard let expected = expectedAssistant(id) else { errorMessage = "Refresh assistant settings before loading models."; completion(nil); return }
        let apiKey = session(id).apiKey
        var request: [String: Any] = ["expected": expected]
        if !apiKey.isEmpty { request["api_key"] = apiKey }
        let input = try? JSONSerialization.data(withJSONObject: request)
        launch(["assistant", "models-json", id], key: "models:\(id)", input: input) { [weak self] result in
            switch result {
            case .success(let data):
                guard let models = (try? JSONSerialization.jsonObject(with: data)) as? [[String: String]] else {
                    self?.errorMessage = "The provider returned an invalid model list."; completion(nil); return
                }
                completion(models.compactMap { $0["id"] }.sorted())
            case .failure(let error): self?.errorMessage = error.localizedDescription; self?.reload(); completion(nil)
            }
        }
    }

    func refreshStatus() {
        guard !polling, let id = selectedID else { return }; polling = true
        launch(["status", id], key: "status") { [weak self] result in
            self?.polling = false
            if case .success(let data) = result,
               let value = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] {
                self?.session(id).guestOnline = value["guest_online"] as? Bool ?? false
            } else { self?.session(id).guestOnline = false }
        }
    }

    var preparing: Bool { sessions.values.contains { $0.action == "prepare" } }
    func finishProcesses(_ completion: @escaping () -> Void) {
        shuttingDown = true
        timer?.invalidate()
        terminationCompletion = completion
        jobs.values.forEach { $0.cancel() }
        if jobs.isEmpty { terminationCompletion = nil; completion() }
    }

    private func launch(_ arguments: [String], key: String, acceptFailureJSON: Bool = false, input: Data? = nil,
                        output: ((String) -> Void)? = nil, diagnostic: ((String) -> Void)? = nil,
                        completion: @escaping (Result<Data, Error>) -> Void) {
        guard !shuttingDown else { completion(.failure(HarborError("Workspace is shutting down; new operations are unavailable."))); return }
        guard jobs[key] == nil else { completion(.failure(HarborError("This operation is already running."))); return }
        let job = CLIJob(), stdout = Pipe(), stderr = Pipe()
        job.process.executableURL = URL(fileURLWithPath: cli); job.process.arguments = arguments
        var environment = ProcessInfo.processInfo.environment; environment["E2EVM_HOME"] = root; environment["HARBORVM_HOME"] = root
        job.process.environment = environment
        let stdin = input == nil ? nil : Pipe()
        job.process.standardInput = stdin ?? FileHandle.nullDevice as Any
        job.process.standardOutput = stdout; job.process.standardError = stderr
        do { try job.process.run() } catch { completion(.failure(error)); return }
        try? stdin?.fileHandleForReading.close()
        jobs[key] = job
        let group = DispatchGroup(), out = CLIBytes(), err = CLIBytes()
        if let stdin, let input {
            // Do not block the UI while the child reads. Early exit must not raise SIGPIPE.
            _ = fcntl(stdin.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)
            group.enter()
            DispatchQueue.global(qos: .userInitiated).async {
                defer { try? stdin.fileHandleForWriting.close(); group.leave() }
                try? stdin.fileHandleForWriting.write(contentsOf: input)
            }
        }
        for (pipe, buffer, update) in [(stdout, out, output), (stderr, err, diagnostic)] {
            group.enter()
            DispatchQueue.global(qos: .userInitiated).async {
                while true {
                    let chunk = pipe.fileHandleForReading.availableData
                    if chunk.isEmpty { break }
                    buffer.append(chunk)
                    if let update { buffer.deliver(update) }
                }
                try? pipe.fileHandleForReading.close()
                group.leave()
            }
        }
        DispatchQueue.global(qos: .userInitiated).async {
            job.process.waitUntilExit(); group.wait()
            let (data, clipped) = out.result(), (failure, _) = err.result()
            let status = job.process.terminationStatus
            DispatchQueue.main.async { [weak self] in
                self?.jobs.removeValue(forKey: key)
                if job.cancelled { completion(.failure(HarborError("Cancelled."))) }
                else if clipped { completion(.failure(HarborError("Output exceeded the 8 MiB capture limit."))) }
                else if status != 0 && !(acceptFailureJSON && !data.isEmpty) {
                    let raw = String(decoding: failure, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                    let value = try? JSONSerialization.jsonObject(with: failure) as? [String: Any]
                    completion(.failure(HarborError(value?["error"] as? String ?? (raw.isEmpty ? "Command exited with status \(status)." : raw))))
                } else { completion(.success(data)) }
                if self?.jobs.isEmpty == true, let finished = self?.terminationCompletion {
                    self?.terminationCompletion = nil; finished()
                }
            }
        }
    }
    init(root: String, cli: String) {
        self.root = root; self.cli = cli
        reload()
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in self?.revision += 1; self?.refreshStatus() }
    }
    deinit { timer?.invalidate() }
    func runCLI(_ arguments: [String]) throws -> Data {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: cli)
        process.arguments = arguments
        var environment = ProcessInfo.processInfo.environment
        environment["E2EVM_HOME"] = root; environment["HARBORVM_HOME"] = root
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
            let updated = try JSONDecoder().decode([MachineProfile].self, from: runCLI(["profile", "list"]))
            for profile in updated {
                if let previous = profiles.first(where: { $0.id == profile.id }),
                   previous.connection != profile.connection || previous.assistant_program != profile.assistant_program,
                   let state = sessions[profile.id] {
                    cancelChat(profile.id)
                    state.messages = []; state.history = []; state.apiKey = ""
                }
            }
            profiles = updated
            if !profiles.contains(where: { $0.id == selectedID }) { selectedID = profiles.first?.id }
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
        guard !shuttingDown else { errorMessage = "Workspace is shutting down; new operations are unavailable."; return }
        do { try machine(id, refreshStopped: action == "start").perform(action) { [weak self] result in
            if case .failure(let error) = result { self?.errorMessage = error.localizedDescription }
            self?.revision += 1
        } } catch { errorMessage = error.localizedDescription }
    }
    func handle(_ request: [String: Any], completion: @escaping ([String: Any]) -> Void) {
        do {
            guard !shuttingDown else { throw HarborError("Workspace is shutting down; new operations are unavailable.") }
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
            if action == "quit" {
                guard !hasActiveMachines, !preparing else { throw HarborError("Stop active machines and finish disk preparation before quitting") }
                completion(["ok": true])
                // Finish this control request before beginning application cleanup.
                RunLoop.main.perform { NSApp.terminate(nil) }
                return
            }
            if action == "close-window" {
                (NSApp.delegate as? ApplicationDelegate)?.window?.performClose(nil)
                completion(["ok": true]); return
            }
            if action == "app-screenshot" {
                guard let view = NSApp.windows.first(where: { $0.isVisible })?.contentView,
                      let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { throw HarborError("No visible workspace window") }
                view.cacheDisplay(in: view.bounds, to: bitmap)
                completion(["ok": true, "png_base64": bitmap.representation(using: .png, properties: [:])!.base64EncodedString()]); return
            }
            let machine = try machine(id, refreshStopped: action == "start")
            switch action {
            case "status": completion(machine.status)
            case "select": selectedID = id; completion(machine.status)
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
