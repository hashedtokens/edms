//! Render EQP report payloads as Markdown, single-page HTML, or PDF.
//!
//! The webserver owns data collection. It supplies `input_data` containing a
//! single EQP report or a bulk index report. This compute module owns only the
//! deterministic rendering and file creation step.

use chrono::Utc;
use printpdf::{
    BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, TextItem,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const PDF_LINES_PER_PAGE: usize = 48;
const PDF_CHARACTERS_PER_LINE: usize = 92;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("output filename must be a non-empty filename, not a path")]
    InvalidOutputFilename,

    #[error("invalid PDF export input: {0}")]
    InvalidInputData(String),

    #[error("could not write converted file: {0}")]
    Io(#[from] std::io::Error),
}

/// Convert an EQP report payload to Markdown.
///
/// `output_filepath` is the takeout root. The generated file is written to
/// `output_filepath/MD/output_filename_timestamp(UTC).md`.
pub fn convert_to_markdown(
    input_data: &str,
    output_filename: &str,
    output_filepath: impl AsRef<Path>,
) -> Result<PathBuf, ConversionError> {
    let output_path = prepare_output_path(output_filename, output_filepath.as_ref(), "MD", "md")?;
    fs::write(&output_path, render_markdown(input_data))?;
    Ok(output_path)
}

/// Convert an EQP report payload to a standalone HTML document.
pub fn convert_to_html(
    input_data: &str,
    output_filename: &str,
    output_filepath: impl AsRef<Path>,
) -> Result<PathBuf, ConversionError> {
    let output_path =
        prepare_output_path(output_filename, output_filepath.as_ref(), "HTML", "html")?;
    fs::write(&output_path, render_html(input_data))?;
    Ok(output_path)
}

/// Convert an EQP report payload to a basic searchable PDF.
pub fn convert_to_pdf(
    input_data: &str,
    output_filename: &str,
    output_filepath: impl AsRef<Path>,
) -> Result<PathBuf, ConversionError> {
    let output_path =
        prepare_output_path(output_filename, output_filepath.as_ref(), "PDF", "pdf")?;
    let text = render_plain_text(input_data);
    let lines = wrap_lines(&text, PDF_CHARACTERS_PER_LINE);
    let mut document = PdfDocument::new("EDMS EQP report");
    let mut pages = Vec::new();

    let page_chunks: Vec<&[String]> = if lines.is_empty() {
        vec![&[]]
    } else {
        lines.chunks(PDF_LINES_PER_PAGE).collect()
    };

    for page_lines in page_chunks {
        let mut operations = vec![
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point::new(Mm(18.0), Mm(279.0)),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Courier),
                size: Pt(9.0),
            },
            Op::SetLineHeight { lh: Pt(11.0) },
        ];

        for line in page_lines {
            operations.push(Op::ShowText {
                items: vec![TextItem::Text(line.clone())],
            });
            operations.push(Op::AddLineBreak);
        }
        operations.push(Op::EndTextSection);
        pages.push(PdfPage::new(Mm(210.0), Mm(297.0), operations));
    }

    let bytes = document
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    fs::write(&output_path, bytes)?;
    Ok(output_path)
}

/// Generate PDF exports for an EDMS collection.
///
/// `per_eqp` creates one PDF for every EQP.
/// `collection` creates one PDF containing all EQPs.
///
/// Files are written under:
/// `export_root/pdfs/pdf_<collection_name>_<timestampUTC>/`
pub fn export_pdf_collection(
    input_data: &str,
    collection_name: &str,
    mode: &str,
    export_root: impl AsRef<Path>,
) -> Result<PathBuf, ConversionError> {
    validate_output_component(collection_name)?;

    if mode != "per_eqp" && mode != "collection" {
        return Err(ConversionError::InvalidInputData(format!(
            "unsupported PDF mode '{mode}'; expected 'per_eqp' or 'collection'"
        )));
    }

    let payload: Value = serde_json::from_str(input_data)
        .map_err(|error| ConversionError::InvalidInputData(error.to_string()))?;

    let endpoints = payload
        .get("endpoints")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ConversionError::InvalidInputData(
                "input_data must contain an 'endpoints' array".to_string(),
            )
        })?;

    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let export_directory = export_root
        .as_ref()
        .join("pdfs")
        .join(format!("pdf_{collection_name}_{timestamp}"));

    fs::create_dir_all(&export_directory)?;

    write_export_metadata(
        &export_directory,
        collection_name,
        &timestamp,
        mode,
        endpoints,
    )?;

    match mode {
        "per_eqp" => {
            for endpoint in endpoints {
                write_single_eqp_pdf(endpoint, &export_directory)?;
            }
        }
        "collection" => {
            write_combined_collection_pdf(endpoints, collection_name, &export_directory)?;
        }
        _ => unreachable!(),
    }

    Ok(export_directory)
}

fn write_single_eqp_pdf(
    endpoint: &Value,
    export_directory: &Path,
) -> Result<(), ConversionError> {
    let eid = report_eid(endpoint).ok_or_else(|| {
        ConversionError::InvalidInputData("each endpoint must contain an EID".to_string())
    })?;

    validate_output_component(eid)?;

    let output_path = export_directory.join(format!("{eid}.pdf"));
    let text = render_plain_text(&endpoint.to_string());

    write_pdf_from_text(&text, &format!("EDMS EQP report: {eid}"), &output_path)
}

fn write_combined_collection_pdf(
    endpoints: &[Value],
    collection_name: &str,
    export_directory: &Path,
) -> Result<(), ConversionError> {
    let output_path = export_directory.join(format!("{collection_name}.pdf"));
    let mut document = PdfDocument::new(&format!("EDMS collection: {collection_name}"));
    let mut pages = Vec::new();

    for endpoint in endpoints {
        let text = render_plain_text(&endpoint.to_string());
        append_text_pages(&mut pages, &text);
    }

    if pages.is_empty() {
        append_text_pages(
            &mut pages,
            "EDMS COLLECTION EXPORT\n\nNo endpoints supplied.",
        );
    }

    let bytes = document
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    fs::write(output_path, bytes)?;

    Ok(())
}

fn write_export_metadata(
    export_directory: &Path,
    collection_name: &str,
    timestamp: &str,
    mode: &str,
    endpoints: &[Value],
) -> Result<(), ConversionError> {
    let mut records = Vec::new();

    for endpoint in endpoints {
        let eid = report_eid(endpoint).unwrap_or("Unknown");
        let crud = field_string(endpoint, &["crud", "method"]).unwrap_or("UNCLASSIFIED");
        let endpoint_string =
            field_string(endpoint, &["endpoint_string", "endpoint_str", "path"])
                .unwrap_or("Unknown");

        let (endpoint_path, query_params) = split_endpoint_query_params(endpoint_string);

        let qp_data = endpoint
            .get("qp_data")
            .or_else(|| endpoint.get("qps"))
            .and_then(Value::as_array);

        match qp_data {
            Some(qps) if !qps.is_empty() => {
                for qp in qps {
                    let request_number = qp.get("request_number").and_then(Value::as_i64);

                    records.push(serde_json::json!({
                        "eid": eid,
                        "crud_type": crud,
                        "endpoint": endpoint_path,
                        "qp": request_number,
                        "query_params": query_params
                    }));
                }
            }
            _ => {
                records.push(serde_json::json!({
                    "eid": eid,
                    "crud_type": crud,
                    "endpoint": endpoint_path,
                    "qp": Value::Null,
                    "query_params": query_params
                }));
            }
        }
    }

    let metadata = serde_json::json!({
        "collection_name": collection_name,
        "created_at_utc": timestamp,
        "fields": [
            "EID",
            "CRUD Type",
            "Endpoint",
            "QP",
            "Query Params"
        ],
        "pdf_mode": mode,
        "records": records
    });

    let serialized = serde_json::to_string_pretty(&metadata)
        .map_err(|error| ConversionError::InvalidInputData(error.to_string()))?;

    fs::write(export_directory.join("metadata.json"), serialized)?;

    Ok(())
}

fn split_endpoint_query_params(endpoint: &str) -> (String, String) {
    match endpoint.split_once('?') {
        Some((path, query)) => {
            let query = query
                .split_once('#')
                .map(|(query, _)| query)
                .unwrap_or(query);

            (path.to_string(), query.to_string())
        }
        None => (endpoint.to_string(), String::new()),
    }
}

fn write_pdf_from_text(
    text: &str,
    title: &str,
    output_path: &Path,
) -> Result<(), ConversionError> {
    let mut document = PdfDocument::new(title);
    let mut pages = Vec::new();

    append_text_pages(&mut pages, text);

    let bytes = document
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    fs::write(output_path, bytes)?;

    Ok(())
}

/// Add one EQP to the PDF.
///
/// Each call begins with a new page, so every EID begins on its own page.
/// An EQP can still extend across multiple pages.
fn append_text_pages(pages: &mut Vec<PdfPage>, text: &str) {
    let lines = wrap_lines(text, PDF_CHARACTERS_PER_LINE);

    if lines.is_empty() {
        pages.push(create_pdf_page(&[]));
        return;
    }

    for page_lines in lines.chunks(PDF_LINES_PER_PAGE) {
        pages.push(create_pdf_page(page_lines));
    }
}

fn create_pdf_page(page_lines: &[String]) -> PdfPage {
    let mut operations = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(18.0), Mm(279.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Courier),
            size: Pt(9.0),
        },
        Op::SetLineHeight { lh: Pt(11.0) },
    ];

    for line in page_lines {
        operations.push(Op::ShowText {
            items: vec![TextItem::Text(line.clone())],
        });
        operations.push(Op::AddLineBreak);
    }

    operations.push(Op::EndTextSection);
    PdfPage::new(Mm(210.0), Mm(297.0), operations)
}

fn validate_output_component(value: &str) -> Result<(), ConversionError> {
    let supplied = Path::new(value);

    if value.trim().is_empty()
        || supplied.file_name().is_none()
        || supplied.file_name() != Some(supplied.as_os_str())
    {
        return Err(ConversionError::InvalidOutputFilename);
    }

    Ok(())
}

fn prepare_output_path(
    output_filename: &str,
    output_filepath: &Path,
    format_directory: &str,
    extension: &str,
) -> Result<PathBuf, ConversionError> {
    let supplied = Path::new(output_filename);
    if output_filename.trim().is_empty()
        || supplied.file_name().is_none()
        || supplied.file_name() != Some(supplied.as_os_str())
    {
        return Err(ConversionError::InvalidOutputFilename);
    }

    let stem = supplied
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or(ConversionError::InvalidOutputFilename)?;
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let directory = output_filepath.join(format_directory);
    fs::create_dir_all(&directory)?;
    Ok(directory.join(format!("{stem}_{timestamp}.{extension}")))
}

fn render_markdown(input_data: &str) -> String {
    match serde_json::from_str::<Value>(input_data) {
        Ok(value) if is_index_report(&value) => render_index_markdown(&value),
        Ok(value) if report_eid(&value).is_some() => render_eqp_markdown(&value),
        Ok(value) => format!(
            "# EDMS export\n\n```json\n{}\n```\n",
            pretty_json(&value)
        ),
        Err(_) => format!("# EDMS export\n\n{input_data}\n"),
    }
}

fn render_eqp_markdown(value: &Value) -> String {
    let eid = report_eid(value).unwrap_or("Unknown");
    let crud = field_string(value, &["crud", "method"]).unwrap_or("UNCLASSIFIED");
    let endpoint = field_string(value, &["endpoint_string", "endpoint_str", "path"])
        .unwrap_or("Unknown");
    let mut output = format!(
        "# EQP Report: {eid}\n\n## Endpoint\n\n- **CRUD:** {crud}\n- **EID:** {eid}\n- **Endpoint String:** `{endpoint}`\n\n## Tags per EQP\n\n"
    );

    let tags = value.get("tags").and_then(Value::as_array);
    if let Some(tags) = tags.filter(|tags| !tags.is_empty()) {
        for tag in tags {
            output.push_str(&format!("- {}\n", tag.as_str().unwrap_or("Unknown")));
        }
    } else {
        output.push_str("No tags supplied.\n");
    }

    output.push_str("\n## QP data for EID\n\n");
    if let Some(qp_data) = value.get("qp_data").or_else(|| value.get("qps")) {
        output.push_str(&format!("```json\n{}\n```\n", pretty_json(qp_data)));
    } else if let Some(count) = value.get("qpPairs").and_then(Value::as_u64) {
        output.push_str(&format!("- **QP pairs:** {count}\n"));
    } else {
        output.push_str("No QP data supplied.\n");
    }
    output
}

fn render_index_markdown(value: &Value) -> String {
    let mut output = String::from("# EQP Report Index\n\n## Stats\n\n");
    output.push_str(&format!(
        "```json\n{}\n```\n\n## Endpoints\n\n| CRUD | EID | Endpoint String |\n|---|---|---|\n",
        pretty_json(value.get("stats").unwrap_or(&Value::Null))
    ));
    if let Some(endpoints) = value.get("endpoints").and_then(Value::as_array) {
        for endpoint in endpoints {
            let crud = field_string(endpoint, &["crud", "method"]).unwrap_or("UNCLASSIFIED");
            let eid = report_eid(endpoint).unwrap_or("Unknown");
            let endpoint_string =
                field_string(endpoint, &["endpoint_string", "endpoint_str", "path"])
                    .unwrap_or("Unknown");
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                escape_markdown_cell(crud),
                escape_markdown_cell(eid),
                escape_markdown_cell(endpoint_string)
            ));
        }
    }
    output
}

fn render_html(input_data: &str) -> String {
    let content = match serde_json::from_str::<Value>(input_data) {
        Ok(value) if is_index_report(&value) => render_index_html(&value),
        Ok(value) if report_eid(&value).is_some() => render_eqp_html(&value),
        Ok(value) => format!("<h1>EDMS export</h1><pre><code>{}</code></pre>", escape_html(&pretty_json(&value))),
        Err(_) => format!("<h1>EDMS export</h1><pre><code>{}</code></pre>", escape_html(input_data)),
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>EDMS EQP report</title>\n  <style>body{{font:16px/1.5 system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #bbb;padding:.5rem;text-align:left}}pre{{background:#f5f5f5;padding:1rem;overflow:auto}}</style>\n</head>\n<body>\n{content}\n</body>\n</html>\n"
    )
}

fn render_eqp_html(value: &Value) -> String {
    let eid = escape_html(report_eid(value).unwrap_or("Unknown"));
    let crud = escape_html(field_string(value, &["crud", "method"]).unwrap_or("UNCLASSIFIED"));
    let endpoint = escape_html(
        field_string(value, &["endpoint_string", "endpoint_str", "path"]).unwrap_or("Unknown"),
    );
    let mut output = format!(
        "<h1>EQP Report: {eid}</h1><h2>Endpoint</h2><dl><dt>CRUD</dt><dd>{crud}</dd><dt>EID</dt><dd>{eid}</dd><dt>Endpoint String</dt><dd><code>{endpoint}</code></dd></dl><h2>Tags per EQP</h2>"
    );
    if let Some(tags) = value.get("tags").and_then(Value::as_array).filter(|tags| !tags.is_empty()) {
        output.push_str("<ul>");
        for tag in tags {
            output.push_str(&format!("<li>{}</li>", escape_html(tag.as_str().unwrap_or("Unknown"))));
        }
        output.push_str("</ul>");
    } else {
        output.push_str("<p>No tags supplied.</p>");
    }
    output.push_str("<h2>QP data for EID</h2>");
    if let Some(qp_data) = value.get("qp_data").or_else(|| value.get("qps")) {
        output.push_str(&format!("<pre><code>{}</code></pre>", escape_html(&pretty_json(qp_data))));
    } else if let Some(count) = value.get("qpPairs").and_then(Value::as_u64) {
        output.push_str(&format!("<p>QP pairs: {count}</p>"));
    } else {
        output.push_str("<p>No QP data supplied.</p>");
    }
    output
}

fn render_index_html(value: &Value) -> String {
    let stats = escape_html(&pretty_json(value.get("stats").unwrap_or(&Value::Null)));
    let mut output = format!(
        "<h1>EQP Report Index</h1><h2>Stats</h2><pre><code>{stats}</code></pre><h2>Endpoints</h2><table><thead><tr><th>CRUD</th><th>EID</th><th>Endpoint String</th></tr></thead><tbody>"
    );
    if let Some(endpoints) = value.get("endpoints").and_then(Value::as_array) {
        for endpoint in endpoints {
            let crud = escape_html(field_string(endpoint, &["crud", "method"]).unwrap_or("UNCLASSIFIED"));
            let eid = escape_html(report_eid(endpoint).unwrap_or("Unknown"));
            let endpoint_string = escape_html(
                field_string(endpoint, &["endpoint_string", "endpoint_str", "path"])
                    .unwrap_or("Unknown"),
            );
            output.push_str(&format!("<tr><td>{crud}</td><td>{eid}</td><td><code>{endpoint_string}</code></td></tr>"));
        }
    }
    output.push_str("</tbody></table>");
    output
}

fn render_plain_text(input_data: &str) -> String {
    match serde_json::from_str::<Value>(input_data) {
        Ok(value) if is_index_report(&value) => {
            let mut output = format!(
                "EQP REPORT INDEX\n\nSTATS\n{}\n\nCRUD | EID | ENDPOINT STRING\n",
                pretty_json(value.get("stats").unwrap_or(&Value::Null))
            );
            if let Some(endpoints) = value.get("endpoints").and_then(Value::as_array) {
                for endpoint in endpoints {
                    output.push_str(&format!(
                        "{} | {} | {}\n",
                        field_string(endpoint, &["crud", "method"]).unwrap_or("UNCLASSIFIED"),
                        report_eid(endpoint).unwrap_or("Unknown"),
                        field_string(endpoint, &["endpoint_string", "endpoint_str", "path"])
                            .unwrap_or("Unknown")
                    ));
                }
            }
            output
        }
        Ok(value) if report_eid(&value).is_some() => {
            let eid = report_eid(&value).unwrap_or("Unknown");
            let crud = field_string(&value, &["crud", "method"]).unwrap_or("UNCLASSIFIED");
            let endpoint = field_string(&value, &["endpoint_string", "endpoint_str", "path"])
                .unwrap_or("Unknown");
            let tags = value
                .get("tags")
                .and_then(Value::as_array)
                .map(|tags| tags.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
                .filter(|tags| !tags.is_empty())
                .unwrap_or_else(|| "None supplied".to_string());
            let qp = value
                .get("qp_data")
                .or_else(|| value.get("qps"))
                .map(pretty_json)
                .or_else(|| value.get("qpPairs").map(|count| format!("QP pairs: {count}")))
                .unwrap_or_else(|| "No QP data supplied.".to_string());
            format!(
                "EQP REPORT: {eid}\n\nCRUD: {crud}\nEID: {eid}\nEndpoint String: {endpoint}\n\nTags per EQP\n{tags}\n\nQP data for EID\n{qp}\n"
            )
        }
        Ok(value) => format!("EDMS EXPORT\n\n{}\n", pretty_json(&value)),
        Err(_) => format!("EDMS EXPORT\n\n{input_data}\n"),
    }
}

fn is_index_report(value: &Value) -> bool {
    value.get("stats").is_some() && value.get("endpoints").and_then(Value::as_array).is_some()
}

fn report_eid(value: &Value) -> Option<&str> {
    field_string(value, &["eid", "endpoint_id"])
}

fn field_string<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn escape_markdown_cell(input: &str) -> String {
    input.replace('|', "\\|").replace('\n', " ")
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn wrap_lines(input: &str, maximum_characters: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    for source_line in input.lines() {
        let characters: Vec<char> = source_line.chars().collect();
        if characters.is_empty() {
            wrapped.push(String::new());
            continue;
        }
        for part in characters.chunks(maximum_characters) {
            wrapped.push(part.iter().collect());
        }
    }
    wrapped
}