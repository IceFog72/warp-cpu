//! CPU raster backend for software rendering (Linux `cpu-renderer` feature).
//!
//! `Scene` -> `u32` XRGB pixels -> `softbuffer`. Zero wgpu, zero llvmpipe.
//!
//! v1 scope: solid rect fills, glyph blits, image blits, layer clip bounds.
//! Deliberate fallbacks (see `ponytail:`): gradients flatten to start color,
//! rounded corners/borders/shadows render square and fades are ignored.
//! Icon tint, image opacity, and source-over alpha are implemented to match the
//! current wgpu image/glyph paths closely enough for the CPU fallback. Terminal
//! UI is overwhelmingly solid rects + text, so this covers
//! the common case; effects parity is a later phase, not this spike.

use std::collections::HashMap;

use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::{Vector2F, Vector2I};

use crate::fonts::{self, canvas::RasterFormat, SubpixelAlignment};
use crate::rendering::GlyphConfig;
use crate::scene::GlyphKey;
use crate::Scene;
use warpui_core::elements::Fill;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Opt-in switch. Separate from `WARP_FORCE_SOFTWARE` (which keeps meaning
/// "llvmpipe via wgpu") so existing software users are not hijacked by this
/// experimental path.
pub fn cpu_renderer_requested() -> bool {
    std::env::var("WARP_CPU_RENDERER")
        .ok()
        .is_some_and(|val| val == "1" || val.eq_ignore_ascii_case("true"))
}

/// Pack to softbuffer 0.4's documented `u32` pixel format:
/// `0x00RRGGBB`. The high byte is reserved and must stay zero.
///
/// The CPU backend therefore creates an opaque native window. softbuffer
/// 0.4 does not provide a supported alpha-present path for Warp's transparent
/// X11/Wayland window, and using a depth-32 transparent visual is the source
/// of the compositor corruption this fallback is meant to avoid.
pub fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

fn unpack_rgb(px: u32) -> (u8, u8, u8) {
    ((px >> 16) as u8, (px >> 8) as u8, px as u8)
}

/// Exact floor division by 255 for values in the range used by 8-bit
/// alpha compositing (0..=65025), avoiding an integer divide in the
/// per-pixel hot path.
#[inline(always)]
fn div255_floor(value: u32) -> u8 {
    let value = value + 1;
    ((value + (value >> 8)) >> 8) as u8
}

/// Exact rounded `(a * b) / 255` for 8-bit channels without an integer
/// division. This is equivalent to `(a*b + 127) / 255` for all inputs.
#[inline(always)]
fn mul_alpha(a: u8, b: u8) -> u8 {
    let value = u32::from(a) * u32::from(b) + 128;
    ((value + (value >> 8)) >> 8) as u8
}

/// Src-over blend of a coverage/straight-alpha source onto an opaque dest.
#[inline(always)]
fn blend_pixel(dst: u32, r: u8, g: u8, b: u8, a: u8) -> u32 {
    if a == 0 {
        return dst;
    }
    if a == 255 {
        return pack_rgb(r, g, b);
    }
    let (dr, dg, db) = unpack_rgb(dst);
    let a = u32::from(a);
    let ia = 255 - a;
    pack_rgb(
        div255_floor(u32::from(r) * a + u32::from(dr) * ia),
        div255_floor(u32::from(g) * a + u32::from(dg) * ia),
        div255_floor(u32::from(b) * a + u32::from(db) * ia),
    )
}

/// Integer rect, max exclusive. All raster works in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IRect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl IRect {
    fn full(width: u32, height: u32) -> Self {
        Self {
            x0: 0,
            y0: 0,
            x1: width as i32,
            y1: height as i32,
        }
    }

    fn intersect(self, other: Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }

    fn is_empty(self) -> bool {
        self.x0 >= self.x1 || self.y0 >= self.y1
    }
}

fn scaled_irect(min: Vector2F, size: Vector2F) -> IRect {
    IRect {
        x0: min.x().floor() as i32,
        y0: min.y().floor() as i32,
        x1: (min.x() + size.x()).ceil() as i32,
        y1: (min.y() + size.y()).ceil() as i32,
    }
}

/// Straight-alpha RGBA image view (glyph canvases excluded; see blit_glyph).
struct RgbaImage<'a> {
    bytes: &'a [u8],
    width: u32,
    height: u32,
}

/// Normalize `Canvas::row_stride` to bytes. Warp's core type documents this
/// field as bytes, but some Linux/font-kit builds have been observed at runtime
/// to report it in pixels (for example a 12x12 RGBA glyph with stride=12).
/// Accept both layouts so the software renderer does not smear rows if that
/// upstream representation changes underneath us.
fn canvas_row_stride_bytes(canvas: &fonts::canvas::Canvas) -> Option<usize> {
    let width = usize::try_from(canvas.size.x()).ok()?;
    let height = usize::try_from(canvas.size.y()).ok()?;
    let bytes_per_pixel = canvas.format.bytes_per_pixel() as usize;
    let min_row_bytes = width.checked_mul(bytes_per_pixel)?;

    // First honor the documented contract: row_stride is already bytes.
    if canvas.row_stride >= min_row_bytes {
        let required = canvas.row_stride.checked_mul(height)?;
        if required <= canvas.pixels.len() {
            return Some(canvas.row_stride);
        }
    }

    // Runtime compatibility path: some Linux/font-kit canvases expose stride
    // in pixels. Convert it to bytes before indexing rows.
    if canvas.row_stride >= width {
        let stride_bytes = canvas.row_stride.checked_mul(bytes_per_pixel)?;
        let required = stride_bytes.checked_mul(height)?;
        if stride_bytes >= min_row_bytes && required <= canvas.pixels.len() {
            return Some(stride_bytes);
        }
    }

    // Last-resort tightly packed layout, only when the backing buffer proves
    // that enough data exists. This avoids out-of-bounds reads on malformed
    // canvases while keeping ordinary tightly packed glyphs renderable.
    let required = min_row_bytes.checked_mul(height)?;
    (required <= canvas.pixels.len()).then_some(min_row_bytes)
}

/// Key for a CPU-rasterized glyph. The GPU renderer keeps a persistent glyph
/// atlas; without an equivalent cache the software path calls FreeType once per
/// visible glyph on every redraw, which dominates frame time in text-heavy UIs.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct CpuGlyphCacheKey {
    glyph_key: GlyphKey,
    scale_bits: u32,
    subpixel_alignment: SubpixelAlignment,
}

struct CachedGlyph {
    bounds: RectI,
    rasterized: fonts::RasterizedGlyph,
}

/// Persistent per-window glyph raster cache for the CPU backend.
///
/// This mirrors the important behavior of the wgpu `GlyphCache`: rasterize a
/// `(glyph, scale, subpixel)` combination once and reuse its bitmap across
/// frames. Empty glyphs (for example whitespace) are cached as `None` too.
pub struct CpuGlyphCache {
    entries: HashMap<CpuGlyphCacheKey, Option<CachedGlyph>>,
    glyph_config: GlyphConfig,
}

impl CpuGlyphCache {
    pub fn new(glyph_config: GlyphConfig) -> Self {
        Self {
            entries: HashMap::new(),
            glyph_config,
        }
    }

    fn get(
        &mut self,
        font_cache: &fonts::Cache,
        glyph_key: GlyphKey,
        scale: f32,
        subpixel_alignment: SubpixelAlignment,
        glyph_config: &GlyphConfig,
    ) -> Option<&CachedGlyph> {
        if self.glyph_config != *glyph_config {
            self.entries.clear();
            self.glyph_config = *glyph_config;
        }

        let key = CpuGlyphCacheKey {
            glyph_key,
            scale_bits: scale.to_bits(),
            subpixel_alignment,
        };

        match self.entries.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut().as_ref(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let raster_scale = Vector2F::splat(scale);
                let bounds =
                    match font_cache.glyph_raster_bounds(glyph_key, raster_scale, glyph_config) {
                        Ok(bounds) => bounds,
                        Err(_) => return None,
                    };

                let cached = if bounds.size() == Vector2I::zero() {
                    None
                } else {
                    match font_cache.rasterized_glyph(
                        glyph_key,
                        raster_scale,
                        subpixel_alignment,
                        glyph_config,
                        RasterFormat::Rgba32,
                    ) {
                        Ok(rasterized) => Some(CachedGlyph { bounds, rasterized }),
                        Err(_) => return None,
                    }
                };
                entry.insert(cached).as_ref()
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Destination framebuffer plus the active layer clip. Bundles the
/// framebuffer triple that every blit needs, keeping arg counts small.
struct Frame<'a> {
    pixels: &'a mut [u32],
    width: u32,
    height: u32,
    clip: IRect,
}

impl Frame<'_> {
    fn bounds(&self) -> IRect {
        IRect::full(self.width, self.height)
    }

    fn fill(&mut self, rect: IRect, packed: u32) {
        let rect = rect.intersect(self.clip).intersect(self.bounds());
        if rect.is_empty() {
            return;
        }
        let stride = self.width as usize;
        for y in rect.y0..rect.y1 {
            let base = y as usize * stride;
            self.pixels[base + rect.x0 as usize..base + rect.x1 as usize].fill(packed);
        }
    }

    fn fill_rgba(&mut self, rect: IRect, r: u8, g: u8, b: u8, a: u8) {
        if a == 0 {
            return;
        }
        if a == 255 {
            self.fill(rect, pack_rgb(r, g, b));
            return;
        }
        let rect = rect.intersect(self.clip).intersect(self.bounds());
        if rect.is_empty() {
            return;
        }
        let stride = self.width as usize;
        for y in rect.y0..rect.y1 {
            let row = y as usize * stride;
            for x in rect.x0..rect.x1 {
                let i = row + x as usize;
                self.pixels[i] = blend_pixel(self.pixels[i], r, g, b, a);
            }
        }
    }

    /// Blit one rasterized glyph canvas. Warp's GPU glyph shader treats the
    /// rasterized glyph's **red channel** as coverage for ordinary text.
    /// On Linux/font-kit grayscale glyphs can have an opaque alpha byte across
    /// the whole glyph canvas, so using `src[3]` paints each glyph's bounding
    /// box instead of the glyph shape. Emoji canvases carry real RGBA color.
    fn blit_glyph(
        &mut self,
        dx: i32,
        dy: i32,
        canvas: &fonts::canvas::Canvas,
        paint: (u8, u8, u8, u8),
        is_emoji: bool,
    ) {
        if canvas.format != RasterFormat::Rgba32 {
            return;
        }
        let (sw, sh) = (canvas.size.x(), canvas.size.y());
        if sw <= 0 || sh <= 0 {
            return;
        }
        let dest = IRect {
            x0: dx,
            y0: dy,
            x1: dx + sw,
            y1: dy + sh,
        }
        .intersect(self.clip)
        .intersect(self.bounds());
        if dest.is_empty() {
            return;
        }
        let Some(src_stride_bytes) = canvas_row_stride_bytes(canvas) else {
            return;
        };
        let stride = self.width as usize;
        for y in dest.y0..dest.y1 {
            let src_row = (y - dy) as usize * src_stride_bytes;
            let dst_row = y as usize * stride;
            for x in dest.x0..dest.x1 {
                let o = src_row + (x - dx) as usize * 4;
                let src = &canvas.pixels[o..o + 4];
                let (r, g, b, a) = if is_emoji {
                    (src[0], src[1], src[2], src[3])
                } else {
                    // Keep this in lockstep with glyph_shader.wgsl, which uses
                    // tex_color.r as the grayscale glyph coverage. Alpha is not
                    // the coverage channel for ordinary Linux/font-kit glyphs.
                    (paint.0, paint.1, paint.2, mul_alpha(src[0], paint.3))
                };
                if a == 0 {
                    continue;
                }
                let i = dst_row + x as usize;
                self.pixels[i] = blend_pixel(self.pixels[i], r, g, b, a);
            }
        }
    }

    /// Nearest-neighbor RGBA blit, scaled into `dest`.
    ///
    /// Source mapping is computed from the original destination rectangle,
    /// not the clipped one. Otherwise an image that is partly outside a layer
    /// clip gets squeezed into the visible subsection instead of cropped.
    fn blit_image(&mut self, dest: IRect, image: RgbaImage, opacity: u8) {
        if dest.is_empty() || image.width == 0 || image.height == 0 || opacity == 0 {
            return;
        }
        let original = dest;
        let dest = original.intersect(self.clip).intersect(self.bounds());
        if dest.is_empty() {
            return;
        }
        let (dw, dh) = (
            (original.x1 - original.x0) as u32,
            (original.y1 - original.y0) as u32,
        );
        if dw == 0 || dh == 0 {
            return;
        }
        let stride = self.width as usize;

        // 32.32 fixed-point source stepping. The old implementation did two
        // integer divisions for every destination pixel. Here the only divides
        // are once per blit; the inner loop is add/shift/multiply-free mapping.
        let x_step = ((u64::from(image.width)) << 32) / u64::from(dw);
        let y_step = ((u64::from(image.height)) << 32) / u64::from(dh);
        let x_start = (dest.x0 - original.x0) as u64 * x_step;
        let mut y_fp = (dest.y0 - original.y0) as u64 * y_step;

        for y in dest.y0..dest.y1 {
            let sy = ((y_fp >> 32) as u32).min(image.height - 1) as usize;
            let dst_row = y as usize * stride;
            let mut x_fp = x_start;
            for x in dest.x0..dest.x1 {
                let sx = ((x_fp >> 32) as u32).min(image.width - 1) as usize;
                let o = (sy * image.width as usize + sx) * 4;
                let a = mul_alpha(image.bytes[o + 3], opacity);
                if a != 0 {
                    let i = dst_row + x as usize;
                    self.pixels[i] = blend_pixel(
                        self.pixels[i],
                        image.bytes[o],
                        image.bytes[o + 1],
                        image.bytes[o + 2],
                        a,
                    );
                }
                x_fp += x_step;
            }
            y_fp += y_step;
        }
    }

    /// Warp's GPU icon shader treats the source red channel as coverage and
    /// replaces RGB with the scene-provided icon color.
    fn blit_icon(&mut self, dest: IRect, image: RgbaImage, color: (u8, u8, u8, u8)) {
        if dest.is_empty() || image.width == 0 || image.height == 0 || color.3 == 0 {
            return;
        }
        let original = dest;
        let dest = original.intersect(self.clip).intersect(self.bounds());
        if dest.is_empty() {
            return;
        }
        let (dw, dh) = (
            (original.x1 - original.x0) as u32,
            (original.y1 - original.y0) as u32,
        );
        if dw == 0 || dh == 0 {
            return;
        }
        let stride = self.width as usize;
        let x_step = ((u64::from(image.width)) << 32) / u64::from(dw);
        let y_step = ((u64::from(image.height)) << 32) / u64::from(dh);
        let x_start = (dest.x0 - original.x0) as u64 * x_step;
        let mut y_fp = (dest.y0 - original.y0) as u64 * y_step;

        for y in dest.y0..dest.y1 {
            let sy = ((y_fp >> 32) as u32).min(image.height - 1) as usize;
            let dst_row = y as usize * stride;
            let mut x_fp = x_start;
            for x in dest.x0..dest.x1 {
                let sx = ((x_fp >> 32) as u32).min(image.width - 1) as usize;
                let o = (sy * image.width as usize + sx) * 4;
                let a = mul_alpha(image.bytes[o], color.3);
                if a != 0 {
                    let i = dst_row + x as usize;
                    self.pixels[i] = blend_pixel(self.pixels[i], color.0, color.1, color.2, a);
                }
                x_fp += x_step;
            }
            y_fp += y_step;
        }
    }
}

/// One-shot diagnostics for the first presented frame: scene stats plus a
/// `/tmp/cpufirst.ppm` pixel dump and histogram summary in the app log.
/// Temporary spike observability; remove once the raster is trusted.
pub fn log_first_frame(scene: &Scene, pixels: &[u32], width: u32, height: u32) {
    let mut rects = 0usize;
    let mut glyphs = 0usize;
    let mut images = 0usize;
    let mut layers = 0usize;
    for layer in scene.layers() {
        layers += 1;
        rects += layer.rects.len();
        glyphs += layer.glyphs.len();
        images += layer.images.len() + layer.icons.len();
    }
    let mut bright = 0u64;
    let mut mid = 0u64;
    let mut dark = 0u64;
    let mut distinct = std::collections::HashSet::new();
    for px in pixels {
        let (r, g, b) = unpack_rgb(*px);
        let lum = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
        if lum > 200 {
            bright += 1;
        } else if lum > 40 {
            mid += 1;
        } else {
            dark += 1;
        }
        if distinct.len() < 4096 {
            distinct.insert(*px);
        }
    }
    let total = (width as u64) * (height as u64);
    log::info!(
        "cpu renderer: first frame {width}x{height} scale={} layers={layers} rects={rects} glyphs={glyphs} images={images} bright={bright} mid={mid} dark={dark} total={total} distinct~={}",
        scene.scale_factor(),
        distinct.len(),
    );
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    ppm.reserve((width as usize) * (height as usize) * 3);
    for px in pixels {
        let (r, g, b) = unpack_rgb(*px);
        ppm.extend_from_slice(&[r, g, b]);
    }
    if let Err(err) = std::fs::write("/tmp/cpufirst.ppm", &ppm) {
        log::warn!("cpu renderer: ppm dump failed: {err:?}");
    }
}

/// Raster the whole scene. `pixels` is `width * height` `u32`s.
pub fn draw_scene(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    scene: &Scene,
    font_cache: &fonts::Cache,
    glyph_config: &GlyphConfig,
    glyph_cache: &mut CpuGlyphCache,
) {
    // CPU present uses an opaque native surface. Transparent scene clear is
    // flattened to black; normal full-window scene backgrounds paint over it.
    pixels.fill(pack_rgb(0, 0, 0));
    let scale = scene.scale_factor();
    let frame = IRect::full(width, height);

    for layer in scene.layers() {
        let clip = layer
            .clip_bounds
            .map(|bounds| {
                let scaled = bounds * scale;
                scaled_irect(scaled.origin(), scaled.size()).intersect(frame)
            })
            .unwrap_or(frame);
        if clip.is_empty() {
            continue;
        }
        let mut frame = Frame {
            pixels: &mut *pixels,
            width,
            height,
            clip,
        };

        for rect in &layer.rects {
            // ponytail: square fallback, no radius/border/shadow in v1.
            let scaled = rect.bounds * scale;
            let dest = scaled_irect(scaled.origin(), scaled.size());
            match rect.background {
                Fill::None => {}
                Fill::Solid(color) => {
                    frame.fill_rgba(dest, color.r, color.g, color.b, color.a);
                }
                Fill::Gradient { start_color, .. } => {
                    frame.fill_rgba(
                        dest,
                        start_color.r,
                        start_color.g,
                        start_color.b,
                        start_color.a,
                    );
                }
            }
        }

        // Match the GPU renderer's primitive order: rects -> images/icons -> glyphs.
        for image in &layer.images {
            let scaled = image.bounds * scale;
            let opacity = (image.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
            frame.blit_image(
                scaled_irect(scaled.origin(), scaled.size()),
                RgbaImage {
                    bytes: image.asset.rgba_bytes(),
                    width: image.asset.width(),
                    height: image.asset.height(),
                },
                opacity,
            );
        }

        for icon in &layer.icons {
            let scaled = icon.bounds * scale;
            frame.blit_icon(
                scaled_irect(scaled.origin(), scaled.size()),
                RgbaImage {
                    bytes: icon.asset.rgba_bytes(),
                    width: icon.asset.width(),
                    height: icon.asset.height(),
                },
                (icon.color.r, icon.color.g, icon.color.b, icon.color.a),
            );
        }

        for glyph in &layer.glyphs {
            // ponytail: fade ignored in v1.
            let pos = glyph.position * scale;
            let subpixel = SubpixelAlignment::new(pos);
            let Some(cached) =
                glyph_cache.get(font_cache, glyph.glyph_key, scale, subpixel, glyph_config)
            else {
                continue;
            };
            let origin = pos - subpixel.to_offset() + cached.bounds.origin().to_f32();
            frame.blit_glyph(
                origin.x().round() as i32,
                origin.y().round() as i32,
                &cached.rasterized.canvas,
                (glyph.color.r, glyph.color.g, glyph.color.b, glyph.color.a),
                cached.rasterized.is_emoji,
            );
        }
    }
}
