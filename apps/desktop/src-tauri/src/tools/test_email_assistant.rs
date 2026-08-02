use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use quick_xml::escape::unescape;
use quick_xml::Reader;
use serde_json::{json, Map, Value};
use tempfile::{Builder as TempFileBuilder, NamedTempFile};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const ACTIONS: &[&str] = &["inspect_template", "generate_document"];
const MAX_DOCX_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 100 * 1024 * 1024;
const MAX_OUTPUT_NAME_CHARS: usize = 160;

#[derive(Debug, Clone)]
struct TextNode {
    raw_start: usize,
    raw_end: usize,
    text: String,
    separated: bool,
}

#[derive(Debug, Default, Clone)]
struct Paragraph {
    nodes: Vec<TextNode>,
}

#[derive(Debug)]
struct OpenTextNode {
    raw_start: usize,
    text: String,
    separated: bool,
}

#[derive(Debug, Clone)]
struct PlaceholderMatch {
    start: usize,
    end: usize,
    name: String,
}

#[derive(Debug, Clone)]
struct DecodedEdit {
    start: usize,
    end: usize,
    replacement: String,
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported test email assistant action: {action}"));
    }

    match action {
        "inspect_template" => inspect_template_payload(payload),
        "generate_document" => generate_document_payload(payload),
        _ => Err(format!("unsupported test email assistant action: {action}")),
    }
}

fn inspect_template_payload(payload: &Value) -> Result<Value, String> {
    let path = required_template_path(payload)?;
    let placeholders = inspect_docx(&path)?;
    Ok(json!({
        "templatePath": path.to_string_lossy(),
        "placeholders": placeholders,
    }))
}

fn generate_document_payload(payload: &Value) -> Result<Value, String> {
    let template_path = required_template_path(payload)?;
    let values = parse_values(payload.get("values"))?;
    let placeholders = inspect_docx(&template_path)?;
    let missing: Vec<String> = placeholders
        .iter()
        .filter(|name| {
            values
                .get(*name)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        })
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "生成测试报告失败：以下字段未填写：{}",
            missing.join("、")
        ));
    }

    let output_path = write_generated_docx(&template_path, &values, &placeholders)?;
    Ok(json!({
        "outputPath": output_path.to_string_lossy(),
        "fileName": output_path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
    }))
}

fn required_template_path(payload: &Value) -> Result<PathBuf, String> {
    let raw = payload
        .get("templatePath")
        .and_then(Value::as_str)
        .ok_or_else(|| "测试邮件助手失败：templatePath 必须是文件路径".to_string())?
        .trim();
    if raw.is_empty() {
        return Err("测试邮件助手失败：模板路径为空".to_string());
    }

    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(format!("测试邮件助手失败：模板路径必须是绝对路径：{raw}"));
    }
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("docx"))
        != Some(true)
    {
        return Err(format!(
            "检查 DOCX 模板失败：文件必须使用 .docx 后缀：{raw}"
        ));
    }

    let metadata = fs::metadata(&path)
        .map_err(|error| format!("检查 DOCX 模板失败：无法读取文件元数据 {raw}：{error}"))?;
    if !metadata.is_file() {
        return Err(format!("检查 DOCX 模板失败：模板不是普通文件：{raw}"));
    }
    if metadata.len() == 0 || metadata.len() > MAX_DOCX_BYTES {
        return Err(format!(
            "检查 DOCX 模板失败：文件大小必须在 1 字节到 {} MB 之间：{}",
            MAX_DOCX_BYTES / 1024 / 1024,
            metadata.len()
        ));
    }

    Ok(path)
}

fn parse_values(value: Option<&Value>) -> Result<HashMap<String, String>, String> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| "生成测试报告失败：values 必须是对象".to_string())?;
    object
        .iter()
        .map(|(name, value)| {
            let text = value
                .as_str()
                .ok_or_else(|| format!("生成测试报告失败：字段“{name}”的值必须是文本"))?;
            Ok((name.clone(), text.to_string()))
        })
        .collect()
}

fn open_docx(path: &Path) -> Result<ZipArchive<File>, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "检查 DOCX 模板失败：无法打开 {}：{error}",
            path.to_string_lossy()
        )
    })?;
    ZipArchive::new(file).map_err(|error| {
        format!(
            "检查 DOCX 模板失败：文件不是有效的 ZIP/DOCX：{}：{error}",
            path.to_string_lossy()
        )
    })
}

fn validate_docx_structure(archive: &mut ZipArchive<File>) -> Result<(), String> {
    {
        let content_types = archive
            .by_name("[Content_Types].xml")
            .map_err(|error| format!("检查 DOCX 模板失败：缺少 [Content_Types].xml：{error}"))?;
        if content_types.is_dir() {
            return Err("检查 DOCX 模板失败：[Content_Types].xml 不是文件".to_string());
        }
    }
    {
        let document = archive
            .by_name("word/document.xml")
            .map_err(|error| format!("检查 DOCX 模板失败：缺少 word/document.xml：{error}"))?;
        if document.is_dir() {
            return Err("检查 DOCX 模板失败：word/document.xml 不是文件".to_string());
        }
    }
    Ok(())
}

fn inspect_docx(path: &Path) -> Result<Vec<String>, String> {
    let placeholders = collect_docx_placeholders(path)?;
    if placeholders.is_empty() {
        return Err("检查 DOCX 模板失败：Word 模板中未找到有效占位符".to_string());
    }
    Ok(placeholders)
}

fn collect_docx_placeholders(path: &Path) -> Result<Vec<String>, String> {
    let mut archive = open_docx(path)?;
    validate_docx_structure(&mut archive)?;
    let mut placeholders = Vec::new();
    let mut word_xml_entries = Vec::new();

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("检查 DOCX 模板失败：读取 ZIP entry #{index}：{error}"))?;
        let name = entry.name().to_string();
        ensure_entry_size(&name, entry.size())?;
        if is_word_xml(&name) && !entry.is_dir() {
            word_xml_entries.push((index, name));
        }
    }
    word_xml_entries.sort_by_key(|(_, name)| word_xml_priority(name));

    for (index, _) in word_xml_entries {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("检查 DOCX 模板失败：读取 ZIP entry #{index}：{error}"))?;
        let name = entry.name().to_string();
        let mut bytes = Vec::with_capacity(entry.size().min(MAX_ENTRY_BYTES) as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| format!("检查 DOCX 模板失败：读取 {name}：{error}"))?;
        let paragraphs = parse_word_xml(&bytes, &name)?;
        append_unique_placeholders(&paragraphs, &mut placeholders);
    }

    Ok(placeholders)
}

fn ensure_entry_size(name: &str, size: u64) -> Result<(), String> {
    if size > MAX_ENTRY_BYTES {
        return Err(format!(
            "检查 DOCX 模板失败：ZIP entry 过大：{name}（{} MB）",
            size / 1024 / 1024
        ));
    }
    Ok(())
}

fn is_word_xml(name: &str) -> bool {
    name.starts_with("word/") && name.ends_with(".xml")
}

fn word_xml_priority(name: &str) -> u8 {
    if name == "word/document.xml" {
        0
    } else if name.starts_with("word/header") {
        1
    } else if name.starts_with("word/footer") {
        2
    } else {
        3
    }
}

fn parse_word_xml(bytes: &[u8], entry_name: &str) -> Result<Vec<Paragraph>, String> {
    let mut reader = Reader::from_reader(bytes);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => buffer.clear(),
            Err(error) => {
                return Err(format!(
                    "检查 DOCX 模板失败：Word XML 无效：{entry_name}：{error}"
                ));
            }
        }
    }

    scan_word_paragraphs(bytes, entry_name)
}

fn scan_word_paragraphs(bytes: &[u8], entry_name: &str) -> Result<Vec<Paragraph>, String> {
    let mut paragraphs = Vec::new();
    let mut paragraph: Option<Paragraph> = None;
    let mut break_before_next_text = false;
    let mut open_text: Option<OpenTextNode> = None;
    let mut position = 0;

    while position < bytes.len() {
        if bytes[position] != b'<' {
            let end = bytes[position..]
                .iter()
                .position(|byte| *byte == b'<')
                .map(|offset| position + offset)
                .unwrap_or(bytes.len());
            if let Some(text) = open_text.as_mut() {
                text.text
                    .push_str(&decode_xml_text(&bytes[position..end], entry_name)?);
            }
            position = end;
            continue;
        }

        if bytes[position..].starts_with(b"<!--") {
            let end = find_sequence(bytes, position + 4, b"-->", entry_name)? + 3;
            position = end;
            continue;
        }
        if bytes[position..].starts_with(b"<![CDATA[") {
            let content_start = position + 9;
            let content_end = find_sequence(bytes, content_start, b"]]>", entry_name)?;
            if let Some(text) = open_text.as_mut() {
                text.text.push_str(
                    std::str::from_utf8(&bytes[content_start..content_end]).map_err(|error| {
                        format!("检查 DOCX 模板失败：Word XML 编码无效：{entry_name}：{error}")
                    })?,
                );
            }
            position = content_end + 3;
            continue;
        }
        if bytes[position..].starts_with(b"<?") {
            position = find_sequence(bytes, position + 2, b"?>", entry_name)? + 2;
            continue;
        }

        let tag_end = find_tag_end(bytes, position, entry_name)?;
        let tag = &bytes[position + 1..tag_end];
        let (is_end, is_empty, name) = parse_tag_name(tag);
        if let Some(name) = name {
            let is_paragraph = is_w_tag(name, b"p");
            let is_text = is_w_tag(name, b"t");

            if !is_end && is_paragraph {
                if paragraph.is_some() {
                    return Err(format!(
                        "检查 DOCX 模板失败：Word XML 段落嵌套：{entry_name}"
                    ));
                }
                paragraph = Some(Paragraph::default());
                break_before_next_text = false;
                if is_empty {
                    paragraphs.push(paragraph.take().expect("paragraph was just created"));
                }
            } else if !is_end && is_text {
                if paragraph.is_some() {
                    if is_empty {
                        paragraph
                            .as_mut()
                            .expect("paragraph exists")
                            .nodes
                            .push(TextNode {
                                raw_start: tag_end + 1,
                                raw_end: tag_end + 1,
                                text: String::new(),
                                separated: break_before_next_text,
                            });
                        break_before_next_text = false;
                    } else if open_text.is_some() {
                        return Err(format!(
                            "检查 DOCX 模板失败：Word XML 文本节点嵌套：{entry_name}"
                        ));
                    } else {
                        open_text = Some(OpenTextNode {
                            raw_start: tag_end + 1,
                            text: String::new(),
                            separated: break_before_next_text,
                        });
                        break_before_next_text = false;
                    }
                }
            } else if is_end && is_text {
                let Some(open) = open_text.take() else {
                    return Err(format!(
                        "检查 DOCX 模板失败：Word XML 文本节点未闭合：{entry_name}"
                    ));
                };
                if let Some(current) = paragraph.as_mut() {
                    current.nodes.push(TextNode {
                        raw_start: open.raw_start,
                        raw_end: position,
                        text: open.text,
                        separated: open.separated,
                    });
                }
            } else if is_end && is_paragraph {
                if open_text.is_some() {
                    return Err(format!(
                        "检查 DOCX 模板失败：Word XML 文本节点未闭合：{entry_name}"
                    ));
                }
                if let Some(current) = paragraph.take() {
                    paragraphs.push(current);
                }
                break_before_next_text = false;
            } else if paragraph.is_some() && is_hard_break(name) {
                break_before_next_text = true;
            }
        }

        position = tag_end + 1;
    }

    if open_text.is_some() || paragraph.is_some() {
        return Err(format!(
            "检查 DOCX 模板失败：Word XML 结构未闭合：{entry_name}"
        ));
    }
    Ok(paragraphs)
}

fn find_sequence(
    bytes: &[u8],
    start: usize,
    needle: &[u8],
    entry_name: &str,
) -> Result<usize, String> {
    bytes[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
        .ok_or_else(|| format!("检查 DOCX 模板失败：Word XML 结构未闭合：{entry_name}"))
}

fn find_tag_end(bytes: &[u8], start: usize, entry_name: &str) -> Result<usize, String> {
    let mut quote = None;
    for (offset, byte) in bytes[start + 1..].iter().enumerate() {
        match (quote, *byte) {
            (None, b'\'' | b'"') => quote = Some(*byte),
            (Some(current), byte) if current == byte => quote = None,
            (None, b'>') => return Ok(start + 1 + offset),
            _ => {}
        }
    }
    Err(format!(
        "检查 DOCX 模板失败：Word XML 标签未闭合：{entry_name}"
    ))
}

fn parse_tag_name(tag: &[u8]) -> (bool, bool, Option<&[u8]>) {
    let mut content = tag;
    let is_end = content.strip_prefix(b"/").is_some();
    if is_end {
        content = &content[1..];
    }
    content = content.strip_suffix(b"/").unwrap_or(content).trim_ascii();
    let name_end = content
        .iter()
        .position(|byte| byte.is_ascii_whitespace() || *byte == b'/')
        .unwrap_or(content.len());
    let name = &content[..name_end];
    if name.is_empty() {
        return (is_end, false, None);
    }
    (
        is_end,
        !is_end && tag.trim_ascii_end().ends_with(b"/"),
        Some(name),
    )
}

fn is_w_tag(name: &[u8], local_name: &[u8]) -> bool {
    name.strip_prefix(b"w:") == Some(local_name)
}

fn is_hard_break(name: &[u8]) -> bool {
    is_w_tag(name, b"tab") || is_w_tag(name, b"br") || is_w_tag(name, b"cr")
}

fn decode_xml_text(bytes: &[u8], entry_name: &str) -> Result<String, String> {
    let raw = std::str::from_utf8(bytes)
        .map_err(|error| format!("检查 DOCX 模板失败：Word XML 编码无效：{entry_name}：{error}"))?;
    unescape(raw)
        .map(|value| value.into_owned())
        .map_err(|error| {
            format!("检查 DOCX 模板失败：Word XML 文本转义无效：{entry_name}：{error}")
        })
}

fn append_unique_placeholders(paragraphs: &[Paragraph], output: &mut Vec<String>) {
    for paragraph in paragraphs {
        let mut sequence: Vec<&TextNode> = Vec::new();
        for node in &paragraph.nodes {
            if node.separated && !sequence.is_empty() {
                append_sequence_placeholders(&sequence, output);
                sequence.clear();
            }
            sequence.push(node);
        }
        append_sequence_placeholders(&sequence, output);
    }
}

fn append_sequence_placeholders(sequence: &[&TextNode], output: &mut Vec<String>) {
    if sequence.is_empty() {
        return;
    }
    let (combined, _) = combine_nodes(sequence);
    for item in find_placeholders(&combined) {
        if !output.iter().any(|existing| existing == &item.name) {
            output.push(item.name);
        }
    }
}

fn find_placeholders(text: &str) -> Vec<PlaceholderMatch> {
    let mut matches = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let Some(open) = text[cursor..].find("{{").map(|offset| cursor + offset) else {
            break;
        };
        let Some(close_offset) = text[open + 2..].find("}}") else {
            break;
        };
        let close = open + 2 + close_offset;
        let raw_name = &text[open + 2..close];
        let name = raw_name.trim();
        if !raw_name.contains(['{', '}']) && !raw_name.contains(['\r', '\n']) && !name.is_empty() {
            matches.push(PlaceholderMatch {
                start: open,
                end: close + 2,
                name: name.to_string(),
            });
        }
        cursor = close + 2;
    }
    matches
}

fn combine_nodes<'a>(sequence: &[&'a TextNode]) -> (String, Vec<(usize, usize, &'a TextNode)>) {
    let mut combined = String::new();
    let mut ranges = Vec::with_capacity(sequence.len());
    for node in sequence {
        let start = combined.len();
        combined.push_str(&node.text);
        ranges.push((start, combined.len(), *node));
    }
    (combined, ranges)
}

fn replace_word_xml(
    xml: &[u8],
    entry_name: &str,
    values: &HashMap<String, String>,
) -> Result<Vec<u8>, String> {
    let paragraphs = parse_word_xml(xml, entry_name)?;
    let mut byte_edits: Vec<(usize, usize, String)> = Vec::new();

    for paragraph in paragraphs {
        let mut sequence_start = 0;
        while sequence_start < paragraph.nodes.len() {
            let mut sequence_end = sequence_start + 1;
            while sequence_end < paragraph.nodes.len() && !paragraph.nodes[sequence_end].separated {
                sequence_end += 1;
            }
            let sequence = &paragraph.nodes[sequence_start..sequence_end];
            let sequence_refs: Vec<&TextNode> = sequence.iter().collect();
            let (combined, ranges) = combine_nodes(&sequence_refs);
            let matches = find_placeholders(&combined);
            if !matches.is_empty() {
                let mut node_edits: Vec<Vec<DecodedEdit>> = vec![Vec::new(); sequence.len()];
                for item in matches {
                    let value = values.get(&item.name).ok_or_else(|| {
                        format!("生成测试报告失败：字段“{}”缺少填写值", item.name)
                    })?;
                    validate_xml_value(value, &item.name)?;
                    let first = ranges
                        .iter()
                        .position(|(_, end, _)| *end > item.start)
                        .expect("placeholder starts in a text node");
                    let last = ranges
                        .iter()
                        .rposition(|(start, _, _)| *start < item.end)
                        .expect("placeholder ends in a text node");

                    if first == last {
                        let node_start = ranges[first].0;
                        node_edits[first].push(DecodedEdit {
                            start: item.start - node_start,
                            end: item.end - node_start,
                            replacement: value.clone(),
                        });
                    } else {
                        let first_start = ranges[first].0;
                        let first_end = ranges[first].1;
                        node_edits[first].push(DecodedEdit {
                            start: item.start - first_start,
                            end: first_end - first_start,
                            replacement: value.clone(),
                        });
                        for edits in node_edits.iter_mut().take(last).skip(first + 1) {
                            edits.push(DecodedEdit {
                                start: 0,
                                end: usize::MAX,
                                replacement: String::new(),
                            });
                        }
                        let last_start = ranges[last].0;
                        node_edits[last].push(DecodedEdit {
                            start: 0,
                            end: item.end - last_start,
                            replacement: String::new(),
                        });
                    }
                }

                for (node_index, edits) in node_edits.into_iter().enumerate() {
                    if edits.is_empty() {
                        continue;
                    }
                    let mut text = sequence[node_index].text.clone();
                    let mut edits = edits;
                    edits.sort_by(|left, right| right.start.cmp(&left.start));
                    for edit in edits {
                        let end = if edit.end == usize::MAX {
                            text.len()
                        } else {
                            edit.end
                        };
                        text.replace_range(edit.start..end, &edit.replacement);
                    }
                    byte_edits.push((
                        sequence[node_index].raw_start,
                        sequence[node_index].raw_end,
                        escape_word_text(&text),
                    ));
                }
            }
            sequence_start = sequence_end;
        }
    }

    byte_edits.sort_by(|left, right| right.0.cmp(&left.0));
    let mut output = xml.to_vec();
    for (start, end, replacement) in byte_edits {
        output.splice(start..end, replacement.into_bytes());
    }
    Ok(output)
}

fn validate_xml_value(value: &str, field_name: &str) -> Result<(), String> {
    if value
        .chars()
        .any(|character| !matches!(character, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'))
    {
        return Err(format!(
            "生成测试报告失败：字段“{field_name}”包含 XML 不支持的控制字符"
        ));
    }
    Ok(())
}

fn escape_word_text(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::new();
    for (index, line) in normalized.split('\n').enumerate() {
        if index > 0 {
            output.push_str("</w:t><w:br/><w:t>");
        }
        for character in line.chars() {
            match character {
                '&' => output.push_str("&amp;"),
                '<' => output.push_str("&lt;"),
                '>' => output.push_str("&gt;"),
                _ => output.push(character),
            }
        }
    }
    output
}

fn write_generated_docx(
    template_path: &Path,
    values: &HashMap<String, String>,
    placeholders: &[String],
) -> Result<PathBuf, String> {
    let output_dir = template_path
        .parent()
        .ok_or_else(|| "生成测试报告失败：模板没有可用的所在目录".to_string())?;
    let source_file = File::open(template_path)
        .map_err(|error| format!("生成测试报告失败：无法重新打开模板：{error}"))?;
    let mut source = ZipArchive::new(source_file)
        .map_err(|error| format!("生成测试报告失败：无法读取模板 ZIP：{error}"))?;
    let mut temp = TempFileBuilder::new()
        .prefix(".lazycat-test-email-")
        .suffix(".docx")
        .tempfile_in(output_dir)
        .map_err(|error| format!("生成测试报告失败：无法创建同目录临时文件：{error}"))?;

    {
        let mut writer = ZipWriter::new(&mut temp);
        for index in 0..source.len() {
            let mut entry = source
                .by_index(index)
                .map_err(|error| format!("生成测试报告失败：读取 ZIP entry #{index}：{error}"))?;
            let name = entry.name().to_string();
            let options = SimpleFileOptions::default().compression_method(entry.compression());
            ensure_entry_size(&name, entry.size())?;
            let mut data = Vec::with_capacity(entry.size().min(MAX_ENTRY_BYTES) as usize);
            entry
                .read_to_end(&mut data)
                .map_err(|error| format!("生成测试报告失败：读取 ZIP entry {name}：{error}"))?;
            if entry.is_dir() {
                writer
                    .add_directory(name, options)
                    .map_err(|error| format!("生成测试报告失败：写入 ZIP 目录：{error}"))?;
            } else if is_word_xml(&name) {
                let replaced = replace_word_xml(&data, &name, values)?;
                writer
                    .start_file(name, options)
                    .map_err(|error| format!("生成测试报告失败：写入 Word XML：{error}"))?;
                writer
                    .write_all(&replaced)
                    .map_err(|error| format!("生成测试报告失败：写入 Word XML 内容：{error}"))?;
            } else {
                writer
                    .start_file(name, options)
                    .map_err(|error| format!("生成测试报告失败：写入 ZIP entry：{error}"))?;
                writer
                    .write_all(&data)
                    .map_err(|error| format!("生成测试报告失败：写入 ZIP entry 内容：{error}"))?;
            }
        }
        writer
            .finish()
            .map_err(|error| format!("生成测试报告失败：完成 ZIP：{error}"))?;
    }
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("生成测试报告失败：刷新临时文件：{error}"))?;

    let generated_placeholders = collect_docx_placeholders(temp.path())?;
    let remaining: Vec<String> = generated_placeholders
        .into_iter()
        .filter(|name| placeholders.iter().any(|original| original == name))
        .collect();
    if !remaining.is_empty() {
        return Err(format!(
            "生成测试报告失败：生成包仍包含占位符：{}",
            remaining.join("、")
        ));
    }

    let base_name = suggested_file_name(template_path, values);
    persist_without_overwrite(temp, output_dir, &base_name)
}

fn suggested_file_name(template_path: &Path, values: &HashMap<String, String>) -> String {
    let application = values
        .get("应用系统名称")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let feature = values
        .get("功能需求内容")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let raw = match (application, feature) {
        (Some(application), Some(feature)) => {
            format!("{application}-{feature}-测试报告")
        }
        _ => format!(
            "{}-测试报告",
            template_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("测试报告模板")
        ),
    };
    let cleaned = sanitize_file_name(&raw);
    format!(
        "{}.docx",
        if cleaned.is_empty() {
            "测试报告"
        } else {
            &cleaned
        }
    )
}

fn sanitize_file_name(value: &str) -> String {
    let mut cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    cleaned
        .chars()
        .take(MAX_OUTPUT_NAME_CHARS)
        .collect::<String>()
        .trim_end_matches(['.', ' '])
        .to_string()
}

fn persist_without_overwrite(
    mut temp: NamedTempFile,
    output_dir: &Path,
    base_name: &str,
) -> Result<PathBuf, String> {
    let base = base_name.strip_suffix(".docx").unwrap_or(base_name);
    let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    for attempt in 0..1000u32 {
        let stem = if attempt == 0 {
            base.to_string()
        } else if attempt == 1 {
            format!("{base}-{timestamp}")
        } else {
            let suffix = attempt - 1;
            format!("{base}-{timestamp}-{suffix}")
        };
        let candidate = output_dir.join(format!("{stem}.docx"));
        match temp.persist_noclobber(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(persist_error)
                if persist_error.error.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                temp = persist_error.file;
            }
            Err(persist_error) => {
                return Err(format!(
                    "生成测试报告失败：无法以不覆盖方式保存 {}：{}",
                    candidate.to_string_lossy(),
                    persist_error.error
                ));
            }
        }
    }
    Err("生成测试报告失败：可用输出文件名冲突过多".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;
    use zip::CompressionMethod;

    fn write_docx(path: &Path, document: &str, header: Option<&str>, extra: Option<&[u8]>) {
        let file = File::create(path).expect("create docx");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("[Content_Types].xml", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(
                b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>",
            )
            .unwrap();
        writer
            .start_file("word/document.xml", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(document.as_bytes()).unwrap();
        if let Some(header) = header {
            writer
                .start_file("word/header1.xml", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(header.as_bytes()).unwrap();
        }
        if let Some(extra) = extra {
            writer
                .start_file("custom/data.bin", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(extra).unwrap();
        }
        writer.finish().unwrap();
    }

    fn values(entries: &[(&str, &str)]) -> Value {
        let object: Map<String, Value> = entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
            .collect();
        json!(object)
    }

    #[test]
    fn suggested_file_name_sanitizes_windows_characters_and_falls_back() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("报告模板.docx");
        let named_values = HashMap::from([
            ("应用系统名称".to_string(), "系统:一".to_string()),
            ("功能需求内容".to_string(), "需求/二".to_string()),
        ]);

        assert_eq!(
            suggested_file_name(&path, &named_values),
            "系统_一-需求_二-测试报告.docx"
        );
        assert_eq!(
            suggested_file_name(&path, &HashMap::new()),
            "报告模板-测试报告.docx"
        );
    }

    #[test]
    fn inspect_finds_ordered_placeholders_across_word_parts() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("模板.docx");
        write_docx(
            &path,
            r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>{{应用</w:t></w:r><w:r><w:t>系统名称}}</w:t></w:r></w:p><w:p><w:r><w:t>{{功能需求内容}}</w:t></w:r></w:p></w:body></w:document>"#,
            Some(r#"<w:hdr xmlns:w="x"><w:p><w:r><w:t>{{页眉}}</w:t></w:r></w:p></w:hdr>"#),
            None,
        );
        assert_eq!(
            inspect_docx(&path).unwrap(),
            vec!["应用系统名称", "功能需求内容", "页眉"]
        );
    }

    #[test]
    fn generation_escapes_multiline_values_keeps_entries_and_does_not_change_source() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("模板.docx");
        let document = r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>{{应用</w:t></w:r><w:r><w:t>系统名称}}</w:t></w:r></w:p><w:p><w:r><w:t>{{功能需求内容}}</w:t></w:r></w:p></w:body></w:document>"#;
        write_docx(&path, document, None, Some(b"keep me"));
        let source_before = fs::read(&path).unwrap();
        let output = generate_document_payload(&json!({
            "templatePath": path,
            "values": values(&[("应用系统名称", "系统 & <一>"), ("功能需求内容", "第一行\n第二行")]),
        }))
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), source_before);
        let output_path = PathBuf::from(output["outputPath"].as_str().unwrap());
        assert_ne!(output_path, path);
        let mut archive = ZipArchive::new(File::open(&output_path).unwrap()).unwrap();
        let mut xml = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("系统 &amp; &lt;一&gt;"));
        assert!(xml.contains("第一行</w:t><w:br/><w:t>第二行"));
        assert!(!xml.contains("{{"));
        let mut extra = Vec::new();
        archive
            .by_name("custom/data.bin")
            .unwrap()
            .read_to_end(&mut extra)
            .unwrap();
        assert_eq!(extra, b"keep me");
    }

    #[test]
    fn generation_preserves_zip_entry_compression_methods() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("模板.docx");
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file("[Content_Types].xml", stored).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer.start_file("word/document.xml", deflated).unwrap();
        writer
            .write_all(
                r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>{{字段}}</w:t></w:r></w:p></w:body></w:document>"#
                    .as_bytes(),
            )
            .unwrap();
        writer.start_file("custom/data.bin", stored).unwrap();
        writer.write_all(b"keep me").unwrap();
        writer.finish().unwrap();

        let output = generate_document_payload(&json!({
            "templatePath": path,
            "values": values(&[("字段", "已填写")]),
        }))
        .unwrap();
        let output_path = PathBuf::from(output["outputPath"].as_str().unwrap());
        let mut archive = ZipArchive::new(File::open(output_path).unwrap()).unwrap();
        assert_eq!(
            archive
                .by_name("[Content_Types].xml")
                .unwrap()
                .compression(),
            CompressionMethod::Stored
        );
        assert_eq!(
            archive.by_name("word/document.xml").unwrap().compression(),
            CompressionMethod::Deflated
        );
        assert_eq!(
            archive.by_name("custom/data.bin").unwrap().compression(),
            CompressionMethod::Stored
        );
        let mut extra = Vec::new();
        archive
            .by_name("custom/data.bin")
            .unwrap()
            .read_to_end(&mut extra)
            .unwrap();
        assert_eq!(extra, b"keep me");
    }

    #[test]
    fn generation_rejects_missing_values_and_templates_without_placeholders() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("模板.docx");
        write_docx(
            &path,
            r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>{{字段一}}</w:t></w:r><w:r><w:t>{{字段二}}</w:t></w:r></w:p></w:body></w:document>"#,
            None,
            None,
        );
        let error = generate_document_payload(&json!({
            "templatePath": path,
            "values": values(&[("字段一", "")]),
        }))
        .unwrap_err();
        assert!(error.contains("字段二"));

        let no_placeholder = directory.path().join("空模板.docx");
        write_docx(
            &no_placeholder,
            r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>没有字段</w:t></w:r></w:p></w:body></w:document>"#,
            None,
            None,
        );
        assert!(inspect_docx(&no_placeholder)
            .unwrap_err()
            .contains("未找到有效占位符"));
    }

    #[test]
    fn repeated_generation_never_overwrites_existing_output() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("模板.docx");
        write_docx(
            &path,
            r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>{{应用系统名称}}</w:t></w:r><w:r><w:t>{{功能需求内容}}</w:t></w:r></w:p></w:body></w:document>"#,
            None,
            None,
        );
        let payload = json!({
            "templatePath": path,
            "values": values(&[("应用系统名称", "系统"), ("功能需求内容", "需求")]),
        });
        let first = generate_document_payload(&payload).unwrap();
        let second = generate_document_payload(&payload).unwrap();
        assert_ne!(first["outputPath"], second["outputPath"]);
        assert!(Path::new(first["outputPath"].as_str().unwrap()).exists());
        assert!(Path::new(second["outputPath"].as_str().unwrap()).exists());
    }
}
