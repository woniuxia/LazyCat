use lopdf::{Document, Object};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const ACTIONS: &[&str] = &[
    "info",
    "split",
    "merge",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported pdf action: {action}"));
    }
    match action {
        "info" => pdf_info(payload),
        "split" => pdf_split(payload),
        "merge" => pdf_merge(payload),
        _ => Err(format!("unsupported pdf action: {action}")),
    }
}

/// Extract a text string from a PDF dictionary entry.
/// Handles both UTF-16BE (BOM) and Latin-1/UTF-8 encoded strings.
fn extract_text(obj: &Object) -> String {
    match obj.as_str() {
        Ok(bytes) => {
            if bytes.starts_with(b"\xFE\xFF") {
                // UTF-16BE with BOM
                let chars: Vec<u16> = bytes[2..]
                    .chunks(2)
                    .map(|c| {
                        if c.len() == 2 {
                            u16::from_be_bytes([c[0], c[1]])
                        } else {
                            u16::from_be_bytes([c[0], 0])
                        }
                    })
                    .collect();
                String::from_utf16_lossy(&chars)
            } else {
                // Try UTF-8 first, fall back to lossy
                String::from_utf8_lossy(bytes).into_owned()
            }
        }
        Err(_) => String::new(),
    }
}

/// Read a string field from the Info dictionary
fn get_info_string(info_dict: &lopdf::Dictionary, key: &[u8]) -> String {
    info_dict.get(key).map(extract_text).unwrap_or_default()
}

/// Parse PDF date format `D:YYYYMMDDHHmmSS+HH'mm'` into `YYYY-MM-DD HH:mm:SS`
fn parse_pdf_date(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    // Strip leading "D:" prefix
    let s = raw.strip_prefix("D:").unwrap_or(raw);
    if s.len() < 8 {
        return raw.to_string();
    }
    let year = &s[0..4];
    let month = if s.len() >= 6 { &s[4..6] } else { "01" };
    let day = if s.len() >= 8 { &s[6..8] } else { "01" };
    let hour = if s.len() >= 10 { &s[8..10] } else { "00" };
    let min = if s.len() >= 12 { &s[10..12] } else { "00" };
    let sec = if s.len() >= 14 { &s[12..14] } else { "00" };

    // Extract timezone if present (e.g. +08'00' or -05'00' or Z)
    let tz_part = if s.len() > 14 { &s[14..] } else { "" };
    let tz = if tz_part.is_empty() {
        String::new()
    } else if tz_part.starts_with('Z') {
        " UTC".to_string()
    } else {
        // "+08'00'" -> " +08:00"
        let cleaned = tz_part.replace('\'', "");
        if cleaned.len() >= 3 {
            let sign = &cleaned[0..1];
            let tz_h = &cleaned[1..3];
            let tz_m = if cleaned.len() >= 5 {
                &cleaned[3..5]
            } else {
                "00"
            };
            format!(" {}{}:{}", sign, tz_h, tz_m)
        } else {
            String::new()
        }
    };

    format!("{}-{}-{} {}:{}:{}{}", year, month, day, hour, min, sec, tz)
}

fn pdf_info(payload: &Value) -> Result<Value, String> {
    let path_str = payload["path"].as_str().ok_or("缺少参数: path")?;
    let path = Path::new(path_str);

    if !path.exists() {
        return Err(format!("文件不存在: {}", path_str));
    }

    let file_size = fs::metadata(path)
        .map_err(|e| format!("无法读取文件信息: {e}"))?
        .len();

    let doc = Document::load(path).map_err(|e| format!("无法加载 PDF 文件: {e}"))?;

    let pages = doc.get_pages();
    let page_count = pages.len() as u32;

    // PDF version from document header
    let pdf_version = doc.version.clone();

    // Is encrypted?
    let encrypted = doc.trailer.get(b"Encrypt").is_ok();

    // Extract first page dimensions (MediaBox)
    let page_size = pages.keys().next().and_then(|&page_num| {
        let page_id = pages.get(&page_num)?;
        let page_dict = doc.get_dictionary(*page_id).ok()?;
        // Try MediaBox on the page, then walk up to parent
        let media_box = page_dict.get(b"MediaBox").ok().or_else(|| {
            let parent_ref = page_dict
                .get(b"Parent")
                .ok()
                .and_then(|o| o.as_reference().ok())?;
            let parent_dict = doc.get_dictionary(parent_ref).ok()?;
            parent_dict.get(b"MediaBox").ok()
        });
        if let Some(Object::Array(arr)) = media_box {
            if arr.len() == 4 {
                let x1 = obj_to_f64(&arr[0]).unwrap_or(0.0);
                let y1 = obj_to_f64(&arr[1]).unwrap_or(0.0);
                let x2 = obj_to_f64(&arr[2]).unwrap_or(0.0);
                let y2 = obj_to_f64(&arr[3]).unwrap_or(0.0);
                let w_pt = (x2 - x1).abs();
                let h_pt = (y2 - y1).abs();
                // Convert points to mm (1 pt = 0.3528 mm)
                let w_mm = w_pt * 0.3528;
                let h_mm = h_pt * 0.3528;
                return Some((w_pt, h_pt, w_mm, h_mm));
            }
        }
        None
    });

    let (page_width_pt, page_height_pt, page_width_mm, page_height_mm) =
        page_size.unwrap_or((0.0, 0.0, 0.0, 0.0));

    // Guess paper size from dimensions
    let paper_size = guess_paper_size(page_width_pt, page_height_pt);

    // Extract metadata from trailer -> Info dictionary
    let info_dict = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|info_ref| match info_ref {
            Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict).ok(),
            Object::Dictionary(dict) => Some(dict),
            _ => None,
        });

    let (title, author, subject, keywords, creator, producer, creation_date, mod_date) =
        match info_dict {
            Some(dict) => (
                get_info_string(dict, b"Title"),
                get_info_string(dict, b"Author"),
                get_info_string(dict, b"Subject"),
                get_info_string(dict, b"Keywords"),
                get_info_string(dict, b"Creator"),
                get_info_string(dict, b"Producer"),
                parse_pdf_date(&get_info_string(dict, b"CreationDate")),
                parse_pdf_date(&get_info_string(dict, b"ModDate")),
            ),
            None => Default::default(),
        };

    Ok(json!({
        "pages": page_count,
        "fileSize": file_size,
        "pdfVersion": pdf_version,
        "encrypted": encrypted,
        "pageWidthPt": page_width_pt,
        "pageHeightPt": page_height_pt,
        "pageWidthMm": (page_width_mm * 10.0).round() / 10.0,
        "pageHeightMm": (page_height_mm * 10.0).round() / 10.0,
        "paperSize": paper_size,
        "title": title,
        "author": author,
        "subject": subject,
        "keywords": keywords,
        "creator": creator,
        "producer": producer,
        "creationDate": creation_date,
        "modDate": mod_date,
    }))
}

/// Extract a float from a PDF Object (Integer or Real)
fn obj_to_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Guess standard paper size name from page dimensions in points
fn guess_paper_size(w: f64, h: f64) -> String {
    // Normalize to portrait (w <= h)
    let (w, h) = if w > h { (h, w) } else { (w, h) };
    let tolerance = 3.0; // points

    let sizes: &[(&str, f64, f64)] = &[
        ("A3", 841.89, 1190.55),
        ("A4", 595.28, 841.89),
        ("A5", 419.53, 595.28),
        ("B5", 498.90, 708.66),
        ("Letter", 612.0, 792.0),
        ("Legal", 612.0, 1008.0),
        ("Tabloid", 792.0, 1224.0),
    ];

    for (name, sw, sh) in sizes {
        if (w - sw).abs() < tolerance && (h - sh).abs() < tolerance {
            return name.to_string();
        }
    }
    String::new()
}

/// Parse a page range string like "1-3,5,7-10" into a sorted set of 1-indexed page numbers.
/// Validates against max_page (total page count).
#[allow(dead_code)]
fn parse_ranges(ranges: &str, max_page: u32) -> Result<BTreeSet<u32>, String> {
    let mut result = BTreeSet::new();

    for part in ranges.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start_s, end_s)) = part.split_once('-') {
            let start: u32 = start_s
                .trim()
                .parse()
                .map_err(|_| format!("无效的页码: '{}'", start_s.trim()))?;
            let end: u32 = end_s
                .trim()
                .parse()
                .map_err(|_| format!("无效的页码: '{}'", end_s.trim()))?;

            if start == 0 || end == 0 {
                return Err("页码从 1 开始，不能为 0".into());
            }
            if start > end {
                return Err(format!("无效的页码范围: {start}-{end}（起始大于结束）"));
            }
            if end > max_page {
                return Err(format!("页码 {end} 超出文档总页数 {max_page}"));
            }

            for p in start..=end {
                result.insert(p);
            }
        } else {
            let page: u32 = part.parse().map_err(|_| format!("无效的页码: '{part}'"))?;
            if page == 0 {
                return Err("页码从 1 开始，不能为 0".into());
            }
            if page > max_page {
                return Err(format!("页码 {page} 超出文档总页数 {max_page}"));
            }
            result.insert(page);
        }
    }

    if result.is_empty() {
        return Err("未指定任何有效页码".into());
    }

    Ok(result)
}

/// Parse range string into groups. Each comma-separated part becomes one output file.
/// e.g. "1-3,5,7-10" → [[1,2,3], [5], [7,8,9,10]]
/// If ranges is empty, split every page: [[1],[2],[3],...]
fn parse_range_groups(ranges: &str, max_page: u32) -> Result<Vec<Vec<u32>>, String> {
    if ranges.trim().is_empty() {
        // Split every page individually
        return Ok((1..=max_page).map(|p| vec![p]).collect());
    }

    let mut groups = Vec::new();
    for part in ranges.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start_s, end_s)) = part.split_once('-') {
            let start: u32 = start_s
                .trim()
                .parse()
                .map_err(|_| format!("无效的页码: '{}'", start_s.trim()))?;
            let end: u32 = end_s
                .trim()
                .parse()
                .map_err(|_| format!("无效的页码: '{}'", end_s.trim()))?;

            if start == 0 || end == 0 {
                return Err("页码从 1 开始，不能为 0".into());
            }
            if start > end {
                return Err(format!("无效的页码范围: {start}-{end}（起始大于结束）"));
            }
            if end > max_page {
                return Err(format!("页码 {end} 超出文档总页数 {max_page}"));
            }
            groups.push((start..=end).collect());
        } else {
            let page: u32 = part.parse().map_err(|_| format!("无效的页码: '{part}'"))?;
            if page == 0 {
                return Err("页码从 1 开始，不能为 0".into());
            }
            if page > max_page {
                return Err(format!("页码 {page} 超出文档总页数 {max_page}"));
            }
            groups.push(vec![page]);
        }
    }

    if groups.is_empty() {
        return Err("未指定任何有效页码".into());
    }

    Ok(groups)
}

fn pdf_split(payload: &Value) -> Result<Value, String> {
    let path_str = payload["path"].as_str().ok_or("缺少参数: path")?;
    let output_dir_str = payload["outputDir"].as_str().ok_or("缺少参数: outputDir")?;
    let ranges_str = payload["ranges"].as_str().unwrap_or("");

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("文件不存在: {}", path_str));
    }

    let output_dir = Path::new(output_dir_str);
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| format!("无法创建输出目录: {e}"))?;
    }

    let doc = Document::load(path).map_err(|e| format!("无法加载 PDF 文件: {e}"))?;

    let pages = doc.get_pages();
    let total_pages = pages.len() as u32;

    let groups = parse_range_groups(ranges_str, total_pages)?;

    // Derive base name from source file
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let mut output_files: Vec<Value> = Vec::new();

    for group in &groups {
        // Build filename from page range
        let range_label = if group.len() == 1 {
            format!("{}", group[0])
        } else {
            format!("{}-{}", group[0], group[group.len() - 1])
        };
        let filename = format!("{}_p{}.pdf", stem, range_label);
        let out_path = output_dir.join(&filename);

        let wanted: BTreeSet<u32> = group.iter().cloned().collect();
        let pages_to_delete: Vec<u32> = (1..=total_pages).filter(|p| !wanted.contains(p)).collect();

        let mut new_doc = doc.clone();
        new_doc.delete_pages(&pages_to_delete);
        new_doc.prune_objects();
        new_doc.renumber_objects();

        let out_path_str = out_path.to_string_lossy().to_string();
        new_doc
            .save(&out_path_str)
            .map_err(|e| format!("保存 {} 失败: {e}", filename))?;

        output_files.push(json!({
            "filename": filename,
            "pages": group.len(),
            "path": out_path_str,
        }));
    }

    Ok(json!({
        "files": output_files,
        "totalFiles": output_files.len(),
    }))
}

fn pdf_merge(payload: &Value) -> Result<Value, String> {
    let paths = payload["paths"]
        .as_array()
        .ok_or("缺少参数: paths（应为字符串数组）")?;

    if paths.is_empty() {
        return Err("至少需要一个 PDF 文件".into());
    }

    let output_path_str = payload["outputPath"]
        .as_str()
        .ok_or("缺少参数: outputPath")?;

    let mut documents: Vec<Document> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        let ps = p.as_str().ok_or(format!("paths[{i}] 不是字符串"))?;
        let path = Path::new(ps);
        if !path.exists() {
            return Err(format!("文件不存在: {}", ps));
        }
        let doc = Document::load(path).map_err(|e| format!("无法加载 PDF 文件 '{}': {e}", ps))?;
        documents.push(doc);
    }

    let sources = documents.len();

    // Merge: take the first document, then append pages from the rest
    let mut merged = documents.remove(0);

    for mut other in documents {
        // Renumber objects in the other document to avoid id collisions
        other.renumber_objects_with(merged.max_id + 1);

        // Copy all objects from other into merged
        for (id, object) in other.objects {
            merged.objects.insert(id, object);
        }
        merged.max_id = merged.max_id.max(other.max_id);

        // Get the page tree root of both documents and append pages
        let other_pages_id = other
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .and_then(|id| merged.get_dictionary(id))
            .and_then(|cat| cat.get(b"Pages"))
            .and_then(Object::as_reference)
            .ok();

        let merged_catalog_id = merged
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .ok();

        if let (Some(other_pages_root), Some(catalog_id)) = (other_pages_id, merged_catalog_id) {
            // Get the Kids array from the other document's Pages
            if let Ok(other_pages_dict) = merged.get_dictionary(other_pages_root) {
                if let Ok(other_kids) = other_pages_dict.get(b"Kids") {
                    let other_kids_clone = other_kids.clone();

                    // Get the merged document's Pages dict
                    if let Ok(merged_catalog) = merged.get_dictionary(catalog_id) {
                        if let Ok(merged_pages_ref) =
                            merged_catalog.get(b"Pages").and_then(Object::as_reference)
                        {
                            // Update the parent reference of other pages' kids to point to merged pages root
                            if let Object::Array(ref kids_arr) = other_kids_clone {
                                for kid_ref in kids_arr {
                                    if let Ok(kid_id) = kid_ref.as_reference() {
                                        if let Ok(kid_dict) = merged
                                            .get_object_mut(kid_id)
                                            .and_then(Object::as_dict_mut)
                                        {
                                            kid_dict
                                                .set("Parent", Object::Reference(merged_pages_ref));
                                        }
                                    }
                                }
                            }

                            // Append kids to the merged document's Pages Kids array
                            if let Ok(merged_pages_dict) =
                                merged.get_dictionary_mut(merged_pages_ref)
                            {
                                if let Ok(Object::Array(ref mut merged_kids)) =
                                    merged_pages_dict.get_mut(b"Kids")
                                {
                                    if let Object::Array(ref other_kids_vec) = other_kids_clone {
                                        merged_kids.extend(other_kids_vec.iter().cloned());
                                    }
                                }
                            }

                            // Update Count separately to avoid borrow conflict
                            let new_count = merged.get_pages().len() as i64;
                            if let Ok(merged_pages_dict) =
                                merged.get_dictionary_mut(merged_pages_ref)
                            {
                                merged_pages_dict.set("Count", Object::Integer(new_count));
                            }
                        }
                    }
                }
            }
        }
    }

    let total_pages = merged.get_pages().len();

    merged
        .save(output_path_str)
        .map_err(|e| format!("保存合并后的 PDF 失败: {e}"))?;

    Ok(json!({
        "pages": total_pages,
        "outputPath": output_path_str,
        "sources": sources,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_page() {
        let result = parse_ranges("3", 10).unwrap();
        assert_eq!(result, BTreeSet::from([3]));
    }

    #[test]
    fn parse_range() {
        let result = parse_ranges("2-5", 10).unwrap();
        assert_eq!(result, BTreeSet::from([2, 3, 4, 5]));
    }

    #[test]
    fn parse_mixed() {
        let result = parse_ranges("1-3,5,7-10", 10).unwrap();
        assert_eq!(result, BTreeSet::from([1, 2, 3, 5, 7, 8, 9, 10]));
    }

    #[test]
    fn parse_with_spaces() {
        let result = parse_ranges(" 1 - 3 , 5 , 7 - 10 ", 10).unwrap();
        assert_eq!(result, BTreeSet::from([1, 2, 3, 5, 7, 8, 9, 10]));
    }

    #[test]
    fn parse_single_page_equals_total() {
        let result = parse_ranges("5", 5).unwrap();
        assert_eq!(result, BTreeSet::from([5]));
    }

    #[test]
    fn parse_deduplicate() {
        let result = parse_ranges("1,1,2,2-3", 5).unwrap();
        assert_eq!(result, BTreeSet::from([1, 2, 3]));
    }

    #[test]
    fn parse_error_zero_page() {
        let err = parse_ranges("0", 10).unwrap_err();
        assert!(err.contains("不能为 0"));
    }

    #[test]
    fn parse_error_exceeds_max() {
        let err = parse_ranges("11", 10).unwrap_err();
        assert!(err.contains("超出文档总页数"));
    }

    #[test]
    fn parse_error_inverted_range() {
        let err = parse_ranges("5-3", 10).unwrap_err();
        assert!(err.contains("起始大于结束"));
    }

    #[test]
    fn parse_error_invalid_number() {
        let err = parse_ranges("abc", 10).unwrap_err();
        assert!(err.contains("无效的页码"));
    }

    #[test]
    fn parse_error_empty() {
        let err = parse_ranges("", 10).unwrap_err();
        assert!(err.contains("未指定任何有效页码"));
    }

    #[test]
    fn parse_error_range_exceeds() {
        let err = parse_ranges("8-15", 10).unwrap_err();
        assert!(err.contains("超出文档总页数"));
    }

    #[test]
    fn parse_error_zero_in_range() {
        let err = parse_ranges("0-3", 10).unwrap_err();
        assert!(err.contains("不能为 0"));
    }

    #[test]
    fn pdf_date_full() {
        assert_eq!(
            parse_pdf_date("D:20200713163754+08'00'"),
            "2020-07-13 16:37:54 +08:00"
        );
    }

    #[test]
    fn pdf_date_utc() {
        assert_eq!(
            parse_pdf_date("D:20250606121846Z"),
            "2025-06-06 12:18:46 UTC"
        );
    }

    #[test]
    fn pdf_date_no_tz() {
        assert_eq!(parse_pdf_date("D:20230101120000"), "2023-01-01 12:00:00");
    }

    #[test]
    fn pdf_date_empty() {
        assert_eq!(parse_pdf_date(""), "");
    }
}
