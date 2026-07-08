use image::ImageFormat;
use serde_json::{json, Value};
use std::fs;
use std::io::BufWriter;
use std::path::PathBuf;

const ACTIONS: &[&str] = &[
    "convert",
    "info",
];

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
    let input_path = PathBuf::from(payload["inputPath"].as_str().unwrap_or_default());
    let output_path = PathBuf::from(payload["outputPath"].as_str().unwrap_or_default());
    if !input_path.exists() {
        return Err("input image not found".into());
    }
    let mut img = image::open(&input_path).map_err(|e| format!("open image failed: {e}"))?;

    // Apply cropping if cropWidth and cropHeight provided
    if let (Some(cw), Some(ch)) = (
        payload["cropWidth"].as_u64(),
        payload["cropHeight"].as_u64(),
    ) {
        if cw > 0 && ch > 0 {
            let x = payload["cropX"].as_u64().unwrap_or(0);
            let y = payload["cropY"].as_u64().unwrap_or(0);
            img = img.crop_imm(x as u32, y as u32, cw as u32, ch as u32);
        }
    }

    // Apply resizing only if width or height is explicitly provided
    let has_width = payload["width"].as_u64().is_some();
    let has_height = payload["height"].as_u64().is_some();
    let final_img = if has_width || has_height {
        let width = payload["width"].as_u64().unwrap_or(img.width() as u64) as u32;
        let height = payload["height"].as_u64().unwrap_or(img.height() as u64) as u32;
        img.resize(width, height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let format = payload["format"].as_str().unwrap_or("png").to_lowercase();
    let quality = payload["quality"].as_u64().unwrap_or(80).min(100) as u8;

    // Create output directory if needed
    if let Some(parent) = output_path.parent() {
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
        _ => {
            // PNG
            final_img
                .save_with_format(&output_path, ImageFormat::Png)
                .map_err(|e| format!("save png failed: {e}"))?;
        }
    }

    let metadata = fs::metadata(&output_path).map_err(|e| format!("stat output failed: {e}"))?;
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "width": final_img.width(),
      "height": final_img.height(),
      "size": metadata.len()
    }))
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
                "quality": 70
            }),
        )
        .expect("convert");
        assert_eq!(converted["width"], 10);
        assert_eq!(converted["height"], 5);
        assert!(std::path::Path::new(output.to_string_lossy().as_ref()).exists());
    }
}
