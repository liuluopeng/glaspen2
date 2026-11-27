import Foundation

/// 最小 protobuf wire 编解码, 只覆盖 map<string,string> StringMap。
/// 避免为两个 Swift-Rust 消息引入 SwiftProtobuf 依赖。
enum ProtoWire {
    static func encodeStringMap(_ map: [String: String]) -> [UInt8] {
        var out: [UInt8] = []
        for (key, value) in map {
            var entry: [UInt8] = []
            appendString(field: 1, key, to: &entry)
            appendString(field: 2, value, to: &entry)
            appendTag(field: 1, wire: 2, to: &out)
            appendVarint(UInt64(entry.count), to: &out)
            out.append(contentsOf: entry)
        }
        return out
    }

    static func encodeStringList(_ values: [String]) -> [UInt8] {
        var out: [UInt8] = []
        for value in values {
            appendString(field: 1, value, to: &out)
        }
        return out
    }

    static func decodeStringMap(_ bytes: [UInt8]) -> [String: String] {
        var result: [String: String] = [:]
        var i = 0
        while i < bytes.count {
            guard let tag = readVarint(bytes, &i) else { break }
            let field = Int(tag >> 3)
            let wire = Int(tag & 7)
            if wire == 2 {
                guard let len = readVarint(bytes, &i), len <= UInt64(bytes.count - i) else { break }
                let end = i + Int(len)
                if field == 1 {
                    var j = i
                    var key = ""
                    var value = ""
                    while j < end {
                        guard let inner = readVarint(bytes, &j) else { break }
                        let innerWire = Int(inner & 7)
                        if innerWire != 2 { j = skipField(bytes, j, innerWire); continue }
                        guard let ilen = readVarint(bytes, &j), ilen <= UInt64(end - j) else { break }
                        let iend = j + Int(ilen)
                        let text = String(decoding: bytes[j..<iend], as: UTF8.self)
                        if Int(inner >> 3) == 1 { key = text } else if Int(inner >> 3) == 2 { value = text }
                        j = iend
                    }
                    result[key] = value
                }
                i = end
            } else {
                i = skipField(bytes, i, wire)
            }
        }
        return result
    }

    private static func appendTag(field: Int, wire: Int, to out: inout [UInt8]) {
        appendVarint(UInt64((field << 3) | wire), to: &out)
    }

    private static func appendString(field: Int, _ value: String, to out: inout [UInt8]) {
        let utf8 = Array(value.utf8)
        appendTag(field: field, wire: 2, to: &out)
        appendVarint(UInt64(utf8.count), to: &out)
        out.append(contentsOf: utf8)
    }

    private static func appendVarint(_ value: UInt64, to out: inout [UInt8]) {
        var v = value
        repeat {
            var byte = UInt8(v & 0x7f)
            v >>= 7
            if v != 0 { byte |= 0x80 }
            out.append(byte)
        } while v != 0
    }

    private static func readVarint(_ bytes: [UInt8], _ i: inout Int) -> UInt64? {
        var shift: UInt64 = 0
        var value: UInt64 = 0
        while i < bytes.count {
            let byte = bytes[i]
            i += 1
            value |= UInt64(byte & 0x7f) << shift
            if byte & 0x80 == 0 { return value }
            shift += 7
            if shift >= 64 { return nil }
        }
        return nil
    }

    private static func skipField(_ bytes: [UInt8], _ start: Int, _ wire: Int) -> Int {
        var i = start
        switch wire {
        case 0:
            _ = readVarint(bytes, &i)
        case 1:
            i += 8
        case 2:
            if let len = readVarint(bytes, &i) { i += Int(len) }
        case 5:
            i += 4
        default:
            return bytes.count
        }
        return min(i, bytes.count)
    }
}
