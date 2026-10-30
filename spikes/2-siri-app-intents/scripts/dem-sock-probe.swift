#!/usr/bin/env swift
// Проба моста Swift → dem.sock (спайк #100). Не App Intent — только IPC + текст диалога.
import Foundation

struct SockRequest: Encodable {
    let action: String
    let root: String
    let project: String?
    let json: Bool
}

struct SockResponse: Decodable {
    let output: String
    let error: String?
}

func demSockPath() -> String {
    if let override = ProcessInfo.processInfo.environment["DEMERZEL_RUNTIME_DIR"], !override.isEmpty {
        return (override as NSString).appendingPathComponent("dem.sock")
    }
    let home = FileManager.default.homeDirectoryForCurrentUser.path
    return (home as NSString).appendingPathComponent(".orchestrator/runtime/dem.sock")
}

func callDemSock(action: String, root: String, project: String?) throws -> SockResponse {
    let path = demSockPath()
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else {
        throw NSError(domain: "dem.sock", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "socket() failed",
        ])
    }
    defer { close(fd) }

    var addr = sockaddr_un()
    addr.sun_family = sa_family_t(AF_UNIX)
    let pathBytes = path.utf8CString
    guard pathBytes.count <= MemoryLayout.size(ofValue: addr.sun_path) else {
        throw NSError(domain: "dem.sock", code: 2, userInfo: [
            NSLocalizedDescriptionKey: "path too long",
        ])
    }
    withUnsafeMutablePointer(to: &addr.sun_path.0) { ptr in
        pathBytes.withUnsafeBufferPointer { buf in
            ptr.update(from: buf.baseAddress!, count: buf.count)
        }
    }

    let connectResult = withUnsafePointer(to: &addr) {
        $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
            connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
        }
    }
    guard connectResult == 0 else {
        throw NSError(domain: "dem.sock", code: 3, userInfo: [
            NSLocalizedDescriptionKey: "connect failed: \(String(cString: strerror(errno)))",
        ])
    }

    let req = SockRequest(action: action, root: root, project: project, json: true)
    var payload = try JSONEncoder().encode(req)
    payload.append(0x0a)
    let written = payload.withUnsafeBytes { write(fd, $0.baseAddress, $0.count) }
    guard written == payload.count else {
        throw NSError(domain: "dem.sock", code: 4, userInfo: [
            NSLocalizedDescriptionKey: "write failed",
        ])
    }

    var buf = [UInt8](repeating: 0, count: 65536)
    let n = read(fd, &buf, buf.count)
    guard n > 0 else {
        throw NSError(domain: "dem.sock", code: 5, userInfo: [
            NSLocalizedDescriptionKey: "empty response",
        ])
    }
    return try JSONDecoder().decode(SockResponse.self, from: Data(buf.prefix(n)))
}

func formatDialog(project: String?, output: String) -> String {
    guard let data = output.data(using: .utf8),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let summary = obj["summary"] as? String
    else {
        return output
    }
    let name = (obj["project"] as? String) ?? (project ?? "проект")
    var dialog = "\(name): \(summary)"
    if let eta = obj["eta_seconds"] as? Int {
        if eta < 60 {
            dialog += ", ещё \(eta) с"
        } else if eta < 3600 {
            dialog += ", ещё \((eta + 59) / 60) мин"
        } else {
            dialog += ", ещё \((eta + 3599) / 3600) ч"
        }
    }
    return dialog
}

let args = CommandLine.arguments
let root = args.count > 1 ? args[1] : FileManager.default.currentDirectoryPath
let project = args.count > 2 ? args[2] : nil
do {
    let resp = try callDemSock(action: "pulse", root: root, project: project)
    if let err = resp.error, !err.isEmpty {
        fputs("error: \(err)\n", stderr)
        exit(1)
    }
    print("DIALOG: \(formatDialog(project: project, output: resp.output))")
    print("RAW: \(resp.output)")
} catch {
    fputs("\(error)\n", stderr)
    exit(1)
}
