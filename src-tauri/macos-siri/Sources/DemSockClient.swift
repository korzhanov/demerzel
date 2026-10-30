import Foundation

/// Клиент локального API Demerzel (`~/.orchestrator/runtime/dem.sock`).
/// Тот же JSON-протокол, что CLI `dem pulse` / `dem restart` (#102 / #101).
public enum DemSockClient {
    public struct Request: Encodable, Sendable {
        public var action: String
        public var root: String
        public var project: String?
        public var json: Bool

        public init(action: String, root: String, project: String? = nil, json: Bool = true) {
            self.action = action
            self.root = root
            self.project = project
            self.json = json
        }
    }

    public struct Response: Decodable, Sendable {
        public var output: String
        public var error: String?
    }

    public struct PulseJSON: Decodable, Sendable {
        public var project: String?
        public var summary: String
        public var eta_seconds: Int?
        public var state: String?
    }

    public struct RestartJSON: Decodable, Sendable {
        public var project: String
        public var status: String
        public var id: UInt64?
    }

    public static func socketPath() -> String {
        if let override = ProcessInfo.processInfo.environment["DEMERZEL_RUNTIME_DIR"], !override.isEmpty {
            return (override as NSString).appendingPathComponent("dem.sock")
        }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return (home as NSString).appendingPathComponent(".orchestrator/runtime/dem.sock")
    }

    public static func call(_ request: Request) throws -> Response {
        let path = socketPath()
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw DemSockError.socketCreate
        }
        defer { close(fd) }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = path.utf8CString
        guard pathBytes.count <= MemoryLayout.size(ofValue: addr.sun_path) else {
            throw DemSockError.pathTooLong
        }
        withUnsafeMutablePointer(to: &addr.sun_path.0) { ptr in
            pathBytes.withUnsafeBufferPointer { buf in
                ptr.update(from: buf.baseAddress!, count: buf.count)
            }
        }

        let ok = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard ok == 0 else { throw DemSockError.connectFailed }

        var payload = try JSONEncoder().encode(request)
        payload.append(0x0a)
        let written = payload.withUnsafeBytes { write(fd, $0.baseAddress, $0.count) }
        guard written == payload.count else { throw DemSockError.writeFailed }

        var buf = [UInt8](repeating: 0, count: 65536)
        let n = read(fd, &buf, buf.count)
        guard n > 0 else { throw DemSockError.emptyResponse }
        return try JSONDecoder().decode(Response.self, from: Data(buf.prefix(n)))
    }

    public static func pulseDialog(root: String, project: String) throws -> (dialog: String, pulse: PulseJSON) {
        let resp = try call(Request(action: "pulse", root: root, project: project, json: true))
        if let err = resp.error, !err.isEmpty {
            throw DemSockError.server(err)
        }
        let pulse = try JSONDecoder().decode(PulseJSON.self, from: Data(resp.output.utf8))
        let name = pulse.project ?? project
        var dialog = "\(name): \(pulse.summary)"
        if let eta = pulse.eta_seconds {
            dialog += ", \(Self.formatEtaRu(eta))"
        }
        return (dialog, pulse)
    }

    public static func restartDialog(root: String, project: String) throws -> (dialog: String, restart: RestartJSON) {
        let resp = try call(Request(action: "restart", root: root, project: project, json: true))
        if let err = resp.error, !err.isEmpty {
            throw DemSockError.server(err)
        }
        let restart = try JSONDecoder().decode(RestartJSON.self, from: Data(resp.output.utf8))
        let dialog = "\(restart.project): \(restart.status)"
        return (dialog, restart)
    }

    public static func formatEtaRu(_ secs: Int) -> String {
        if secs < 60 { return "ещё \(secs) с" }
        if secs < 3600 { return "ещё \((secs + 59) / 60) мин" }
        return "ещё \((secs + 3599) / 3600) ч"
    }
}

public enum DemSockError: Error, LocalizedError {
    case socketCreate
    case pathTooLong
    case connectFailed
    case writeFailed
    case emptyResponse
    case server(String)

    public var errorDescription: String? {
        switch self {
        case .socketCreate: return "не удалось создать Unix-сокет"
        case .pathTooLong: return "путь dem.sock слишком длинный"
        case .connectFailed: return "нет supervisor (dem.sock)"
        case .writeFailed: return "запись в dem.sock не удалась"
        case .emptyResponse: return "пустой ответ dem.sock"
        case .server(let msg): return msg
        }
    }
}
