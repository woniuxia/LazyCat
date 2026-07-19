use image::ImageFormat;
use serde_json::{json, Value};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const ACTIONS: &[&str] = &["convert", "info"];

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
        "info" => image_info(payload),
        _ => Err(format!("unsupported image action: {action}")),
    }
}

fn detect_format_name(path: &std::path::Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" => "PNG".into(),
        "jpg" | "jpeg" => "JPEG".into(),
        "webp" => "WebP".into(),
        "avif" => "AVIF".into(),
        "bmp" => "BMP".into(),
        "gif" => "GIF".into(),
        "tiff" | "tif" => "TIFF".into(),
        "ico" => "ICO".into(),
        _ => ext.to_uppercase(),
    }
}

fn image_info(payload: &Value) -> Result<Value, String> {
    let input_path = PathBuf::from(payload["inputPath"].as_str().unwrap_or_default());
    if !input_path.exists() {
        return Err("input image not found".into());
    }
    let metadata = fs::metadata(&input_path).map_err(|e| format!("stat failed: {e}"))?;
    let img = image::open(&input_path).map_err(|e| format!("open image failed: {e}"))?;
    let format_name = detect_format_name(&input_path);
    Ok(json!({
        "width": img.width(),
        "height": img.height(),
        "size": metadata.len(),
        "format": format_name
    }))
}

fn image_convert(payload: &Value) -> Result<Value, String> {
    let input_path = required_path(payload, "inputPath")?;
    let output_path = required_path(payload, "outputPath")?;
    if !input_path.exists() {
        return Err("input image not found".into());
    }
    if paths_are_same(&input_path, &output_path) {
        return Err("输出路径不能与源图片相同".into());
    }
    let overwrite = payload["overwrite"].as_bool().unwrap_or(false);
    if output_path.exists() && !overwrite {
        return Err(format!(
            "输出文件已存在：{}。请确认覆盖后重试",
            output_path.display()
        ));
    }
    let mut img = image::open(&input_path).map_err(|e| format!("open image failed: {e}"))?;

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

    let format = payload["format"].as_str().unwrap_or("png").to_lowercase();
    let quality = payload["quality"].as_u64().unwrap_or(80).clamp(1, 100) as u8;

    // Create output directory if needed
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|e| format!("create output parent failed: {e}"))?;
    }

    // Save with quality support for JPEG and WebP
    match format.as_str() {
        "jpeg" | "jpg" => {
            let file = fs::File::create(&output_path)
                .map_err(|e| format!("create output file failed: {e}"))?;
            let writer = BufWriter::new(file);
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality);
            final_img
                .write_with_encoder(encoder)
                .map_err(|e| format!("save jpeg failed: {e}"))?;
        }
        "webp" => {
            // image crate's WebP encoder doesn't support quality directly,
            // fall back to save_with_format
            final_img
                .save_with_format(&output_path, ImageFormat::WebP)
                .map_err(|e| format!("save webp failed: {e}"))?;
        }
        "avif" => {
            final_img
                .save_with_format(&output_path, ImageFormat::Avif)
                .map_err(|e| format!("save avif failed: {e}"))?;
        }
        "png" => {
            final_img
                .save_with_format(&output_path, ImageFormat::Png)
                .map_err(|e| format!("save png failed: {e}"))?;
        }
        _ => return Err(format!("不支持的输出格式: {format}")),
    }

    let metadata = fs::metadata(&output_path).map_err(|e| format!("stat output failed: {e}"))?;
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "width": final_img.width(),
      "height": final_img.height(),
      "size": metadata.len()
    }))
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

        let img = image::DynamicImage::new_rgba8(16, 8);
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
