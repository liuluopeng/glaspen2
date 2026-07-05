//! Dynamic loader for libcairo-2.dll.
//!
//! Loads Cairo via LoadLibrary/GetProcAddress (through libloading),
//! avoiding ABI issues between MSVC Rust and MinGW-built Cairo.
//!
//! Falls back to the pure-Rust stub if the DLL cannot be found.

use std::sync::OnceLock;

// ── Type aliases for Cairo C types ──
// On x86-64 these are all pointers or i32/f64; no struct layout dependency.
type CairoSurfaceT = *mut std::ffi::c_void;
type CairoT = *mut std::ffi::c_void;

// ── Cairo enum constants ──
#[allow(dead_code)] const CAIRO_FORMAT_ARGB32: i32 = 0;
#[allow(dead_code)] const CAIRO_OPERATOR_CLEAR: i32 = 0;
#[allow(dead_code)] const CAIRO_OPERATOR_OVER: i32 = 2;
const CAIRO_LINE_CAP_BUTT: i32 = 0;
const CAIRO_LINE_CAP_ROUND: i32 = 1;
const CAIRO_LINE_CAP_SQUARE: i32 = 2;
const CAIRO_LINE_JOIN_MITER: i32 = 0;
const CAIRO_LINE_JOIN_ROUND: i32 = 1;
const CAIRO_LINE_JOIN_BEVEL: i32 = 2;

// ── Function pointer types ──
type FnImageSurfaceCreate = unsafe extern "C" fn(format: i32, width: i32, height: i32) -> CairoSurfaceT;
type FnSurfaceDestroy = unsafe extern "C" fn(surface: CairoSurfaceT);
type FnSurfaceFlush = unsafe extern "C" fn(surface: CairoSurfaceT);
type FnSurfaceMarkDirty = unsafe extern "C" fn(surface: CairoSurfaceT);
type FnImageSurfaceGetWidth = unsafe extern "C" fn(surface: CairoSurfaceT) -> i32;
type FnImageSurfaceGetHeight = unsafe extern "C" fn(surface: CairoSurfaceT) -> i32;
type FnImageSurfaceGetStride = unsafe extern "C" fn(surface: CairoSurfaceT) -> i32;
type FnImageSurfaceGetData = unsafe extern "C" fn(surface: CairoSurfaceT) -> *mut u8;
type FnCreate = unsafe extern "C" fn(target: CairoSurfaceT) -> CairoT;
type FnDestroy = unsafe extern "C" fn(cr: CairoT);
type FnSetSourceRgba = unsafe extern "C" fn(cr: CairoT, r: f64, g: f64, b: f64, a: f64);
type FnSetLineWidth = unsafe extern "C" fn(cr: CairoT, width: f64);
type FnSetLineCap = unsafe extern "C" fn(cr: CairoT, cap: i32);
type FnSetLineJoin = unsafe extern "C" fn(cr: CairoT, join: i32);
type FnSetOperator = unsafe extern "C" fn(cr: CairoT, op: i32);
type FnMoveTo = unsafe extern "C" fn(cr: CairoT, x: f64, y: f64);
type FnLineTo = unsafe extern "C" fn(cr: CairoT, x: f64, y: f64);
type FnArc = unsafe extern "C" fn(cr: CairoT, xc: f64, yc: f64, radius: f64, angle1: f64, angle2: f64);
type FnStroke = unsafe extern "C" fn(cr: CairoT);
type FnFill = unsafe extern "C" fn(cr: CairoT);
type FnPaint = unsafe extern "C" fn(cr: CairoT);
type FnSave = unsafe extern "C" fn(cr: CairoT);
type FnRestore = unsafe extern "C" fn(cr: CairoT);

// ── Loaded function table ──
struct CairoVTable {
    image_surface_create: FnImageSurfaceCreate,
    surface_destroy: FnSurfaceDestroy,
    surface_flush: FnSurfaceFlush,
    surface_mark_dirty: FnSurfaceMarkDirty,
    image_surface_get_width: FnImageSurfaceGetWidth,
    image_surface_get_height: FnImageSurfaceGetHeight,
    image_surface_get_stride: FnImageSurfaceGetStride,
    image_surface_get_data: FnImageSurfaceGetData,
    create: FnCreate,
    destroy: FnDestroy,
    set_source_rgba: FnSetSourceRgba,
    set_line_width: FnSetLineWidth,
    set_line_cap: FnSetLineCap,
    set_line_join: FnSetLineJoin,
    set_operator: FnSetOperator,
    move_to: FnMoveTo,
    line_to: FnLineTo,
    arc: FnArc,
    stroke: FnStroke,
    fill: FnFill,
    paint: FnPaint,
    save: FnSave,
    restore: FnRestore,
}

// ── Global: loaded once, never freed ──
static CAIRO_LIB: OnceLock<Option<libloading::Library>> = OnceLock::new();
static CAIRO_VT: OnceLock<Option<CairoVTable>> = OnceLock::new();

fn cairo_vtable() -> Option<&'static CairoVTable> {
    CAIRO_VT.get_or_init(|| {
        let lib = match unsafe { libloading::Library::new("libcairo-2.dll") } {
            Ok(lib) => lib,
            Err(e) => {
                eprintln!("[cairo_dl] Failed to load libcairo-2.dll: {} — using stub", e);
                return None;
            }
        };

        macro_rules! sym {
            ($lib:expr, $name:expr) => {
                unsafe {
                    match $lib.get::<*const u8>($name.as_bytes()) {
                        Ok(f) => std::mem::transmute(*f),
                        Err(_) => {
                            eprintln!("[cairo_dl] Missing symbol: {}", $name);
                            return None;
                        }
                    }
                }
            };
        }

        let vt = CairoVTable {
            image_surface_create: sym!(lib, "cairo_image_surface_create"),
            surface_destroy: sym!(lib, "cairo_surface_destroy"),
            surface_flush: sym!(lib, "cairo_surface_flush"),
            surface_mark_dirty: sym!(lib, "cairo_surface_mark_dirty"),
            image_surface_get_width: sym!(lib, "cairo_image_surface_get_width"),
            image_surface_get_height: sym!(lib, "cairo_image_surface_get_height"),
            image_surface_get_stride: sym!(lib, "cairo_image_surface_get_stride"),
            image_surface_get_data: sym!(lib, "cairo_image_surface_get_data"),
            create: sym!(lib, "cairo_create"),
            destroy: sym!(lib, "cairo_destroy"),
            set_source_rgba: sym!(lib, "cairo_set_source_rgba"),
            set_line_width: sym!(lib, "cairo_set_line_width"),
            set_line_cap: sym!(lib, "cairo_set_line_cap"),
            set_line_join: sym!(lib, "cairo_set_line_join"),
            set_operator: sym!(lib, "cairo_set_operator"),
            move_to: sym!(lib, "cairo_move_to"),
            line_to: sym!(lib, "cairo_line_to"),
            arc: sym!(lib, "cairo_arc"),
            stroke: sym!(lib, "cairo_stroke"),
            fill: sym!(lib, "cairo_fill"),
            paint: sym!(lib, "cairo_paint"),
            save: sym!(lib, "cairo_save"),
            restore: sym!(lib, "cairo_restore"),
        };

        // Keep the library handle alive so function pointers remain valid
        CAIRO_LIB.get_or_init(|| Some(lib));
        eprintln!("[cairo_dl] libcairo-2.dll loaded successfully");
        Some(vt)
    }).as_ref()
}

/// Returns true if real Cairo DLL is loaded and working.
pub fn is_cairo_loaded() -> bool {
    cairo_vtable().is_some()
}

/// Initialize Cairo. Call once at startup.
/// Returns true if real Cairo is available.
pub fn cairo_init() -> bool {
    cairo_vtable().is_some()
}

// ── Safe Rust wrappers ──

/// Wrapper around an ARGB32 Cairo image surface.
pub struct CairoRealSurface {
    ptr: CairoSurfaceT,
    width: i32,
    height: i32,
    stride: i32,
}

impl CairoRealSurface {
    pub fn create(width: i32, height: i32) -> Option<Self> {
        let vt = cairo_vtable()?;
        let ptr = unsafe { (vt.image_surface_create)(CAIRO_FORMAT_ARGB32, width, height) };
        if ptr.is_null() { return None; }
        let stride = unsafe { (vt.image_surface_get_stride)(ptr) };
        Some(Self { ptr, width, height, stride })
    }

    pub fn width(&self) -> i32 { self.width }
    pub fn height(&self) -> i32 { self.height }
    pub fn stride(&self) -> i32 { self.stride }

    /// Get raw pointer to BGRA pixel data. Call flush() first.
    pub fn data_ptr(&self) -> *const u8 {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.image_surface_get_data)(self.ptr) }
    }

    /// Get mutable raw pointer to BGRA pixel data. Call flush() first, mark_dirty() after.
    pub fn data_ptr_mut(&self) -> *mut u8 {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.image_surface_get_data)(self.ptr) }
    }

    pub fn flush(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.surface_flush)(self.ptr); }
    }

    pub fn mark_dirty(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.surface_mark_dirty)(self.ptr); }
    }

    pub fn raw_ptr(&self) -> CairoSurfaceT { self.ptr }
}

impl Drop for CairoRealSurface {
    fn drop(&mut self) {
        if let Some(vt) = cairo_vtable() {
            unsafe { (vt.surface_destroy)(self.ptr); }
        }
    }
}

/// SAFETY: Cairo surface data access is thread-safe for read-only.
/// Writing to the surface from multiple threads needs external synchronization.
unsafe impl Send for CairoRealSurface {}
unsafe impl Sync for CairoRealSurface {}

/// Wrapper around a cairo_t* context.
pub struct CairoRealContext {
    ptr: CairoT,
}

impl CairoRealContext {
    pub fn new(surface: &CairoRealSurface) -> Option<Self> {
        let vt = cairo_vtable()?;
        let ptr = unsafe { (vt.create)(surface.ptr) };
        if ptr.is_null() { return None; }
        Some(Self { ptr })
    }

    pub fn set_source_rgba(&self, r: f64, g: f64, b: f64, a: f64) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.set_source_rgba)(self.ptr, r, g, b, a); }
    }

    pub fn set_line_width(&self, width: f64) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.set_line_width)(self.ptr, width); }
    }

    pub fn set_line_cap_round(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.set_line_cap)(self.ptr, CAIRO_LINE_CAP_ROUND); }
    }

    pub fn set_line_join_round(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.set_line_join)(self.ptr, CAIRO_LINE_JOIN_ROUND); }
    }

    pub fn set_operator_clear(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.set_operator)(self.ptr, CAIRO_OPERATOR_CLEAR); }
    }

    pub fn set_operator_over(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.set_operator)(self.ptr, CAIRO_OPERATOR_OVER); }
    }

    pub fn move_to(&self, x: f64, y: f64) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.move_to)(self.ptr, x, y); }
    }

    pub fn line_to(&self, x: f64, y: f64) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.line_to)(self.ptr, x, y); }
    }

    pub fn arc(&self, xc: f64, yc: f64, radius: f64, angle1: f64, angle2: f64) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.arc)(self.ptr, xc, yc, radius, angle1, angle2); }
    }

    pub fn stroke(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.stroke)(self.ptr); }
    }

    pub fn fill(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.fill)(self.ptr); }
    }

    pub fn paint(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.paint)(self.ptr); }
    }

    pub fn save(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.save)(self.ptr); }
    }

    pub fn restore(&self) {
        let vt = cairo_vtable().unwrap();
        unsafe { (vt.restore)(self.ptr); }
    }
}

impl Drop for CairoRealContext {
    fn drop(&mut self) {
        if let Some(vt) = cairo_vtable() {
            unsafe { (vt.destroy)(self.ptr); }
        }
    }
}
