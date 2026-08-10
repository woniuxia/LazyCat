use image::codecs::png::{CompressionType, FilterType};
use image::{AnimationDecoder, DynamicImage, ImageFormat, ImageReader};
use serde_json::{json, Value};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const ACTIONS: &[&str] = &["convert", "compress", "info"];
const MAX_COMPRESS_INPUT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_COMPRESS_PIXELS: u64 = 50_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EncodedFormat {
    Png,
    Jpeg,
    Webp,
    Avif,
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported image action: {action}"));
    }
    match action {
        "convert" => image_convert(payload),
        "compress" => image_compress(payload),
        "info" => image_info(payload),
        _ => Err(format!("unsupported image action: {action}")),
    }
}

fn parse_output_format(payload: &Value) -> Result<EncodedFormat, String> {
    let format = payload["format"].as_str().unwrap_or("png").to_lowercase();
    match format.as_str() {
        "png" => Ok(EncodedFormat::Png),
        "jpeg" | "jpg" => Ok(EncodedFormat::Jpeg),
        "webp" => Ok(EncodedFormat::Webp),
        "avif" => Ok(EncodedFormat::Avif),
        _ => Err(format!("不支持的输出格式: {format}")),
    }
}

fn encoded_format_for_image(format: ImageFormat) -> Option<EncodedFormat> {
    match format {
        ImageFormat::Png => Some(EncodedFormat::Png),
        ImageFormat::Jpeg => Some(EncodedFormat::Jpeg),
        ImageFormat::WebP => Some(EncodedFormat::Webp),
        ImageFormat::Avif => Some(EncodedFormat::Avif),
        _ => None,
    }
}

fn format_label(format: ImageFormat) -> String {
    match format {
        ImageFormat::Png => "PNG".into(),
        ImageFormat::Jpeg => "JPEG".into(),
        ImageFormat::WebP => "WebP".into(),
        ImageFormat::Avif => "AVIF".into(),
        ImageFormat::Bmp => "BMP".into(),
        ImageFormat::Gif => "GIF".into(),
        ImageFormat::Tiff => "TIFF".into(),
        ImageFormat::Ico => "ICO".into(),
        _ => format!("{format:?}"),
    }
}

fn open_image_reader(path: &Path) -> Result<ImageReader<BufReader<fs::File>>, String> {
    let reader = ImageReader::open(path).map_err(|e| format!("open image failed: {e}"))?;
    reader
        .with_guessed_format()
        .map_err(|e| format!("detect image format failed: {e}"))
}

fn image_format(path: &Path) -> Result<ImageFormat, String> {
    let reader = open_image_reader(path)?;
    reader
        .format()
        .ok_or_else(|| "无法识别图片格式".to_string())
}

fn image_dimensions(path: &Path) -> Result<(ImageFormat, u32, u32), String> {
    let reader = open_image_reader(path)?;
    let format = reader
        .format()
        .ok_or_else(|| "无法识别图片格式".to_string())?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("read image dimensions failed: {e}"))?;
    Ok((format, width, height))
}

fn load_image(path: &Path) -> Result<(ImageFormat, DynamicImage), String> {
    let reader = open_image_reader(path)?;
    let format = reader
        .format()
        .ok_or_else(|| "无法识别图片格式".to_string())?;
    let image = reader
        .decode()
        .map_err(|e| format!("open image failed: {e}"))?;
    Ok((format, image))
}

fn ensure_static_gif(path: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|e| format!("open GIF failed: {e}"))?;
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file))
        .map_err(|e| format!("decode GIF failed: {e}"))?;
    let mut frames = decoder.into_frames();
    frames
        .next()
        .transpose()
        .map_err(|e| format!("decode GIF frame failed: {e}"))?;
    if frames
        .next()
        .transpose()
        .map_err(|e| format!("decode GIF frame failed: {e}"))?
        .is_some()
    {
        return Err("动画 GIF 暂不支持转换".into());
    }
    Ok(())
}

fn image_info(payload: &Value) -> Result<Value, String> {
    let input_path = required_path(payload, "inputPath")?;
    if !input_path.exists() {
        return Err("input image not found".into());
    }
    let metadata = fs::metadata(&input_path).map_err(|e| format!("stat failed: {e}"))?;
    let (format, img) = load_image(&input_path)?;
    Ok(json!({
        "width": img.width(),
        "height": img.height(),
        "size": metadata.len(),
        "format": format_label(format)
    }))
}

fn image_convert(payload: &Value) -> Result<Value, String> {
    let input_path = required_path(payload, "inputPath")?;
    let output_path = required_path(payload, "outputPath")?;
    let overwrite = payload["overwrite"].as_bool().unwrap_or(false);
    validate_paths(&input_path, &output_path, overwrite)?;

    let source_format = image_format(&input_path)?;
    if source_format == ImageFormat::Gif {
        ensure_static_gif(&input_path)?;
    }
    let (_, mut img) = load_image(&input_path)?;

    if let Some((x, y, width, height)) = parse_crop(payload, img.width(), img.height())? {
        img = img.crop_imm(x, y, width, height);
    }

    let width = optional_dimension(payload, "width")?;
    let height = optional_dimension(payload, "height")?;
    let final_img = if width.is_some() || height.is_some() {
        let (target_width, target_height) =
            resolve_resize_dimensions(img.width(), img.height(), width, height);
        img.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let format = parse_output_format(payload)?;
    let quality = parse_quality(payload)?;
    let compression_level = parse_compression_level(payload)?;
    if format == EncodedFormat::Jpeg && image_has_transparency(&final_img) {
        return Err("带透明通道的图片不能转换为 JPEG，请先去除透明通道".into());
    }
    let output_size = persist_encoded_image(
        &final_img,
        format,
        quality,
        compression_level,
        &output_path,
        overwrite,
    )?;
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "width": final_img.width(),
      "height": final_img.height(),
      "size": output_size
    }))
}

fn image_compress(payload: &Value) -> Result<Value, String> {
    let input_path = required_path(payload, "inputPath")?;
    let output_path = required_path(payload, "outputPath")?;
    let overwrite = payload["overwrite"].as_bool().unwrap_or(false);
    validate_paths(&input_path, &output_path, overwrite)?;

    let input_size = fs::metadata(&input_path)
        .map_err(|e| format!("stat input failed: {e}"))?
        .len();
    if input_size > MAX_COMPRESS_INPUT_BYTES {
        return Err("压缩输入图片不能超过 100 MB".into());
    }

    let (source_format, width, height) = image_dimensions(&input_path)?;
    let format = encoded_format_for_image(source_format)
        .ok_or("压缩仅支持 PNG、JPEG、WebP、AVIF，BMP/GIF/TIFF 不支持".to_string())?;
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_COMPRESS_PIXELS {
        return Err("压缩输入图片的像素数不能超过 50 MP".into());
    }

    let (_, image) = load_image(&input_path)?;
    let quality = parse_quality(payload)?;
    let compression_level = parse_compression_level(payload)?;
    let output_size = persist_encoded_image(
        &image,
        format,
        quality,
        compression_level,
        &output_path,
        overwrite,
    )?;
    let saved_bytes = input_size as i128 - output_size as i128;
    let compression_ratio = if input_size == 0 {
        0.0
    } else {
        output_size as f64 / input_size as f64 * 100.0
    };

    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "width": width,
      "height": height,
      "inputSize": input_size,
      "size": output_size,
      "savedBytes": saved_bytes,
      "compressionRatio": compression_ratio
    }))
}

fn validate_paths(input_path: &Path, output_path: &Path, overwrite: bool) -> Result<(), String> {
    if !input_path.exists() {
        return Err("input image not found".into());
    }
    if !input_path.is_file() {
        return Err("input image is not a file".into());
    }
    if paths_are_same(input_path, output_path) {
        return Err("输出路径不能与源图片相同".into());
    }
    if output_path.exists() && !overwrite {
        return Err(format!(
            "输出文件已存在：{}。请确认覆盖后重试",
            output_path.display()
        ));
    }
    Ok(())
}

fn parse_quality(payload: &Value) -> Result<u8, String> {
    let raw = payload["quality"].as_u64().unwrap_or(80);
    if !(1..=100).contains(&raw) {
        return Err("quality 必须在 1 到 100 之间".into());
    }
    Ok(raw as u8)
}

fn parse_compression_level(payload: &Value) -> Result<u8, String> {
    let raw = payload["compressionLevel"].as_u64().unwrap_or(6);
    if !(1..=9).contains(&raw) {
        return Err("compressionLevel 必须在 1 到 9 之间".into());
    }
    Ok(raw as u8)
}

fn image_has_transparency(image: &DynamicImage) -> bool {
    image.has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel[3] < u8::MAX)
}

fn persist_encoded_image(
    image: &DynamicImage,
    format: EncodedFormat,
    quality: u8,
    compression_level: u8,
    output_path: &Path,
    overwrite: bool,
) -> Result<u64, String> {
    let output_dir = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_dir).map_err(|e| format!("create output parent failed: {e}"))?;

    let mut temp = NamedTempFile::new_in(output_dir)
        .map_err(|e| format!("create temporary output failed: {e}"))?;
    {
        let file = temp.as_file_mut();
        encode_image(image, format, quality, compression_level, file)?;
        file.sync_all()
            .map_err(|e| format!("flush output failed: {e}"))?;
    }

    if overwrite {
        temp.persist(output_path)
            .map_err(|e| format!("persist output failed: {e}"))?;
    } else {
        temp.persist_noclobber(output_path)
            .map_err(|e| format!("persist output failed: {e}"))?;
    }
    fs::metadata(output_path)
        .map_err(|e| format!("stat output failed: {e}"))
        .map(|metadata| metadata.len())
}

fn encode_image(
    image: &DynamicImage,
    format: EncodedFormat,
    quality: u8,
    compression_level: u8,
    file: &mut fs::File,
) -> Result<(), String> {
    match format {
        EncodedFormat::Jpeg => image
            .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut *file, quality,
            ))
            .map_err(|e| format!("save jpeg failed: {e}")),
        EncodedFormat::Png => image
            .write_with_encoder(image::codecs::png::PngEncoder::new_with_quality(
                &mut *file,
                CompressionType::Level(compression_level),
                FilterType::Adaptive,
            ))
            .map_err(|e| format!("save png failed: {e}")),
        EncodedFormat::Webp => image
            .write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(&mut *file))
            .map_err(|e| format!("save webp failed: {e}")),
        EncodedFormat::Avif => image
            .write_with_encoder(image::codecs::avif::AvifEncoder::new_with_speed_quality(
                &mut *file, 4, quality,
            ))
            .map_err(|e| format!("save avif failed: {e}")),
    }
}

fn required_path(payload: &Value, key: &str) -> Result<PathBuf, String> {
    let raw = payload[key]
        .as_str()
        .ok_or_else(|| format!("缺少参数: {key}"))?
        .trim();
    if raw.is_empty() {
        return Err(format!("参数不能为空: {key}"));
    }
    Ok(PathBuf::from(raw))
}

fn paths_are_same(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn optional_dimension(payload: &Value, key: &str) -> Result<Option<u32>, String> {
    let Some(raw) = payload[key].as_u64() else {
        return Ok(None);
    };
    if raw == 0 {
        return Err(format!("{key} 必须大于 0"));
    }
    u32::try_from(raw)
        .map(Some)
        .map_err(|_| format!("{key} 超出支持范围"))
}

fn parse_crop(
    payload: &Value,
    image_width: u32,
    image_height: u32,
) -> Result<Option<(u32, u32, u32, u32)>, String> {
    let has_crop = ["cropX", "cropY", "cropWidth", "cropHeight"]
        .iter()
        .any(|key| !payload[*key].is_null());
    if !has_crop {
        return Ok(None);
    }

    let x = payload["cropX"].as_u64().unwrap_or(0);
    let y = payload["cropY"].as_u64().unwrap_or(0);
    let width = payload["cropWidth"]
        .as_u64()
        .ok_or("启用裁剪时必须提供裁剪宽度")?;
    let height = payload["cropHeight"]
        .as_u64()
        .ok_or("启用裁剪时必须提供裁剪高度")?;
    if width == 0 || height == 0 {
        return Err("裁剪宽度和高度必须大于 0".into());
    }
    let right = x.checked_add(width).ok_or("裁剪横向范围超出图片边界")?;
    let bottom = y.checked_add(height).ok_or("裁剪纵向范围超出图片边界")?;
    if right > image_width as u64 || bottom > image_height as u64 {
        return Err(format!(
            "裁剪区域超出图片边界（原图 {} x {}）",
            image_width, image_height
        ));
    }

    Ok(Some((x as u32, y as u32, width as u32, height as u32)))
}

fn resolve_resize_dimensions(
    source_width: u32,
    source_height: u32,
    width: Option<u32>,
    height: Option<u32>,
) -> (u32, u32) {
    match (width, height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => {
            let height =
                ((width as f64 * source_height as f64 / source_width as f64).round() as u32).max(1);
            (width, height)
        }
        (None, Some(height)) => {
            let width = ((height as f64 * source_width as f64 / source_height as f64).round()
                as u32)
                .max(1);
            (width, height)
        }
        (None, None) => (source_width, source_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn image_info_and_convert_should_work() {
        let dir = std::env::temp_dir().join(format!("lazycat-image-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let input = dir.join("input.png");
        let output = dir.join("out.jpg");

        let img = image::DynamicImage::new_rgb8(16, 8);
        img.save_with_format(&input, ImageFormat::Png)
            .expect("save input");

        let info = execute(
            "info",
            &json!({ "inputPath": input.to_string_lossy().to_string() }),
        )
        .expect("info");
        assert_eq!(info["width"], 16);
        assert_eq!(info["height"], 8);

        let converted = execute(
            "convert",
            &json!({
                "inputPath": input.to_string_lossy().to_string(),
                "outputPath": output.to_string_lossy().to_string(),
                "format": "jpeg",
                "width": 10,
                "height": 10,
                "quality": 70,
                "overwrite": true
            }),
        )
        .expect("convert");
        assert_eq!(converted["width"], 10);
        assert_eq!(converted["height"], 10);
        assert!(std::path::Path::new(output.to_string_lossy().as_ref()).exists());
    }

    #[test]
    fn compress_should_keep_dimensions_and_report_stats() {
        let dir =
            std::env::temp_dir().join(format!("lazycat-image-compress-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let input = dir.join("input.png");
        let output = dir.join("compressed.png");

        image::DynamicImage::new_rgb8(64, 32)
            .save_with_format(&input, ImageFormat::Png)
            .expect("save input");
        let input_size = std::fs::metadata(&input).expect("stat input").len();

        let compressed = execute(
            "compress",
            &json!({
                "inputPath": input.to_string_lossy(),
                "outputPath": output.to_string_lossy(),
                "compressionLevel": 9,
                "overwrite": true
            }),
        )
        .expect("compress");

        let output_size = std::fs::metadata(&output).expect("stat output").len();
        assert_eq!(compressed["width"], 64);
        assert_eq!(compressed["height"], 32);
        assert_eq!(compressed["inputSize"], input_size);
        assert_eq!(compressed["size"], output_size);
        assert_eq!(
            compressed["savedBytes"].as_i64(),
            Some(input_size as i64 - output_size as i64)
        );
        assert!(compressed["compressionRatio"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn transparent_image_cannot_be_converted_to_jpeg() {
        let dir =
            std::env::temp_dir().join(format!("lazycat-image-transparent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let input = dir.join("input.png");
        let output = dir.join("output.jpg");

        image::DynamicImage::new_rgba8(4, 4)
            .save_with_format(&input, ImageFormat::Png)
            .expect("save input");

        let error = execute(
            "convert",
            &json!({
                "inputPath": input.to_string_lossy(),
                "outputPath": output.to_string_lossy(),
                "format": "jpeg"
            }),
        )
        .unwrap_err();

        assert!(error.contains("透明通道"));
        assert!(!output.exists());
    }

    #[test]
    fn compression_rejects_unsupported_source_format() {
        let dir = std::env::temp_dir().join(format!(
            "lazycat-image-unsupported-compress-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let input = dir.join("input.bmp");
        let output = dir.join("output.bmp");

        image::DynamicImage::new_rgb8(4, 4)
            .save_with_format(&input, ImageFormat::Bmp)
            .expect("save input");

        let error = execute(
            "compress",
            &json!({
                "inputPath": input.to_string_lossy(),
                "outputPath": output.to_string_lossy()
            }),
        )
        .unwrap_err();

        assert!(error.contains("仅支持 PNG、JPEG、WebP、AVIF"));
        assert!(!output.exists());
    }

    #[test]
    fn convert_rejects_existing_output_without_confirmation() {
        let dir =
            std::env::temp_dir().join(format!("lazycat-image-overwrite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let input = dir.join("input.png");
        let output = dir.join("out.png");
        image::DynamicImage::new_rgba8(4, 4)
            .save_with_format(&input, ImageFormat::Png)
            .expect("save input");
        std::fs::write(&output, b"existing").expect("save existing output");

        let error = execute(
            "convert",
            &json!({
                "inputPath": input.to_string_lossy(),
                "outputPath": output.to_string_lossy(),
                "format": "png"
            }),
        )
        .unwrap_err();

        assert!(error.contains("输出文件已存在"));
        assert_eq!(std::fs::read(&output).unwrap(), b"existing");
    }

    #[test]
    fn crop_must_stay_inside_source_image() {
        let error = parse_crop(
            &json!({ "cropX": 8, "cropY": 2, "cropWidth": 4, "cropHeight": 4 }),
            10,
            10,
        )
        .unwrap_err();
        assert!(error.contains("超出图片边界"));
    }

    #[test]
    fn one_dimension_resize_preserves_aspect_ratio() {
        assert_eq!(
            resolve_resize_dimensions(1600, 900, Some(800), None),
            (800, 450)
        );
        assert_eq!(
            resolve_resize_dimensions(1600, 900, None, Some(450)),
            (800, 450)
        );
    }
}
