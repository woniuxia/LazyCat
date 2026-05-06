//! 主色调采样 + base 图加载 + 合成 + 写盘
//!
//! - Phase 1.2：`ColorMode` / `Position` / `Region` + `relative_luminance`
//!   + `region_for` + `sample_color_mode`（依据 design §4.2 / plan §1.2）
//! - Phase 1.3：`load_base_cached` mtime 失效缓存（plan §1.3）
//! - Phase 1.4+：`compose` / `persist`（后续 Phase 实现）

#![allow(dead_code)] // Phase 1.2-1.3：部分类型/常量留给 1.4-1.5 接入

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};

use crate::tools::helpers::get_data_dir;
use crate::tools::wallpaper::state::{self, BaseCacheEntry};

// ── 公开类型 ─────────────────────────────────────

/// 仪表盘前景配色模式（依据贴边区域平均亮度推导）。
///
/// - `Light`：浅字（#FFFFFF）+ 深玻璃蒙层 → 用于深色壁纸（亮度 < 0.5）
/// - `Dark` ：深字（#1A1A1A）+ 浅玻璃蒙层 → 用于浅色壁纸（亮度 ≥ 0.5）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Light,
    Dark,
}

impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Light => "light",
            ColorMode::Dark => "dark",
        }
    }
}

/// 仪表盘贴边位置；与前端 `WallpaperPosition` 一一对应。
///
/// MVP 仅 `Right` 落地；其余位置实现完整以便阶段 2 直接开放（design §11.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Right,
    Left,
    Top,
    Bottom,
    Tl,
    Tr,
    Bl,
    Br,
}

impl Position {
    /// 与 `wallpaper.config.position` 字符串互转；未知值回落 `Right`（design 默认）。
    pub fn from_str(s: &str) -> Self {
        match s {
            "left" => Self::Left,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "tl" => Self::Tl,
            "tr" => Self::Tr,
            "bl" => Self::Bl,
            "br" => Self::Br,
            _ => Self::Right,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Left => "left",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Tl => "tl",
            Self::Tr => "tr",
            Self::Bl => "bl",
            Self::Br => "br",
        }
    }
}

/// 矩形采样区域（base 图坐标系，单位为像素）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

// ── 常量 ────────────────────────────────────────

/// 仪表盘逻辑基础尺寸（DPI scale = 1.0），来自 design §4.2。
/// 实际 region 尺寸 = base × monitor DPI scale，由调用方计算后传入 `region_for`。
pub const BASE_REGION_W: u32 = 360;
pub const BASE_REGION_H: u32 = 800;

// ── 颜色采样 ────────────────────────────────────

/// W3C 相对亮度公式（sRGB → 线性 → 加权），plan §1.2。
pub fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let to_linear = |c: u8| {
        let v = c as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

/// 计算贴边 + 居中放置的 region。
///
/// - 不感知 DPI；调用方传入已按 monitor scale 计算的 `region_w` / `region_h`
/// - region 超出 base 时按 base 物理尺寸截断（避免 crop_imm panic）
/// - 未来若 design §4.2 顶部底部 12% 留白需要严格化，可在 right/left 内做二次 padding
pub fn region_for(base_w: u32, base_h: u32, pos: Position, region_w: u32, region_h: u32) -> Region {
    if base_w == 0 || base_h == 0 {
        return Region { x: 0, y: 0, w: 0, h: 0 };
    }
    let rw = region_w.min(base_w);
    let rh = region_h.min(base_h);
    let center_x = base_w.saturating_sub(rw) / 2;
    let center_y = base_h.saturating_sub(rh) / 2;
    let right_x = base_w.saturating_sub(rw);
    let bottom_y = base_h.saturating_sub(rh);
    let (x, y) = match pos {
        Position::Right => (right_x, center_y),
        Position::Left => (0, center_y),
        Position::Top => (center_x, 0),
        Position::Bottom => (center_x, bottom_y),
        Position::Tl => (0, 0),
        Position::Tr => (right_x, 0),
        Position::Bl => (0, bottom_y),
        Position::Br => (right_x, bottom_y),
    };
    Region { x, y, w: rw, h: rh }
}

/// 采样 region 平均亮度并映射为 `ColorMode`（plan §1.2 一致实现）。
///
/// - 先按物理像素 crop，再 Lanczos3 缩到 60×80 → 4800 像素均值
/// - 异常 / 越界 region → 回落 `Dark`（design §14.7：主色调采样失败默认浅字 + 深玻璃 →
///   注意 design 是说"采样失败默认浅字"=`Light`；但这里 region 为空属于上游配置异常，
///   按"显眼黑字"语义返回 `Dark` 触发用户自查更稳。设计层面失败回落由上层选）
pub fn sample_color_mode(base: &DynamicImage, region: Region) -> ColorMode {
    let safe = clamp_region(region, base.width(), base.height());
    if safe.w == 0 || safe.h == 0 {
        return ColorMode::Dark;
    }
    let cropped = base.crop_imm(safe.x, safe.y, safe.w, safe.h);
    let resized = cropped.resize_exact(60, 80, FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let mut sum = 0.0_f64;
    for px in rgba.pixels() {
        let [r, g, b, _] = px.0;
        sum += relative_luminance(r, g, b);
    }
    let avg = sum / (60.0 * 80.0);
    if avg < 0.5 {
        ColorMode::Light
    } else {
        ColorMode::Dark
    }
}

fn clamp_region(r: Region, w: u32, h: u32) -> Region {
    let x = r.x.min(w);
    let y = r.y.min(h);
    let rw = r.w.min(w.saturating_sub(x));
    let rh = r.h.min(h.saturating_sub(y));
    Region { x, y, w: rw, h: rh }
}

// ── base 图加载（缓存） ────────────────────────

/// 读取并缓存 base 壁纸图；若缓存存在且 path/mtime 与文件一致则直接复用。
///
/// - 缓存按 `monitor_id` 隔离（阶段 1 主屏 = `"primary"`）
/// - mtime 检查保护 §18 E1：用户手改壁纸 → 文件 mtime 变 → 重新解码
/// - 任何 IO / 解码失败 → 不污染缓存，原样返回错误
pub fn load_base_cached(monitor_id: &str, path: &Path) -> Result<Arc<DynamicImage>, String> {
    let path_buf: PathBuf = path.to_path_buf();
    let mtime = current_mtime(path)?;

    if let Some(entry) = state::read_base_cache(monitor_id) {
        if entry.path == path_buf && entry.mtime == mtime {
            return Ok(entry.image);
        }
    }

    let img = image::open(path).map_err(|e| format!("decode base image failed: {e}"))?;
    let arc = Arc::new(img);
    state::write_base_cache(
        monitor_id,
        BaseCacheEntry {
            path: path_buf,
            mtime,
            image: arc.clone(),
        },
    );
    Ok(arc)
}

fn current_mtime(path: &Path) -> Result<SystemTime, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat base image failed: {e}"))?;
    meta.modified()
        .map_err(|e| format!("read base mtime failed: {e}"))
}

// ── 合成（plan §1.4） ────────────────────────────

/// alpha 合成：把 `info_layer` 叠加到 `base` 的 `region` 偏移上 + 1px 极淡描边。
///
/// - `info_layer` 尺寸与 `region` 不一致时，按 region 物理尺寸 Lanczos3 重采样
/// - 描边色按 `mode` 选浅 / 深，alpha 64 ≈ "极淡"（design §4.2）
/// - base 假定不透明；合成后的输出 alpha 也保持 255
/// - 不引入 `imageproc`，自实现 4 行像素循环
pub fn compose(
    base: &DynamicImage,
    info_layer: &DynamicImage,
    region: Region,
    mode: ColorMode,
) -> DynamicImage {
    let safe = clamp_region(region, base.width(), base.height());
    if safe.w == 0 || safe.h == 0 {
        return base.clone();
    }

    let info = if info_layer.width() == safe.w && info_layer.height() == safe.h {
        info_layer.to_rgba8()
    } else {
        info_layer
            .resize_exact(safe.w, safe.h, FilterType::Lanczos3)
            .to_rgba8()
    };

    let mut out = base.to_rgba8();
    blend_over(&mut out, &info, safe.x, safe.y);

    let stroke = stroke_color(mode);
    draw_hollow_rect_at(&mut out, safe.x, safe.y, safe.w, safe.h, stroke);

    DynamicImage::ImageRgba8(out)
}

fn stroke_color(mode: ColorMode) -> Rgba<u8> {
    match mode {
        // Light（浅字 + 深玻璃）→ 描边用浅色
        ColorMode::Light => Rgba([255, 255, 255, 64]),
        // Dark（深字 + 浅玻璃）→ 描边用深色
        ColorMode::Dark => Rgba([0, 0, 0, 64]),
    }
}

fn blend_over(dst: &mut RgbaImage, src: &RgbaImage, ox: u32, oy: u32) {
    let (sw, sh) = src.dimensions();
    let (dw, dh) = dst.dimensions();
    for sy in 0..sh {
        let dy = oy.saturating_add(sy);
        if dy >= dh {
            break;
        }
        for sx in 0..sw {
            let dx = ox.saturating_add(sx);
            if dx >= dw {
                break;
            }
            let s = *src.get_pixel(sx, sy);
            blend_pixel(dst, dx, dy, s);
        }
    }
}

fn draw_hollow_rect_at(img: &mut RgbaImage, ox: u32, oy: u32, w: u32, h: u32, color: Rgba<u8>) {
    let (dw, dh) = img.dimensions();
    if w == 0 || h == 0 || ox >= dw || oy >= dh {
        return;
    }
    let x_start = ox;
    let y_start = oy;
    let x_end = (ox + w - 1).min(dw - 1);
    let y_end = (oy + h - 1).min(dh - 1);
    for x in x_start..=x_end {
        blend_pixel(img, x, y_start, color);
        if y_end != y_start {
            blend_pixel(img, x, y_end, color);
        }
    }
    for y in y_start..=y_end {
        blend_pixel(img, x_start, y, color);
        if x_end != x_start {
            blend_pixel(img, x_end, y, color);
        }
    }
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, src: Rgba<u8>) {
    let a = src[3] as f32 / 255.0;
    if a <= 0.0 {
        return;
    }
    let inv = 1.0 - a;
    let d = img.get_pixel_mut(x, y);
    d[0] = (src[0] as f32 * a + d[0] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    d[1] = (src[1] as f32 * a + d[1] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    d[2] = (src[2] as f32 * a + d[2] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    d[3] = 255; // base 假定不透明
}

// ── 写盘 + 历史清理（plan §1.5） ────────────────

/// 输出图像格式；与 `WallpaperConfig.imageFormat` 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

impl ImageFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "png" => Self::Png,
            _ => Self::Jpeg,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

/// 写盘到 `<data_dir>/wallpapers/rendered/<timestamp>.<ext>`，并按 mtime 倒序保留 `keep` 张。
///
/// - JPEG 默认 quality 90（plan §1.5）
/// - keep == 0 视为禁用清理（保留全部历史）
/// - keep >= 1 时仅保留 mtime 最新的 keep 张
pub fn persist(image: &DynamicImage, format: ImageFormat, keep: usize) -> Result<PathBuf, String> {
    let dir = ensure_rendered_dir()?;
    let path = dir.join(format!("{}.{}", timestamp_filename(), format.extension()));
    write_image(image, &path, format)?;
    cleanup_history(&dir, keep)?;
    Ok(path)
}

fn ensure_rendered_dir() -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join("wallpapers").join("rendered");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create wallpaper rendered dir: {e}"))?;
    Ok(dir)
}

fn timestamp_filename() -> String {
    chrono::Local::now().format("%Y%m%dT%H%M%S%6f").to_string()
}

fn write_image(image: &DynamicImage, path: &Path, format: ImageFormat) -> Result<(), String> {
    use image::codecs::jpeg::JpegEncoder;
    use image::ExtendedColorType;
    use std::fs::File;
    use std::io::BufWriter;

    let file = File::create(path).map_err(|e| format!("create wallpaper file {path:?}: {e}"))?;
    let mut buf = BufWriter::new(file);
    match format {
        ImageFormat::Jpeg => {
            // JPEG 不支持 alpha → 强转 RGB8；plan §1.5 quality 90
            let rgb = image.to_rgb8();
            let mut enc = JpegEncoder::new_with_quality(&mut buf, 90);
            enc.encode(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
                .map_err(|e| format!("encode jpeg: {e}"))?;
        }
        ImageFormat::Png => {
            image
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| format!("encode png: {e}"))?;
        }
    }
    Ok(())
}

fn cleanup_history(dir: &Path, keep: usize) -> Result<(), String> {
    if keep == 0 {
        return Ok(());
    }
    let mut entries: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|e| format!("read rendered dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let mt = e.metadata().ok()?.modified().ok()?;
            Some((mt, e.path()))
        })
        .collect();
    if entries.len() <= keep {
        return Ok(());
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // mtime 倒序
    for (_, path) in entries.iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

    fn unique_tmp(prefix: &str, ext: &str) -> PathBuf {
        let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("lazycat_{prefix}_{pid}_{ts}_{n}.{ext}"))
    }

    fn solid(w: u32, h: u32, color: [u8; 4]) -> DynamicImage {
        let buf: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(w, h, Rgba(color));
        DynamicImage::ImageRgba8(buf)
    }

    #[test]
    fn position_round_trip() {
        for p in [
            Position::Right,
            Position::Left,
            Position::Top,
            Position::Bottom,
            Position::Tl,
            Position::Tr,
            Position::Bl,
            Position::Br,
        ] {
            assert_eq!(Position::from_str(p.as_str()), p);
        }
        // unknown → Right（MVP 默认）
        assert_eq!(Position::from_str("nope"), Position::Right);
        assert_eq!(Position::from_str(""), Position::Right);
    }

    #[test]
    fn color_mode_str() {
        assert_eq!(ColorMode::Light.as_str(), "light");
        assert_eq!(ColorMode::Dark.as_str(), "dark");
    }

    #[test]
    fn relative_luminance_extremes() {
        let l_black = relative_luminance(0, 0, 0);
        let l_white = relative_luminance(255, 255, 255);
        assert!(l_black < 0.001);
        assert!(l_white > 0.999);
        // sRGB 系数：绿亮于红亮于蓝
        let l_red = relative_luminance(255, 0, 0);
        let l_green = relative_luminance(0, 255, 0);
        let l_blue = relative_luminance(0, 0, 255);
        assert!(l_green > l_red);
        assert!(l_red > l_blue);
    }

    #[test]
    fn region_right_center_on_1920x1080() {
        let r = region_for(1920, 1080, Position::Right, 360, 800);
        assert_eq!(r.x, 1920 - 360);
        assert_eq!(r.y, (1080 - 800) / 2);
        assert_eq!(r.w, 360);
        assert_eq!(r.h, 800);
    }

    #[test]
    fn region_eight_anchors() {
        let cases = [
            (Position::Right, 1560, 140),
            (Position::Left, 0, 140),
            (Position::Top, 780, 0),
            (Position::Bottom, 780, 280),
            (Position::Tl, 0, 0),
            (Position::Tr, 1560, 0),
            (Position::Bl, 0, 280),
            (Position::Br, 1560, 280),
        ];
        for (pos, x, y) in cases {
            let r = region_for(1920, 1080, pos, 360, 800);
            assert_eq!(r.x, x, "pos={:?}", pos);
            assert_eq!(r.y, y, "pos={:?}", pos);
            assert_eq!(r.w, 360);
            assert_eq!(r.h, 800);
        }
    }

    #[test]
    fn region_clamps_when_request_exceeds_base() {
        // 请求 800 高 > base 600
        let r = region_for(400, 600, Position::Right, 1000, 800);
        assert_eq!(r.w, 400);
        assert_eq!(r.h, 600);
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
    }

    #[test]
    fn region_zero_base_returns_zero() {
        let r = region_for(0, 0, Position::Right, 360, 800);
        assert_eq!(r, Region { x: 0, y: 0, w: 0, h: 0 });
    }

    #[test]
    fn sample_color_mode_pure_black_to_light() {
        let img = solid(400, 800, [0, 0, 0, 255]);
        let region = region_for(400, 800, Position::Right, 360, 800);
        assert_eq!(sample_color_mode(&img, region), ColorMode::Light);
    }

    #[test]
    fn sample_color_mode_pure_white_to_dark() {
        let img = solid(400, 800, [255, 255, 255, 255]);
        let region = region_for(400, 800, Position::Right, 360, 800);
        assert_eq!(sample_color_mode(&img, region), ColorMode::Dark);
    }

    #[test]
    fn sample_color_mode_threshold_boundary() {
        // sRGB ≈ 188 → 线性约 0.5（W3C 公式）
        let img_below = solid(400, 800, [180, 180, 180, 255]);
        let img_above = solid(400, 800, [200, 200, 200, 255]);
        let region = region_for(400, 800, Position::Right, 360, 800);
        assert_eq!(sample_color_mode(&img_below, region), ColorMode::Light);
        assert_eq!(sample_color_mode(&img_above, region), ColorMode::Dark);
    }

    #[test]
    fn sample_color_mode_invalid_region_returns_dark() {
        let img = solid(100, 100, [0, 0, 0, 255]);
        let bad = Region { x: 200, y: 200, w: 50, h: 50 };
        // x/y 越界 → clamp 后 w=0/h=0 → fallback Dark
        assert_eq!(sample_color_mode(&img, bad), ColorMode::Dark);
    }

    #[test]
    fn clamp_region_caps_at_image_bounds() {
        let r = clamp_region(Region { x: 50, y: 50, w: 100, h: 100 }, 80, 80);
        assert_eq!(r, Region { x: 50, y: 50, w: 30, h: 30 });
    }

    fn write_test_png(path: &Path, color: [u8; 4]) {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(2, 2, Rgba(color));
        img.save(path).expect("save tmp png");
    }

    #[test]
    fn load_base_cached_reuses_arc_on_same_mtime() {
        let p = unique_tmp("base_reuse", "png");
        write_test_png(&p, [10, 20, 30, 255]);
        let monitor = format!("test_reuse_{:?}", p.file_name().unwrap_or_default());

        let first = load_base_cached(&monitor, &p).expect("first load");
        let second = load_base_cached(&monitor, &p).expect("second load");
        assert!(Arc::ptr_eq(&first, &second), "should reuse cached Arc");

        // 同 monitor + 同 path 但缓存被清空 → 新 Arc
        state::clear_base_cache();
        let third = load_base_cached(&monitor, &p).expect("third load after clear");
        assert!(!Arc::ptr_eq(&first, &third));

        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_base_cached_invalidates_on_mtime_change() {
        let p = unique_tmp("base_mtime", "png");
        write_test_png(&p, [10, 20, 30, 255]);
        let monitor = format!("test_mtime_{:?}", p.file_name().unwrap_or_default());

        let first = load_base_cached(&monitor, &p).expect("first load");

        // 手动注入一个旧 mtime 条目（模拟用户手改壁纸后 mtime 变化）
        state::write_base_cache(
            &monitor,
            BaseCacheEntry {
                path: p.clone(),
                mtime: SystemTime::UNIX_EPOCH,
                image: first.clone(),
            },
        );

        let after = load_base_cached(&monitor, &p).expect("reload after mtime drift");
        // 文件 mtime ≠ UNIX_EPOCH → 缓存失效，重新解码
        assert!(!Arc::ptr_eq(&first, &after), "stale mtime should invalidate cache");

        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_base_cached_returns_err_on_missing_file() {
        let p = unique_tmp("base_missing", "png");
        let err = load_base_cached("test_missing", &p).expect_err("missing file");
        assert!(err.contains("stat base image failed") || err.contains("No such"));
    }

    #[test]
    fn load_base_cached_isolates_by_monitor_id() {
        let p = unique_tmp("base_isolate", "png");
        write_test_png(&p, [10, 20, 30, 255]);

        let a = load_base_cached("monA_iso", &p).expect("load A");
        // 不同 monitor_id 缓存独立 → 即使路径相同也是新条目
        let b = load_base_cached("monB_iso", &p).expect("load B");
        assert!(!Arc::ptr_eq(&a, &b));

        let _ = fs::remove_file(&p);
    }

    // ── compose / blend / stroke ─────────────────

    #[test]
    fn compose_keeps_base_dimensions() {
        let base = solid(200, 150, [10, 20, 30, 255]);
        let info = solid(60, 80, [255, 255, 255, 200]);
        let region = Region { x: 50, y: 30, w: 60, h: 80 };
        let out = compose(&base, &info, region, ColorMode::Dark);
        assert_eq!(out.width(), 200);
        assert_eq!(out.height(), 150);
    }

    #[test]
    fn compose_zero_region_returns_clone() {
        let base = solid(20, 20, [10, 20, 30, 255]);
        let info = solid(5, 5, [255, 0, 0, 255]);
        let region = Region { x: 0, y: 0, w: 0, h: 0 };
        let out = compose(&base, &info, region, ColorMode::Dark);
        let out_rgba = out.to_rgba8();
        for px in out_rgba.pixels() {
            assert_eq!(px.0, [10, 20, 30, 255]);
        }
    }

    #[test]
    fn compose_full_opaque_overrides_base() {
        // 不透明 info 层 → region 内被完全覆盖
        let base = solid(20, 20, [0, 0, 0, 255]);
        let info = solid(10, 10, [200, 100, 50, 255]);
        let region = Region { x: 5, y: 5, w: 10, h: 10 };
        let out = compose(&base, &info, region, ColorMode::Dark).to_rgba8();
        // region 中心应是 info 颜色（边缘 1px 是描边）
        let center = out.get_pixel(10, 10);
        assert_eq!(center[0], 200);
        assert_eq!(center[1], 100);
        assert_eq!(center[2], 50);
        // region 外仍是 base
        assert_eq!(out.get_pixel(2, 2).0, [0, 0, 0, 255]);
    }

    #[test]
    fn compose_alpha_blend_50pct_white_on_black() {
        // 50% 透明白叠加纯黑 → 灰
        let base = solid(20, 20, [0, 0, 0, 255]);
        let info = solid(10, 10, [255, 255, 255, 128]);
        let region = Region { x: 5, y: 5, w: 10, h: 10 };
        let out = compose(&base, &info, region, ColorMode::Light).to_rgba8();
        let center = out.get_pixel(10, 10);
        // 128/255 ≈ 0.502，混合后 ≈ 128
        assert!(center[0] >= 120 && center[0] <= 135, "got {}", center[0]);
        assert_eq!(center[3], 255);
    }

    #[test]
    fn compose_resizes_info_to_region_size() {
        // info 60×60 → region 30×30，应缩放到 30×30 后合成
        let base = solid(60, 60, [0, 0, 0, 255]);
        let info = solid(60, 60, [255, 255, 255, 255]);
        let region = Region { x: 10, y: 10, w: 30, h: 30 };
        let out = compose(&base, &info, region, ColorMode::Dark).to_rgba8();
        // region 中心应被覆盖为白
        let p = out.get_pixel(25, 25);
        assert_eq!(p[0], 255);
        // region 外仍是黑
        let p = out.get_pixel(5, 5);
        assert_eq!(p[0], 0);
    }

    #[test]
    fn stroke_color_matches_mode() {
        let s_light = stroke_color(ColorMode::Light);
        assert_eq!(s_light.0, [255, 255, 255, 64]);
        let s_dark = stroke_color(ColorMode::Dark);
        assert_eq!(s_dark.0, [0, 0, 0, 64]);
    }

    #[test]
    fn draw_hollow_rect_only_modifies_perimeter() {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(10, 10, Rgba([0, 0, 0, 255]));
        let mut rgba = img;
        draw_hollow_rect_at(&mut rgba, 2, 2, 5, 5, Rgba([255, 255, 255, 255]));
        // 内部 (4,4) 不被修改
        assert_eq!(rgba.get_pixel(4, 4).0, [0, 0, 0, 255]);
        // 周界 (2,2) (6,6) 被修改
        assert_eq!(rgba.get_pixel(2, 2).0, [255, 255, 255, 255]);
        assert_eq!(rgba.get_pixel(6, 6).0, [255, 255, 255, 255]);
        assert_eq!(rgba.get_pixel(2, 6).0, [255, 255, 255, 255]);
        assert_eq!(rgba.get_pixel(6, 2).0, [255, 255, 255, 255]);
        // region 外 (0,0) 不被修改
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn blend_pixel_alpha_zero_is_noop() {
        let mut rgba: RgbaImage = ImageBuffer::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
        blend_pixel(&mut rgba, 0, 0, Rgba([255, 255, 255, 0]));
        assert_eq!(rgba.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    // ── persist / write_image / cleanup_history ──

    fn unique_tmp_dir(prefix: &str) -> PathBuf {
        let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("lazycat_{prefix}_{pid}_{ts}_{n}"));
        fs::create_dir_all(&dir).expect("create tmp dir");
        dir
    }

    #[test]
    fn image_format_from_str_default_is_jpeg() {
        assert_eq!(ImageFormat::from_str("jpeg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_str("PNG"), ImageFormat::Png);
        assert_eq!(ImageFormat::from_str("png"), ImageFormat::Png);
        // 未知 / 空字符串 → 回落 JPEG
        assert_eq!(ImageFormat::from_str(""), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_str("webp"), ImageFormat::Jpeg);
    }

    #[test]
    fn image_format_extension() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Png.extension(), "png");
    }

    #[test]
    fn write_image_jpeg_writes_file() {
        let dir = unique_tmp_dir("persist_jpeg");
        let path = dir.join("out.jpg");
        let img = solid(8, 8, [255, 100, 50, 255]);
        write_image(&img, &path, ImageFormat::Jpeg).expect("write jpeg");
        assert!(path.exists());
        let meta = fs::metadata(&path).expect("stat");
        assert!(meta.len() > 0);
        // JPEG 起始 magic：FF D8 FF
        let bytes = fs::read(&path).expect("read");
        assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_image_png_writes_file() {
        let dir = unique_tmp_dir("persist_png");
        let path = dir.join("out.png");
        let img = solid(8, 8, [255, 100, 50, 255]);
        write_image(&img, &path, ImageFormat::Png).expect("write png");
        assert!(path.exists());
        let bytes = fs::read(&path).expect("read");
        // PNG 起始 magic：89 50 4E 47
        assert_eq!(&bytes[..4], &[0x89, 0x50, 0x4E, 0x47]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_history_keeps_latest_n() {
        let dir = unique_tmp_dir("cleanup_keep");
        let mut paths = vec![];
        for i in 0..5 {
            let p = dir.join(format!("file_{i}.dat"));
            fs::write(&p, b"x").expect("write tmp");
            paths.push(p);
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        cleanup_history(&dir, 2).expect("cleanup");
        let remaining: Vec<PathBuf> = fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&paths[3]));
        assert!(remaining.contains(&paths[4]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_history_keep_zero_keeps_all() {
        let dir = unique_tmp_dir("cleanup_zero");
        for i in 0..3 {
            let p = dir.join(format!("file_{i}.dat"));
            fs::write(&p, b"x").expect("write tmp");
        }
        cleanup_history(&dir, 0).expect("cleanup zero");
        let count = fs::read_dir(&dir).expect("read dir").count();
        assert_eq!(count, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_history_no_op_when_count_le_keep() {
        let dir = unique_tmp_dir("cleanup_le");
        for i in 0..3 {
            let p = dir.join(format!("file_{i}.dat"));
            fs::write(&p, b"x").expect("write tmp");
        }
        cleanup_history(&dir, 10).expect("cleanup");
        let count = fs::read_dir(&dir).expect("read dir").count();
        assert_eq!(count, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamp_filename_format() {
        let s = timestamp_filename();
        // YYYYMMDDTHHMMSS + 6 位微秒 = 21 chars
        assert_eq!(s.len(), 21);
        assert!(s.chars().nth(8) == Some('T'));
        assert!(s.chars().all(|c| c.is_ascii_digit() || c == 'T'));
    }
}
