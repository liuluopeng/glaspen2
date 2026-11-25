// S7: 镜头 FFI JSON 解析与状态应用(纯数据转换, 镜头数学仍在 core::camera)。
import AppKit

@objc public final class CameraState: NSObject {
    @objc(assignResult:assignZoom:)
    public static func assign(result: NSDictionary, assignZoom: Bool) -> Bool {
        guard let px = result["pan_x"] as? NSNumber,
              let py = result["pan_y"] as? NSNumber,
              let pz = result["zoom"] as? NSNumber else {
            NSLog("[camera] 镜头 JSON 缺字段: \(String(describing: result["error"] ?? result))")
            return false
        }
        glaspen2_router_set_camera(px.doubleValue, py.doubleValue,
                                  pz.doubleValue, assignZoom ? 1 : 0)
        return true
    }

    @objc(resultFromJSON:)
    public static func resultFromJSON(_ json: UnsafeMutablePointer<CChar>?) -> NSDictionary? {
        guard let json else {
            NSLog("[camera] core 镜头 FFI 返回 NULL")
            return nil
        }
        let data = Data(bytes: json, count: strlen(json))
        glaspen2_free_c_string(json)
        guard let obj = try? JSONSerialization.jsonObject(with: data),
              let dict = obj as? NSDictionary else {
            NSLog("[camera] core 镜头 JSON 无法解析")
            return nil
        }
        return dict
    }
}
