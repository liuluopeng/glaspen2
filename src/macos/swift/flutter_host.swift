// S7: Flutter 设置面板宿主(引擎/VC 在 Swift 创建并持有)。
import AppKit
import FlutterMacOS

@objc public final class FlutterHost: NSObject {
    private static var engine: FlutterEngine?
    private static var viewController: FlutterViewController?

    @objc public static func ensureEngine() -> FlutterEngine {
        if let engine { return engine }
        let e = FlutterEngine(name: "glaspen_settings", project: nil)
        e.run(withEntrypoint: nil)
        engine = e
        return e
    }

    @objc public static func ensureViewController() -> FlutterViewController {
        if let viewController { return viewController }
        let vc = FlutterViewController(engine: ensureEngine(), nibName: nil, bundle: nil)
        viewController = vc
        return vc
    }
}

@_cdecl("sw_flutter_engine")
public func flutterEnginePtr() -> UnsafeMutableRawPointer? {
    Unmanaged.passUnretained(FlutterHost.ensureEngine()).toOpaque()
}

@_cdecl("sw_flutter_vc")
public func flutterViewControllerPtr() -> UnsafeMutableRawPointer? {
    Unmanaged.passUnretained(FlutterHost.ensureViewController()).toOpaque()
}
