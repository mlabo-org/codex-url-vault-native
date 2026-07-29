import Foundation

@_silgen_name("url_vault_free_string")
private func ffiFreeString(_ value: UnsafeMutablePointer<CChar>?)
@_silgen_name("url_vault_init")
private func ffiInit(_ home: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_list_urls")
private func ffiListURLs(
    _ home: UnsafePointer<CChar>?,
    _ category: UnsafePointer<CChar>?,
    _ limit: UInt32
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_search_urls")
private func ffiSearchURLs(
    _ home: UnsafePointer<CChar>?,
    _ query: UnsafePointer<CChar>?,
    _ limit: UInt32
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_list_categories")
private func ffiListCategories(_ home: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_save_url")
private func ffiSaveURL(
    _ home: UnsafePointer<CChar>?,
    _ input: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_update_url")
private func ffiUpdateURL(
    _ home: UnsafePointer<CChar>?,
    _ target: UnsafePointer<CChar>?,
    _ changes: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_move_url")
private func ffiMoveURL(
    _ home: UnsafePointer<CChar>?,
    _ target: UnsafePointer<CChar>?,
    _ category: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_archive_url")
private func ffiArchiveURL(
    _ home: UnsafePointer<CChar>?,
    _ target: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_preview_import")
private func ffiPreviewImport(
    _ home: UnsafePointer<CChar>?,
    _ html: UnsafePointer<CChar>?,
    _ fileName: UnsafePointer<CChar>?,
    _ stripCommonRoot: Bool
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_apply_import")
private func ffiApplyImport(
    _ home: UnsafePointer<CChar>?,
    _ input: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_get_snapshot")
private func ffiGetSnapshot(
    _ home: UnsafePointer<CChar>?,
    _ target: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("url_vault_open_url")
private func ffiOpenURL(
    _ home: UnsafePointer<CChar>?,
    _ target: UnsafePointer<CChar>?,
    _ browser: UnsafePointer<CChar>?,
    _ dryRun: Bool
) -> UnsafeMutablePointer<CChar>?

private struct FFIEnvelope<Value: Decodable>: Decodable {
    let ok: Bool
    let data: Value?
    let error: String?
}

enum VaultBridgeError: LocalizedError {
    case noResponse
    case native(String)

    var errorDescription: String? {
        switch self {
        case .noResponse:
            return "The native Vault core returned no response."
        case .native(let message):
            return message
        }
    }
}

enum RustVaultBridge {
    private static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()

    private static let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        return encoder
    }()

    static func initialize() throws -> VaultInitResult {
        try decode(ffiInit(nil))
    }

    static func listURLs(category: String?) throws -> [VaultBookmark] {
        if let category {
            return try category.withCString { pointer in
                try decode(ffiListURLs(nil, pointer, 0))
            }
        }
        return try decode(ffiListURLs(nil, nil, 0))
    }

    static func searchURLs(query: String, limit: UInt32 = 100) throws -> [VaultBookmark] {
        try query.withCString { pointer in
            try decode(ffiSearchURLs(nil, pointer, limit))
        }
    }

    static func listCategories() throws -> [VaultCategory] {
        try decode(ffiListCategories(nil))
    }

    static func saveURL(_ request: SaveURLRequest) throws -> VaultMutation<VaultBookmark> {
        try invokeJSON(request, ffiSaveURL)
    }

    static func updateURL(
        target: String,
        changes: UpdateURLRequest
    ) throws -> VaultMutation<VaultBookmark> {
        let json = try encode(changes)
        return try target.withCString { targetPointer in
            try json.withCString { jsonPointer in
                try decode(ffiUpdateURL(nil, targetPointer, jsonPointer))
            }
        }
    }

    static func moveURL(
        target: String,
        category: String
    ) throws -> VaultMutation<VaultBookmark> {
        try target.withCString { targetPointer in
            try category.withCString { categoryPointer in
                try decode(ffiMoveURL(nil, targetPointer, categoryPointer))
            }
        }
    }

    static func archiveURL(target: String) throws -> VaultMutation<VaultBookmark> {
        try target.withCString { pointer in
            try decode(ffiArchiveURL(nil, pointer))
        }
    }

    static func previewImport(
        html: String,
        fileName: String,
        stripCommonRoot: Bool
    ) throws -> VaultImportPreview {
        try html.withCString { htmlPointer in
            try fileName.withCString { filePointer in
                try decode(ffiPreviewImport(nil, htmlPointer, filePointer, stripCommonRoot))
            }
        }
    }

    static func applyImport(_ request: ApplyImportRequest) throws -> VaultImportResult {
        try invokeJSON(request, ffiApplyImport)
    }

    static func getSnapshot(target: String) throws -> VaultSnapshotPayload {
        try target.withCString { pointer in
            try decode(ffiGetSnapshot(nil, pointer))
        }
    }

    static func openURL(target: String, browser: String? = nil) throws -> VaultOpenResult {
        try target.withCString { targetPointer in
            if let browser {
                return try browser.withCString { browserPointer in
                    try decode(ffiOpenURL(nil, targetPointer, browserPointer, false))
                }
            }
            return try decode(ffiOpenURL(nil, targetPointer, nil, false))
        }
    }

    private static func invokeJSON<Request: Encodable, Response: Decodable>(
        _ request: Request,
        _ operation: (
            UnsafePointer<CChar>?,
            UnsafePointer<CChar>?
        ) -> UnsafeMutablePointer<CChar>?
    ) throws -> Response {
        let json = try encode(request)
        return try json.withCString { pointer in
            try decode(operation(nil, pointer))
        }
    }

    private static func encode<Value: Encodable>(_ value: Value) throws -> String {
        let data = try encoder.encode(value)
        guard let json = String(data: data, encoding: .utf8) else {
            throw VaultBridgeError.native("Could not encode a native Vault request.")
        }
        return json
    }

    private static func decode<Value: Decodable>(
        _ pointer: UnsafeMutablePointer<CChar>?
    ) throws -> Value {
        guard let pointer else {
            throw VaultBridgeError.noResponse
        }
        defer { ffiFreeString(pointer) }
        let data = Data(String(cString: pointer).utf8)
        let envelope = try decoder.decode(FFIEnvelope<Value>.self, from: data)
        guard envelope.ok, let value = envelope.data else {
            throw VaultBridgeError.native(envelope.error ?? "The native Vault operation failed.")
        }
        return value
    }
}
