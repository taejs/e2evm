import AppKit

enum GuestKeyboard {
    typealias Stroke = (UInt16, String, NSEvent.ModifierFlags)
    private static let plain: [Character: UInt16] = ["a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29, "]": 30, "o": 31, "u": 32, "[": 33, "i": 34, "p": 35, "l": 37, "j": 38, "'": 39, "k": 40, ";": 41, "\\": 42, ",": 43, "/": 44, "n": 45, "m": 46, ".": 47, "`": 50, " ": 49, "\n": 36, "\r": 36, "\t": 48]
    private static let shifted: [Character: Character] = ["!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0", "_": "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";", "\"": "'", "<": ",", ">": ".", "?": "/", "~": "`"]
    static func character(_ value: Character) throws -> Stroke {
        if let code = plain[value] { return (code, String(value), []) }
        if let base = shifted[value], let code = plain[base] { return (code, String(value), .shift) }
        let lower = String(value).lowercased()
        if lower.count == 1, value.isASCII, let code = plain[Character(lower)], value.isUppercase { return (code, String(value), .shift) }
        throw HarborError("Text input supports a US ASCII keyboard; use guest exec for Unicode text")
    }
    static func named(_ name: String) throws -> Stroke {
        let parts = name.lowercased().split(separator: "+", omittingEmptySubsequences: false).map(String.init)
        guard let last = parts.last, !last.isEmpty else { throw HarborError("A key name is required") }
        var flags: NSEvent.ModifierFlags = []
        for modifier in parts.dropLast() {
            switch modifier {
            case "ctrl", "control": flags.insert(.control)
            case "alt", "option": flags.insert(.option)
            case "shift": flags.insert(.shift)
            case "cmd", "command": flags.insert(.command)
            default: throw HarborError("Unsupported guest key modifier")
            }
        }
        let keys: [String: (UInt16, String)] = ["enter": (36, "\r"), "return": (36, "\r"), "tab": (48, "\t"), "escape": (53, "\u{1b}"), "esc": (53, "\u{1b}"), "backspace": (51, "\u{8}"), "delete": (51, "\u{8}"), "up": (126, "\u{f700}"), "down": (125, "\u{f701}"), "left": (123, "\u{f702}"), "right": (124, "\u{f703}"), "space": (49, " ")]
        if let value = keys[last] { return (value.0, value.1, flags) }
        if last.count == 1, let value = last.first {
            let stroke = try character(value)
            return (stroke.0, stroke.1, stroke.2.union(flags))
        }
        throw HarborError("Unsupported guest key")
    }
}
