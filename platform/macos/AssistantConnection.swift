import Foundation

struct AssistantConnection: Codable, Equatable {
    enum Provider: String, CaseIterable, Identifiable, Codable {
        case codex
        case ollama
        case compatible = "openai-compatible"
        var id: String { rawValue }
        var title: String {
            switch self {
            case .codex: return "Codex CLI"
            case .ollama: return "Ollama"
            case .compatible: return "OpenAI-compatible API"
            }
        }
        var defaultEndpoint: String {
            self == .ollama ? "http://127.0.0.1:11434" : ""
        }
    }
    var provider: Provider
    var endpoint: String?
    var model: String?
    var key_env: String?
    static let codex = AssistantConnection(provider: .codex)
    var title: String { provider.title }
    var detail: String {
        provider == .codex ? "LOCAL CLI · HOST" : (model?.isEmpty == false ? model! : "Choose a model in settings")
    }
}
