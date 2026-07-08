using System;
using System.Runtime.InteropServices;

namespace GlasPen2
{
    /// <summary>
    /// P/Invoke declarations for the Rust glaspen2.dll FFI functions.
    /// This enables C# to call Rust directly (same process, zero IPC latency),
    /// matching the macOS ObjC → Rust FFI pattern.
    /// </summary>
    public static class GlaspenNative
    {
        private const string DllName = "glaspen2.dll";

        // ── Database & Settings ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_init_db(int screenW, int screenH);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_save_settings(double r, double g, double b, double widthScale);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_load_settings_parts(out double r, out double g, out double b, out double w);

        // ── Modeler (ink-stroke-modeler-rs) ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_modeler_begin(
            double r, double g, double b,
            double x, double y, double pressure, double timestamp, double widthScale);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_modeler_move(
            double x, double y, double pressure, double timestamp, double widthScale);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_modeler_end(
            double x, double y, double pressure, double timestamp, double widthScale);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_modeler_point_count();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_modeler_get_point(int idx, out double x, out double y, out double w);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_modeler_clear_buffer();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_modeler_commit_to_strokes(
            double r, double g, double b,
            IntPtr invColors, int invCount);

        // ── Simple stroke recording (no modeler) ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_begin_stroke(double r, double g, double b, double widthScale);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_add_point(double x, double y, double width);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_end_stroke();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_clear_strokes(int screenW, int screenH);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_delete_last_stroke();

        // ── Stroke data access ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_stroke_count();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_get_stroke_point_count(int idx);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_get_stroke_color(int idx, out double r, out double g, out double b);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern double glaspen2_get_stroke_avg_width(int idx);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_get_stroke_point(int idx, int pidx, out double x, out double y);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern double glaspen2_get_stroke_point_width(int idx, int pidx);

        // ── Page navigation ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_load_strokes_for_screen(long screenId);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_smooth_loaded_strokes();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern long glaspen2_prev_screen_id();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern long glaspen2_next_screen_id();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern long glaspen2_get_current_screen_id();

        // ── Export ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_save_drawing(IntPtr data, int width, int height, int stride);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_save_svg();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_save_xoj();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_save_gif_cropped(IntPtr data, int width, int height, int stride);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_stroke_bbox(out double xMin, out double yMin, out double xMax, out double yMax);

        // ── Utility ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern double glaspen2_now_secs();

        // ── Cairo renderer FFI ──

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr glaspen2_cairo_renderer_create(int w, int h);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_renderer_destroy(IntPtr renderer);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_draw_line(IntPtr renderer,
            double x0, double y0, double x1, double y1,
            double width, double r, double g, double b);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_draw_dot(IntPtr renderer,
            double x, double y, double width, double r, double g, double b);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_clear(IntPtr renderer);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr glaspen2_cairo_surface_data(IntPtr renderer);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr glaspen2_cairo_surface_data_mut(IntPtr renderer);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_surface_size(IntPtr renderer,
            out int w, out int h, out int stride);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_draw_modeler_buffer(IntPtr renderer,
            double r, double g, double b);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void glaspen2_cairo_replay_strokes(IntPtr renderer);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int glaspen2_cairo_undo(IntPtr renderer);
    }
}
