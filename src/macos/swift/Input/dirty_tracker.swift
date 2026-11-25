// S7: 脏矩形跟踪(点/表面点/表面矩形 → 视图点并集), 以 C 符号导出供 ObjC 调用。
import AppKit

@objc public final class DirtyTracker: NSObject {
    private static var hasDirty = false
    private static var rect = NSRect.zero
}

@_cdecl("dirty_reset")
public func dirtyReset() {
    DirtyTracker.reset()
}

@_cdecl("dirty_include_point")
public func dirtyIncludePoint(_ x: Double, _ y: Double, _ r: Double) {
    DirtyTracker.includePoint(x: x, y: y, r: r)
}

@_cdecl("dirty_include_surface_point")
public func dirtyIncludeSurfacePoint(_ x: Double, _ y: Double, _ r: Double) {
    let viewH = Double(glaspen2_router_screen_h())
    DirtyTracker.includePoint(x: x, y: viewH - y, r: r)
}

@_cdecl("dirty_include_surface_rect")
public func dirtyIncludeSurfaceRect(_ x: Double, _ y: Double, _ w: Double, _ h: Double) {
    let viewH = Double(glaspen2_router_screen_h())
    DirtyTracker.includePoint(x: x, y: viewH - (y + h), r: 0)
    DirtyTracker.includePoint(x: x + w, y: viewH - y, r: 0)
}

@_cdecl("gl_dirty_has_state")
public func glDirtyHasState() -> Int32 {
    DirtyTracker.hasAnyDirty ? 1 : 0
}

@_cdecl("gl_dirty_rect_x")
public func glDirtyRectX() -> Double { Double(DirtyTracker.currentRect.origin.x) }
@_cdecl("gl_dirty_rect_y")
public func glDirtyRectY() -> Double { Double(DirtyTracker.currentRect.origin.y) }
@_cdecl("gl_dirty_rect_w")
public func glDirtyRectW() -> Double { Double(DirtyTracker.currentRect.size.width) }
@_cdecl("gl_dirty_rect_h")
public func glDirtyRectH() -> Double { Double(DirtyTracker.currentRect.size.height) }

extension DirtyTracker {
    fileprivate static var hasAnyDirty: Bool { hasDirty }
    fileprivate static var currentRect: NSRect { rect }
    fileprivate static func reset() { hasDirty = false }

    fileprivate static func includePoint(x: Double, y: Double, r: Double) {
        let add = NSRect(x: x - r, y: y - r, width: r * 2, height: r * 2)
        if !hasDirty {
            rect = add
            hasDirty = true
        } else {
            rect = rect.union(add)
        }
    }
}
