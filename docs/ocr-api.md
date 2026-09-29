# OCR 服务 HTTP API —— glaspen2 对接指南

axum 侧内置 **PP-OCRv6（PaddleOCR）ONNX 模型**（检测 det + 识别 rec），
提供 HTTP OCR 服务。glaspen2 的手写/画布内容拍照后可直接调它取文字。

- 模型:PP-OCRv6 检测(62MB) + 识别(73MB) + 字符字典,ONNX Runtime CPU 推理
- 输入:一张或多张图片(png/jpeg/webp/gif)
- 输出:逐张的识别文本

---

## 1. 端点

```
POST {API_BASE}/api/ocr/images
Content-Type: multipart/form-data; boundary=<boundary>
```

- **多图**:multipart 字段名 `image` **重复出现**即可(一图一个字段),
  字段顺序 = 结果顺序。字段名 `images` 同样接受。
- 响应壳是 `{code:200,...}` 直出(不是 `{msg,data}` 包装),见 §2。

认证:当前服务端**无鉴权**(仅限局域网使用)。glaspen2 客户端侧规则:
**已登录才发起** —— 集成配置了服务且登录成功后才调用本端点(自动携带
`Authorization: Bearer` 头, 服务端当前忽略); 未登录时 glaspen2 不发起
OCR, 涂鸦不受影响。

## 2. 响应

```json
{
  "code": 200,
  "count": 2,
  "results": [
    { "index": 0, "text": "识别出的完整文本", "chars": 42 },
    { "index": 1, "text": "第二张图的文本", "chars": 7 }
  ]
}
```

- `text` = 该图内全部文本区域的拼接(检测框从上到下)。
- 单张失败**不拖垮整批**:失败张返回 `error` 对象占位:

```json
{ "index": 1, "error": { "code": "IMAGE_DECODE_ERROR", "message": "图片解码失败: …" } }
```

错误码:`NO_IMAGE`(一个图字段都没有)/ `IMAGE_DECODE_ERROR` /
`OCR_ERROR`(模型推理失败)/ `MULTIPART_ERROR`、`READ_ERROR`(表单损坏)。

## 3. glaspen2 侧调用(ureq 示例)

与 `auth.rs` 的登录调用同栈(ureq + 手工 multipart body,
TLS 对 dev 自签证书 disable_verification):

```rust
fn ocr_images(api_base: &str, images: &[Vec<u8>]) -> Result<serde_json::Value, String> {
    let boundary = format!("glaspen-ocr-{}", std::process::id());
    let mut body: Vec<u8> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; \
                     name=\"image\"; filename=\"img{i}.png\"\r\n\
                     Content-Type: application/octet-stream\r\n\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(img);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(true)
        .build();
    let agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build();
    let agent = ureq::Agent::new_with_config(agent);
    let resp = agent
        .post(&format!("{api_base}/api/ocr/images"))
        .header("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .send(body.as_slice())
        .map_err(|e| format!("OCR 请求失败: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    if v["code"].as_i64() == Some(200) { Ok(v) } else { Err(format!("OCR 失败: {v}")) }
}
```

注意:OCR 是 CPU 重活(检测+识别两次 ONNX 推理),超时给足(≥60s/张,
上面示例全局 300s)。

## 4. 约束与建议

- 单图建议 ≤ 4000×4000;过大图服务端按原尺寸推理,慢且耗内存,
  建议客户端先等比缩到长边 ≤ 2000。
- 多张图**串行**识别(避免争核),N 张耗时 ≈ N × 单张(约 1–3s/张,
  视 CPU 与图内文字量)。
- 免鉴权仅限局域网;不要把 23001 暴露到公网后裸用。

## 5. 兼容端点

旧 `POST /api/ocr`(单图,响应 `{code:200,text}`)保留不动,
kongde 客户端仍在用。glaspen2 **直接用新的 `/api/ocr/images`** 即可。

## 6. 联调

```bash
curl -s -X POST http://<nas>:23001/api/ocr/images \
  -F "image=@a.png" -F "image=@b.png"
```

服务端日志每张图打 `[OCR] 开始识别 WxH` / `[OCR] 识别结果: N 字`。

## 7. 部署说明

模型从**可执行文件同级的 `models/` 目录**加载
(`ppocr_v6_det.onnx` / `ppocr_v6_rec.onnx` / `ppocr_v6_dict.json`)。

- 本机 dev(23001):模型已在位,开箱即用。
- docker(23000):镜像**未内置**模型,需挂载:
  `-v ./models:/app/models`(模型文件可向 axum 仓库索要)。
