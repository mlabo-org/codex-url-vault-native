import Darwin
import Foundation

enum VaultDataChangeMonitorError: LocalizedError {
    case cannotWatchDirectory(URL, Int32)

    var errorDescription: String? {
        switch self {
        case .cannotWatchDirectory(let url, let errorCode):
            return "Could not watch the Vault directory at \(url.path): \(String(cString: strerror(errorCode)))"
        }
    }
}

/// Observes committed Vault mutations without polling. The Rust core atomically
/// replaces `current-bookmarks.html` after every successful content mutation, so
/// its file identity is the completion signal shared by every writer.
final class VaultDataChangeMonitor: @unchecked Sendable {
    private struct FileFingerprint: Equatable {
        let fileNumber: UInt64
        let size: UInt64
        let modifiedAt: Date
    }

    private let canonicalHTMLURL: URL
    private let directoryURL: URL
    private let debounceInterval: DispatchTimeInterval
    private let changeHandler: @Sendable () -> Void
    private let queue = DispatchQueue(label: "com.suzukimakoto.codex-url-vault.data-change-monitor")

    private var source: DispatchSourceFileSystemObject?
    private var pendingChange: DispatchWorkItem?
    private var lastFingerprint: FileFingerprint?

    init(
        canonicalHTMLURL: URL,
        debounceInterval: DispatchTimeInterval = .milliseconds(200),
        changeHandler: @escaping @Sendable () -> Void
    ) {
        self.canonicalHTMLURL = canonicalHTMLURL
        self.directoryURL = canonicalHTMLURL.deletingLastPathComponent()
        self.debounceInterval = debounceInterval
        self.changeHandler = changeHandler
        self.lastFingerprint = Self.fingerprint(for: canonicalHTMLURL)
    }

    func start() throws {
        guard source == nil else { return }

        let fileDescriptor = open(directoryURL.path, O_EVTONLY)
        guard fileDescriptor >= 0 else {
            throw VaultDataChangeMonitorError.cannotWatchDirectory(directoryURL, errno)
        }

        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fileDescriptor,
            eventMask: [.write, .delete, .rename],
            queue: queue
        )
        source.setEventHandler { [weak self] in
            self?.scheduleChangeCheck()
        }
        source.setCancelHandler {
            close(fileDescriptor)
        }
        self.source = source
        source.resume()
    }

    func acknowledgeCurrentState() {
        let fingerprint = Self.fingerprint(for: canonicalHTMLURL)
        queue.async { [weak self] in
            self?.lastFingerprint = fingerprint
            self?.pendingChange?.cancel()
            self?.pendingChange = nil
        }
    }

    func stop() {
        pendingChange?.cancel()
        pendingChange = nil
        source?.cancel()
        source = nil
    }

    deinit {
        stop()
    }

    private func scheduleChangeCheck() {
        pendingChange?.cancel()
        let workItem = DispatchWorkItem { [weak self] in
            self?.reportChangeIfNeeded()
        }
        pendingChange = workItem
        queue.asyncAfter(deadline: .now() + debounceInterval, execute: workItem)
    }

    private func reportChangeIfNeeded() {
        pendingChange = nil
        let fingerprint = Self.fingerprint(for: canonicalHTMLURL)
        guard fingerprint != lastFingerprint else { return }
        lastFingerprint = fingerprint
        changeHandler()
    }

    private static func fingerprint(for url: URL) -> FileFingerprint? {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
              let fileNumber = attributes[.systemFileNumber] as? NSNumber,
              let size = attributes[.size] as? NSNumber,
              let modifiedAt = attributes[.modificationDate] as? Date else {
            return nil
        }
        return FileFingerprint(
            fileNumber: fileNumber.uint64Value,
            size: size.uint64Value,
            modifiedAt: modifiedAt
        )
    }
}
