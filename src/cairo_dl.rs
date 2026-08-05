//! Cairo 渲染器(动态加载,唯一保留的 cairo 实现)。
//!
//! 源自已验证原型 wAPItry/src/common.rs:用 libloading 加载 libcairo-2.dll,
//! 把 cairo surface 直接绑定到 32bit BGRA 像素缓冲(CAIRO_FORMAT_ARGB32 与
//! 32bit DIB 布局一致:B,G,R,A 小端),无需拷贝。
//!
//! 两种用法:
//!   - `load(bits, w, h)`:绑定外部缓冲(overlay 的 DIB 内存)
//!   - `create_owned(w, h)`:创建自有表面(导出/OCR 渲染,之后读 `bits()`)

#![allow(unsafe_op_in_unsafe_fn)]

use libloading::{Library, Symbol};

/// CAIRO_FORMAT_ARGB32 = 0
const CAIRO_FORMAT_ARGB32: i32 = 0;
/// CAIRO_LINE_CAP_ROUND = 1
const CAIRO_LINE_CAP_ROUND: i32 = 1;
/// CAIRO_LINE_JOIN_ROUND = 1
const CAIRO_LINE_JOIN_ROUND: i32 = 1;

pub struct CairoRenderer {
    _lib: Library,
    surface: *mut std::ffi::c_void,
    cr: *mut std::ffi::c_void,
    bits: *mut u8,
    pub w: i32,
    pub h: i32,
    set_source_rgba: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64),
    set_line_width: unsafe extern "C" fn(*mut std::ffi::c_void, f64),
    move_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64),
    line_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64),
    stroke: unsafe extern "C" fn(*mut std::ffi::c_void),
    flush: unsafe extern "C" fn(*mut std::ffi::c_void),
    new_path: unsafe extern "C" fn(*mut std::ffi::c_void),
    fill: unsafe extern "C" fn(*mut std::ffi::c_void),
    arc: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64, f64),
}

/// SAFETY: 每个 CairoRenderer 实例只能被单线程使用;Send 用于多线程导出场景,
/// 调用方需保证同一实例不同时被多线程写入。
unsafe impl Send for CairoRenderer {}

impl CairoRenderer {
    /// 加载 cairo DLL 并绑定到外部像素缓冲(失败返回 None,调用方回退自绘)
    pub fn load(bits: *mut u8, w: i32, h: i32) -> Option<Self> {
        unsafe {
            let lib = load_library()?;

            unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Option<T> {
                let s: Symbol<T> = lib.get(name).ok()?;
                Some(*s)
            }

            let create_surface: unsafe extern "C" fn(*mut u8, i32, i32, i32, i32) -> *mut std::ffi::c_void =
                sym(&lib, b"cairo_image_surface_create_for_data")?;
            let cairo_create: unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void =
                sym(&lib, b"cairo_create")?;
            let set_source_rgba: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64) =
                sym(&lib, b"cairo_set_source_rgba")?;
            let set_line_width: unsafe extern "C" fn(*mut std::ffi::c_void, f64) =
                sym(&lib, b"cairo_set_line_width")?;
            let set_line_cap: unsafe extern "C" fn(*mut std::ffi::c_void, i32) =
                sym(&lib, b"cairo_set_line_cap")?;
            let set_line_join: unsafe extern "C" fn(*mut std::ffi::c_void, i32) =
                sym(&lib, b"cairo_set_line_join")?;
            let move_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64) =
                sym(&lib, b"cairo_move_to")?;
            let line_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64) =
                sym(&lib, b"cairo_line_to")?;
            let stroke: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_stroke")?;
            let flush: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_surface_flush")?;
            let new_path: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_new_path")?;
            let fill: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_fill")?;
            let arc: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64, f64) =
                sym(&lib, b"cairo_arc")?;

            let surface = create_surface(bits, CAIRO_FORMAT_ARGB32, w, h, w * 4);
            if surface.is_null() {
                return None;
            }
            let cr = cairo_create(surface);
            if cr.is_null() {
                let surface_destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                    sym(&lib, b"cairo_surface_destroy").unwrap();
                surface_destroy(surface);
                return None;
            }
            // 圆头线帽/连接
            let _ = (set_line_cap)(cr, CAIRO_LINE_CAP_ROUND);
            let _ = (set_line_join)(cr, CAIRO_LINE_JOIN_ROUND);

            Some(Self {
                _lib: lib,
                surface,
                cr,
                bits,
                w,
                h,
                set_source_rgba,
                set_line_width,
                move_to,
                line_to,
                stroke,
                flush,
                new_path,
                fill,
                arc,
            })
        }
    }

    /// 创建自有表面(导出/OCR 渲染用),之后通过 `bits()` 读像素
    pub fn create_owned(w: i32, h: i32) -> Option<Self> {
        unsafe {
            let lib = load_library()?;

            unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Option<T> {
                let s: Symbol<T> = lib.get(name).ok()?;
                Some(*s)
            }

            let create_surface: unsafe extern "C" fn(i32, i32, i32) -> *mut std::ffi::c_void =
                sym(&lib, b"cairo_image_surface_create")?;
            let get_data: unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut u8 =
                sym(&lib, b"cairo_image_surface_get_data")?;
            let cairo_create: unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void =
                sym(&lib, b"cairo_create")?;
            let set_source_rgba: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64) =
                sym(&lib, b"cairo_set_source_rgba")?;
            let set_line_width: unsafe extern "C" fn(*mut std::ffi::c_void, f64) =
                sym(&lib, b"cairo_set_line_width")?;
            let set_line_cap: unsafe extern "C" fn(*mut std::ffi::c_void, i32) =
                sym(&lib, b"cairo_set_line_cap")?;
            let set_line_join: unsafe extern "C" fn(*mut std::ffi::c_void, i32) =
                sym(&lib, b"cairo_set_line_join")?;
            let move_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64) =
                sym(&lib, b"cairo_move_to")?;
            let line_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64) =
                sym(&lib, b"cairo_line_to")?;
            let stroke: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_stroke")?;
            let flush: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_surface_flush")?;
            let new_path: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_new_path")?;
            let fill: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_fill")?;
            let arc: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64, f64) =
                sym(&lib, b"cairo_arc")?;

            let surface = create_surface(CAIRO_FORMAT_ARGB32, w, h);
            if surface.is_null() {
                return None;
            }
            let cr = cairo_create(surface);
            if cr.is_null() {
                let surface_destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                    sym(&lib, b"cairo_surface_destroy").unwrap();
                surface_destroy(surface);
                return None;
            }
            let _ = (set_line_cap)(cr, CAIRO_LINE_CAP_ROUND);
            let _ = (set_line_join)(cr, CAIRO_LINE_JOIN_ROUND);

            let bits = get_data(surface);
            if bits.is_null() {
                let destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                    sym(&lib, b"cairo_destroy").unwrap();
                let surface_destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                    sym(&lib, b"cairo_surface_destroy").unwrap();
                destroy(cr);
                surface_destroy(surface);
                return None;
            }

            Some(Self {
                _lib: lib,
                surface,
                cr,
                bits,
                w,
                h,
                set_source_rgba,
                set_line_width,
                move_to,
                line_to,
                stroke,
                flush,
                new_path,
                fill,
                arc,
            })
        }
    }

    /// 绑定一个外部 cairo image surface(macOS ObjC 侧传入的 g_surface)。
    /// 不拥有 surface(ObjC 负责创建/销毁),只持有 context 和绘制函数指针,
    /// 用于把 STROKES 重绘进既有表面(glaspen2_draw_rebuild)。
    pub fn from_surface(surface: *mut std::ffi::c_void) -> Option<Self> {
        unsafe {
            let lib = load_library()?;

            unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Option<T> {
                let s: Symbol<T> = lib.get(name).ok()?;
                Some(*s)
            }

            let cairo_create: unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void =
                sym(&lib, b"cairo_create")?;
            let get_data: unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut u8 =
                sym(&lib, b"cairo_image_surface_get_data")?;
            let get_width: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32 =
                sym(&lib, b"cairo_image_surface_get_width")?;
            let get_height: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32 =
                sym(&lib, b"cairo_image_surface_get_height")?;
            let get_stride: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32 =
                sym(&lib, b"cairo_image_surface_get_stride")?;
            let set_source_rgba: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64) =
                sym(&lib, b"cairo_set_source_rgba")?;
            let set_line_width: unsafe extern "C" fn(*mut std::ffi::c_void, f64) =
                sym(&lib, b"cairo_set_line_width")?;
            let set_line_cap: unsafe extern "C" fn(*mut std::ffi::c_void, i32) =
                sym(&lib, b"cairo_set_line_cap")?;
            let set_line_join: unsafe extern "C" fn(*mut std::ffi::c_void, i32) =
                sym(&lib, b"cairo_set_line_join")?;
            let move_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64) =
                sym(&lib, b"cairo_move_to")?;
            let line_to: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64) =
                sym(&lib, b"cairo_line_to")?;
            let stroke: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_stroke")?;
            let flush: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_surface_flush")?;
            let new_path: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_new_path")?;
            let fill: unsafe extern "C" fn(*mut std::ffi::c_void) =
                sym(&lib, b"cairo_fill")?;
            let arc: unsafe extern "C" fn(*mut std::ffi::c_void, f64, f64, f64, f64, f64) =
                sym(&lib, b"cairo_arc")?;

            if surface.is_null() {
                return None;
            }
            let cr = cairo_create(surface);
            if cr.is_null() {
                return None;
            }
            let _ = (set_line_cap)(cr, CAIRO_LINE_CAP_ROUND);
            let _ = (set_line_join)(cr, CAIRO_LINE_JOIN_ROUND);

            let bits = get_data(surface);
            if bits.is_null() {
                let destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                    sym(&lib, b"cairo_destroy")?;
                destroy(cr);
                return None;
            }

            Some(Self {
                _lib: lib,
                surface,
                cr,
                bits,
                w: get_width(surface),
                h: get_height(surface),
                set_source_rgba,
                set_line_width,
                move_to,
                line_to,
                stroke,
                flush,
                new_path,
                fill,
                arc,
            })
        }
    }

    /// 像素缓冲(BGRA 预乘)
    pub fn bits(&self) -> *mut u8 {
        self.bits
    }

    /// 画一条抗锯齿线段(圆头),颜色为 (R,G,B) 0..255
    pub fn stroke_line(&self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: (u8, u8, u8)) {
        unsafe {
            let _ = (self.set_source_rgba)(
                self.cr,
                color.0 as f64 / 255.0,
                color.1 as f64 / 255.0,
                color.2 as f64 / 255.0,
                1.0,
            );
            let _ = (self.set_line_width)(self.cr, width.max(0.5) as f64);
            let _ = (self.move_to)(self.cr, x0 as f64, y0 as f64);
            let _ = (self.line_to)(self.cr, x1 as f64, y1 as f64);
            let _ = (self.stroke)(self.cr);
        }
    }

    /// 填充闭合轮廓多边形(可变宽度笔迹),抗锯齿
    pub fn fill_outline(&self, outline: &[(f32, f32)], color: (u8, u8, u8)) {
        unsafe {
            let _ = (self.set_source_rgba)(
                self.cr,
                color.0 as f64 / 255.0,
                color.1 as f64 / 255.0,
                color.2 as f64 / 255.0,
                1.0,
            );
            let _ = (self.new_path)(self.cr);
            if let Some(p0) = outline.first() {
                let _ = (self.move_to)(self.cr, p0.0 as f64, p0.1 as f64);
                for p in outline.iter().skip(1) {
                    let _ = (self.line_to)(self.cr, p.0 as f64, p.1 as f64);
                }
                // cairo_fill 会隐式闭合路径
                let _ = (self.fill)(self.cr);
            }
        }
    }

    /// 填充实心圆(笔迹端点圆帽)
    pub fn fill_circle(&self, cx: f32, cy: f32, radius: f32, color: (u8, u8, u8)) {
        unsafe {
            let _ = (self.set_source_rgba)(
                self.cr,
                color.0 as f64 / 255.0,
                color.1 as f64 / 255.0,
                color.2 as f64 / 255.0,
                1.0,
            );
            let _ = (self.new_path)(self.cr);
            let _ = (self.arc)(self.cr, cx as f64, cy as f64, radius.max(0.0) as f64, 0.0, 6.283185307179586);
            let _ = (self.fill)(self.cr);
        }
    }

    /// 填充实心矩形(彩虹指示器等)
    pub fn fill_rect(&self, x: f32, y: f32, w: f32, h: f32, color: (u8, u8, u8)) {
        unsafe {
            let _ = (self.set_source_rgba)(
                self.cr,
                color.0 as f64 / 255.0,
                color.1 as f64 / 255.0,
                color.2 as f64 / 255.0,
                1.0,
            );
            let _ = (self.new_path)(self.cr);
            let _ = (self.move_to)(self.cr, x as f64, y as f64);
            let _ = (self.line_to)(self.cr, (x + w) as f64, y as f64);
            let _ = (self.line_to)(self.cr, (x + w) as f64, (y + h) as f64);
            let _ = (self.line_to)(self.cr, x as f64, (y + h) as f64);
            let _ = (self.fill)(self.cr);
        }
    }

    /// 清空为全透明(alpha=0)
    pub fn clear(&self) {
        unsafe {
            let n = (self.w as usize) * (self.h as usize) * 4;
            std::slice::from_raw_parts_mut(self.bits, n).fill(0);
            let _ = (self.flush)(self.surface);
        }
    }

    /// 把 cairo 的绘制结果写回像素缓冲
    pub fn flush(&self) {
        unsafe {
            let _ = (self.flush)(self.surface);
        }
    }
}

impl Drop for CairoRenderer {
    fn drop(&mut self) {
        unsafe {
            let _ = (self.flush)(self.surface);
            let destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                *self._lib.get(b"cairo_destroy").unwrap();
            let surface_destroy: unsafe extern "C" fn(*mut std::ffi::c_void) =
                *self._lib.get(b"cairo_surface_destroy").unwrap();
            destroy(self.cr);
            surface_destroy(self.surface);
        }
    }
}

/// macOS: 在已加载镜像里找到 libcairo 的绝对路径 (由 ObjC 的 -lcairo 链接进来)。
/// macOS 的 dlopen 不自动补 "lib" 前缀, 按名加载 ("cairo"/"libcairo.2.dylib") 都会失败。
#[cfg(target_os = "macos")]
fn find_loaded_cairo_path() -> Option<std::path::PathBuf> {
    unsafe extern "C" {
        fn _dyld_image_count() -> u32;
        fn _dyld_get_image_name(i: u32) -> *const std::ffi::c_char;
    }
    unsafe {
        for i in 0.._dyld_image_count() {
            let name = _dyld_get_image_name(i);
            if name.is_null() {
                continue;
            }
            let cstr = std::ffi::CStr::from_ptr(name);
            if let Ok(s) = cstr.to_str() {
                if s.contains("/libcairo") {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    }
    None
}

/// 加载 cairo 库。候选顺序:
///   1. exe 同目录(build.rs 已把 vendor/win/cairo 的 DLL 复制到产物目录,
///      安装器也随包分发,不依赖用户安装 Rnote/MSYS2)
///   2. 系统 DLL 搜索路径
/// 找到后把所在目录加入 DLL 搜索路径,保证 cairo 的依赖 DLL 可解析。
fn load_library() -> Option<Library> {
    #[cfg(windows)]
    {
        // 1) exe 同目录(构建产物 target/*,或打包后的安装目录)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let dll = dir.join("libcairo-2.dll");
                if dll.exists() {
                    let dir_str = dir.to_string_lossy().to_string();
                    // 让 cairo 的依赖 DLL(glib/pixman/freetype...)也能被解析
                    let wide: Vec<u16> = dir_str.encode_utf16().chain(std::iter::once(0)).collect();
                    unsafe {
                        let _ = windows::Win32::System::LibraryLoader::SetDllDirectoryW(
                            windows::core::PCWSTR(wide.as_ptr()),
                        );
                    }
                    let l = unsafe { Library::new(&dll) };
                    if let Ok(l) = l {
                        eprintln!("[cairo_dl] 加载 cairo: {}", dll.display());
                        return Some(l);
                    }
                }
            }
        }
        // 2) 系统 DLL 搜索路径
        unsafe { Library::new("libcairo-2.dll") }.ok()
    }
    #[cfg(target_os = "macos")]
    {
        // 主进程已链接 libcairo (ObjC -lcairo), 按它的绝对路径 dlopen
        let path = find_loaded_cairo_path()?;
        let l = unsafe { Library::new(&path) };
        if let Ok(l) = l {
            eprintln!("[cairo_dl] 加载 cairo: {}", path.display());
            return Some(l);
        }
        // 兜底: 常见 Homebrew 位置
        unsafe { Library::new("/opt/homebrew/opt/cairo/lib/libcairo.2.dylib") }.ok()
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        // Linux: dlopen 自动补 lib 前缀
        unsafe { Library::new("cairo") }.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 验证 macOS 上 cairo_dl 能真正加载 cairo 并绘制 (撤销/缩略图/PDF 依赖它)
    #[test]
    fn test_create_owned_draws_pixels() {
        let Some(r) = CairoRenderer::create_owned(64, 64) else {
            panic!("cairo must load via main-process handle");
        };
        r.clear();
        r.fill_circle(16.0, 16.0, 8.0, (255, 0, 0));
        r.stroke_line(16.0, 16.0, 48.0, 48.0, 6.0, (0, 255, 0));
        r.flush();
        let bits = unsafe { std::slice::from_raw_parts(r.bits(), 64 * 64 * 4) };
        let mut non_zero = 0usize;
        for px in bits.chunks(4) {
            if px[3] != 0 {
                non_zero += 1;
            }
        }
        assert!(non_zero > 100, "stroke + circle must produce visible pixels (got {})", non_zero);
        // 圆帽端点和线段中点附近应有像素
        let at = |x: usize, y: usize| -> u8 {
            let off = (y * 64 + x) * 4;
            bits[off + 3]
        };
        assert!(at(16, 16) > 0, "circle center should be opaque");
        assert!(at(32, 32) > 0, "line midpoint should be opaque");
    }
}
