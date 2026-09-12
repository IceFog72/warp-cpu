use super::{
    blend_pixel, canvas_row_stride_bytes, div255_floor, mul_alpha, pack_rgb, unpack_rgb,
    CpuGlyphCache, Frame, IRect, RgbaImage,
};
use crate::fonts::canvas::{Canvas, RasterFormat};
use crate::rendering::GlyphConfig;
use pathfinder_geometry::vector::Vector2I;

fn irect(x0: i32, y0: i32, x1: i32, y1: i32) -> IRect {
    IRect { x0, y0, x1, y1 }
}

fn frame<'a>(px: &'a mut [u32], width: u32, height: u32, clip: IRect) -> Frame<'a> {
    Frame {
        pixels: px,
        width,
        height,
        clip,
    }
}

fn full_frame<'a>(px: &'a mut [u32], width: u32, height: u32) -> Frame<'a> {
    let clip = IRect::full(width, height);
    frame(px, width, height, clip)
}

#[test]
fn cpu_glyph_cache_starts_empty() {
    let cache = CpuGlyphCache::new(GlyphConfig::default());
    assert_eq!(cache.len(), 0);
}

#[test]
fn pack_unpack_roundtrip() {
    assert_eq!(unpack_rgb(pack_rgb(255, 0, 0)), (255, 0, 0));
    assert_eq!(unpack_rgb(pack_rgb(0, 255, 0)), (0, 255, 0));
    assert_eq!(unpack_rgb(pack_rgb(1, 2, 3)), (1, 2, 3));
    assert_eq!(
        pack_rgb(1, 2, 3) >> 24,
        0,
        "softbuffer high byte must stay zero"
    );
}

#[test]
fn alpha_multiply_rounds() {
    assert_eq!(mul_alpha(255, 255), 255);
    assert_eq!(mul_alpha(255, 128), 128);
    assert_eq!(mul_alpha(128, 128), 64);
    assert_eq!(mul_alpha(0, 255), 0);
}

#[test]
fn fast_div255_matches_integer_division_exhaustively() {
    for value in 0..=65_025u32 {
        assert_eq!(div255_floor(value), (value / 255) as u8, "value={value}");
    }
}

#[test]
fn fast_mul_alpha_matches_reference_exhaustively() {
    for a in 0..=255u32 {
        for b in 0..=255u32 {
            let expected = ((a * b + 127) / 255) as u8;
            assert_eq!(mul_alpha(a as u8, b as u8), expected, "a={a} b={b}");
        }
    }
}

#[test]
fn blend_extremes() {
    let dst = pack_rgb(10, 20, 30);
    assert_eq!(blend_pixel(dst, 200, 100, 50, 0), dst);
    assert_eq!(blend_pixel(dst, 200, 100, 50, 255), pack_rgb(200, 100, 50));
}

#[test]
fn blend_midpoint() {
    // 50% white over black ~= 127 gray per channel.
    assert_eq!(blend_pixel(0, 255, 255, 255, 128), pack_rgb(128, 128, 128));
}

#[test]
fn irect_intersect_clamps() {
    let a = irect(0, 0, 10, 10);
    let b = irect(5, -5, 15, 5);
    assert_eq!(a.intersect(b), irect(5, 0, 10, 5));
    assert!(a.intersect(irect(20, 20, 30, 30)).is_empty());
}

#[test]
fn fill_rect_respects_bounds() {
    let mut px = vec![0u32; 4 * 4];
    full_frame(&mut px, 4, 4).fill(irect(1, 1, 3, 3), pack_rgb(255, 0, 0));
    assert_eq!(px[0], 0);
    assert_eq!(px[5], pack_rgb(255, 0, 0));
    assert_eq!(px[10], pack_rgb(255, 0, 0));
    assert_eq!(px[15], 0);
}

#[test]
fn fill_rect_empty_is_noop() {
    let mut px = vec![7u32; 4];
    full_frame(&mut px, 2, 2).fill(irect(1, 1, 1, 1), pack_rgb(255, 255, 255));
    assert!(px.iter().all(|px| *px == 7));
}

fn coverage_canvas(pixels: Vec<u8>, w: i32, h: i32) -> Canvas {
    Canvas {
        pixels,
        size: Vector2I::new(w, h),
        row_stride: w as usize * 4,
        format: RasterFormat::Rgba32,
    }
}

#[test]
fn glyph_blit_accepts_pixel_stride_reported_by_linux_fontkit() {
    // Reproduces the runtime failure: a 2x2 RGBA canvas whose row_stride is
    // reported as 2 (pixels), while the backing buffer still contains 8 bytes
    // per row. Treating row_stride as bytes starts row 2 halfway through row 1.
    let canvas = Canvas {
        pixels: vec![
            255, 255, 255, 255, 0, 0, 0, 255, // row 0: on, off
            0, 0, 0, 255, 255, 255, 255, 255, // row 1: off, on
        ],
        size: Vector2I::new(2, 2),
        row_stride: 2,
        format: RasterFormat::Rgba32,
    };

    assert_eq!(canvas_row_stride_bytes(&canvas), Some(8));

    let mut px = vec![pack_rgb(0, 0, 0); 4];
    full_frame(&mut px, 2, 2).blit_glyph(0, 0, &canvas, (255, 255, 255, 255), false);
    assert_eq!(px[0], pack_rgb(255, 255, 255));
    assert_eq!(px[1], pack_rgb(0, 0, 0));
    assert_eq!(px[2], pack_rgb(0, 0, 0));
    assert_eq!(px[3], pack_rgb(255, 255, 255));
}

#[test]
fn glyph_stride_normalizer_keeps_documented_byte_stride() {
    let canvas = coverage_canvas(vec![255; 2 * 2 * 4], 2, 2);
    assert_eq!(canvas.row_stride, 8);
    assert_eq!(canvas_row_stride_bytes(&canvas), Some(8));
}

#[test]
fn glyph_blit_applies_paint_and_offset() {
    // 2x1 white coverage mask, full alpha.
    let canvas = coverage_canvas(vec![255, 255, 255, 255, 255, 255, 255, 255], 2, 1);
    let mut px = vec![0u32; 4 * 2];
    full_frame(&mut px, 4, 2).blit_glyph(1, 0, &canvas, (0, 255, 0, 255), false);
    assert_eq!(px[1], pack_rgb(0, 255, 0));
    assert_eq!(px[2], pack_rgb(0, 255, 0));
    assert_eq!(px[0], 0);
    assert_eq!(px[3], 0);
}

#[test]
fn glyph_blit_uses_red_channel_as_coverage() {
    // This reproduces the Linux/font-kit shape that exposed the screenshot bug:
    // alpha can be 255 for the whole Rgba32 canvas while grayscale coverage is
    // carried in RGB. Using src[3] would paint both glyph cells as solid white.
    let canvas = coverage_canvas(
        vec![
            0, 0, 0, 255, // zero coverage, opaque alpha byte
            128, 128, 128, 255, // ~50% coverage, opaque alpha byte
        ],
        2,
        1,
    );
    let mut px = vec![pack_rgb(0, 0, 0); 2];
    full_frame(&mut px, 2, 1).blit_glyph(0, 0, &canvas, (255, 255, 255, 255), false);
    assert_eq!(px[0], pack_rgb(0, 0, 0));
    assert_eq!(px[1], pack_rgb(128, 128, 128));
}

#[test]
fn glyph_blit_clips_to_layer_bounds() {
    let canvas = coverage_canvas(vec![255, 255, 255, 255, 255, 255, 255, 255], 2, 1);
    let mut px = vec![0u32; 4];
    frame(&mut px, 4, 1, irect(0, 0, 1, 1)).blit_glyph(0, 0, &canvas, (255, 0, 0, 255), false);
    assert_eq!(px[0], pack_rgb(255, 0, 0));
    assert_eq!(px[1], 0);
}

#[test]
fn rgba_blit_scales_nearest() {
    // 2x2 src: red, green / blue, white (opaque).
    let src = vec![
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let mut px = vec![0u32; 4 * 4];
    full_frame(&mut px, 4, 4).blit_image(
        irect(0, 0, 4, 4),
        RgbaImage {
            bytes: &src,
            width: 2,
            height: 2,
        },
        255,
    );
    assert_eq!(px[0], pack_rgb(255, 0, 0));
    assert_eq!(px[2], pack_rgb(0, 255, 0));
    assert_eq!(px[8], pack_rgb(0, 0, 255));
    assert_eq!(px[15], pack_rgb(255, 255, 255));
}

#[test]
fn fill_rgba_blends_instead_of_forcing_opaque_source_rgb() {
    let mut px = vec![pack_rgb(0, 0, 0); 1];
    full_frame(&mut px, 1, 1).fill_rgba(irect(0, 0, 1, 1), 255, 255, 255, 128);
    assert_eq!(px[0], pack_rgb(128, 128, 128));
}

#[test]
fn image_opacity_is_applied() {
    let src = [255, 255, 255, 255];
    let mut px = vec![pack_rgb(0, 0, 0); 1];
    full_frame(&mut px, 1, 1).blit_image(
        irect(0, 0, 1, 1),
        RgbaImage {
            bytes: &src,
            width: 1,
            height: 1,
        },
        128,
    );
    assert_eq!(px[0], pack_rgb(128, 128, 128));
}

#[test]
fn image_clip_crops_without_rescaling_source() {
    // Source pixels: red, green, blue, white. Draw width 4 but clip to x=2..4.
    // Correct cropping keeps source pixels 2 and 3 (blue, white). The old code
    // rescaled the full image into the clipped width and produced red/blue.
    let src = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let mut px = vec![0u32; 4];
    frame(&mut px, 4, 1, irect(2, 0, 4, 1)).blit_image(
        irect(0, 0, 4, 1),
        RgbaImage {
            bytes: &src,
            width: 4,
            height: 1,
        },
        255,
    );
    assert_eq!(px[0], 0);
    assert_eq!(px[1], 0);
    assert_eq!(px[2], pack_rgb(0, 0, 255));
    assert_eq!(px[3], pack_rgb(255, 255, 255));
}

#[test]
fn icon_blit_uses_red_as_coverage_and_scene_tint() {
    let src = [128, 0, 0, 255];
    let mut px = vec![pack_rgb(0, 0, 0); 1];
    full_frame(&mut px, 1, 1).blit_icon(
        irect(0, 0, 1, 1),
        RgbaImage {
            bytes: &src,
            width: 1,
            height: 1,
        },
        (200, 100, 50, 255),
    );
    assert_eq!(px[0], pack_rgb(100, 50, 25));
}
