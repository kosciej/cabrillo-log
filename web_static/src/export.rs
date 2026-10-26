//! Standalone HTML and PDF exports of the report.

use crate::common::Data;
use crate::report_doc::{DocProps, ReportDocument, filter_label};
use stats::report::ReportFilter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = cabrilloExport, js_name = download)]
    fn js_download(html: &str, file_name: &str);
    #[wasm_bindgen(js_namespace = cabrilloExport, js_name = print)]
    fn js_print(html: &str);
}

#[derive(Clone, Copy, PartialEq)]
pub enum ExportKind {
    Html,
    Pdf,
}

/// Extra styles for the exported document, on top of the app stylesheet.
const DOC_CSS: &str = r#"
.report-doc { max-width: 1100px; margin: 0 auto; padding: 24px 16px; display: flex; flex-direction: column; gap: 16px; }
.doc-head { padding-bottom: 12px; border-bottom: 2px solid var(--ink); }
.doc-section { font-size: 20px; margin: 16px 0 0; }
.doc-foot { color: var(--muted); font-size: 12px; text-align: center; margin-top: 8px; }
.doc-foot a { color: inherit; }
.doc-dupes { columns: 4 170px; column-gap: 16px; list-style: none; margin: 8px 0 0; padding: 0; font-size: 12px; font-variant-numeric: tabular-nums; }
.doc-dupes li { break-inside: avoid; color: var(--ink-2); }
.doc-dupes .call { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-weight: 600; color: var(--ink); }
.report-doc .legend-item, .report-doc .zone.worked, .report-doc svg .hit { cursor: default; }
@page { size: A4; margin: 12mm; }
@media print {
    * { print-color-adjust: exact; -webkit-print-color-adjust: exact; }
    body { background: #fff; font-size: 10.5px; }
    .report-doc { max-width: none; padding: 0; gap: 10px; }
    .card, .tile { break-inside: avoid; }
    .card:has(table.compact) { break-inside: auto; }
    tr { break-inside: avoid; }
    thead { display: table-header-group; }
    table.data th { position: static; }
    .doc-section { break-before: page; }
    .two-col { grid-template-columns: 1fr 1fr; }
}
"#;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The app's own stylesheet, so the export looks like the app without a network.
fn app_css() -> String {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.query_selector("style").ok().flatten())
        .and_then(|s| s.text_content())
        .unwrap_or_default()
}

fn file_stem(data: &Data, filter: &ReportFilter) -> String {
    let info = &data.log.info;
    let mut parts = vec![info.callsign.clone(), info.contest.clone()];
    if filter != &ReportFilter::default() {
        parts.push(filter_label(filter));
    }
    let stem = parts
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let stem = stem.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    if stem.is_empty() { "cabrillo-report".into() } else { format!("{}-report", stem) }
}

/// Render the report for the current filter and download it or open the print dialog.
pub async fn export(data: Data, filter: ReportFilter, kind: ExportKind) {
    let generated = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .map(|s| format!("{} UTC", s.get(..16).unwrap_or("").replace('T', " ")))
        .unwrap_or_default();
    let info = data.log.info.clone();
    let title = format!(
        "{} {} – {}",
        info.callsign,
        info.contest,
        filter_label(&filter)
    );
    let stem = file_stem(&data, &filter);
    let body = yew::LocalServerRenderer::<ReportDocument>::with_props(DocProps {
        data,
        filter,
        generated,
    })
    .hydratable(false)
    .render()
    .await;

    // The PDF is printed in the light theme; the HTML file follows the viewer's theme.
    let theme = match kind {
        ExportKind::Pdf => r#" data-theme="light""#,
        ExportKind::Html => "",
    };
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\"{theme}>\n<head>\n<meta charset=\"UTF-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n<title>{}</title>\n<style>{}{}</style>\n</head>\n<body>\n{}\n</body>\n</html>\n",
        escape(title.trim()),
        app_css(),
        DOC_CSS,
        body
    );
    match kind {
        ExportKind::Html => js_download(&html, &format!("{}.html", stem)),
        ExportKind::Pdf => js_print(&html),
    }
}
