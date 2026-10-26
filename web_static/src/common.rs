use chrono::NaiveDateTime;
use stats::report::{ContestLog, LogQso, Report};
use std::rc::Rc;
use yew::prelude::*;

/// Log and report shared by all views. Compared by pointer, so views only
/// re-render when a new log is loaded or the filter changes.
#[derive(Clone)]
pub struct Data {
    pub log: Rc<ContestLog>,
    pub report: Rc<Report>,
    /// Open the log tab narrowed to the given QSOs.
    pub on_focus: Callback<Focus>,
    /// Switch the global band filter.
    pub on_band: Callback<Option<String>>,
}

impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.log, &other.log) && Rc::ptr_eq(&self.report, &other.report)
    }
}

impl Data {
    pub fn qsos(&self) -> impl Iterator<Item = (usize, &LogQso)> {
        self.report.qsos.iter().map(|&i| (i, &self.log.qsos[i]))
    }

    pub fn focus(&self, focus: Focus) -> Callback<MouseEvent> {
        let on_focus = self.on_focus.clone();
        Callback::from(move |_| on_focus.emit(focus.clone()))
    }
}

#[derive(Properties, PartialEq)]
pub struct ViewProps {
    pub data: Data,
}

/// A drill-down from any view into the log.
#[derive(Clone, PartialEq, Debug)]
pub enum Focus {
    Country(String),
    Continent(String),
    Zone(u32),
    Hour(NaiveDateTime),
    Call(String),
    Prefix(String),
    Dupes,
    NewMults,
    /// Run (true) or search & pounce (false) QSOs.
    Run(bool),
    Unknown,
}

impl Focus {
    pub fn label(&self) -> String {
        match self {
            Focus::Country(c) => format!("Country: {}", c),
            Focus::Continent(c) => format!("Continent: {}", continent_name(c)),
            Focus::Zone(z) => format!("CQ zone {}", z),
            Focus::Hour(t) => format!("Hour {}", t.format("%a %H:00")),
            Focus::Call(c) => format!("Call: {}", c),
            Focus::Prefix(p) => format!("Prefix: {}", p),
            Focus::Dupes => "Dupes".into(),
            Focus::NewMults => "New multipliers".into(),
            Focus::Run(true) => "Run QSOs".into(),
            Focus::Run(false) => "S&P QSOs".into(),
            Focus::Unknown => "Unknown country".into(),
        }
    }

    pub fn matches(&self, index: usize, qso: &LogQso, report: &Report) -> bool {
        match self {
            Focus::Country(c) => qso.country() == c,
            Focus::Continent(c) => qso.continent() == c,
            Focus::Zone(z) => qso.cq_zone() == *z,
            Focus::Hour(t) => qso.time >= *t && qso.time < *t + chrono::Duration::hours(1),
            Focus::Call(c) => qso.call == *c,
            Focus::Prefix(p) => qso.prefix == *p,
            Focus::Dupes => qso.dupe,
            Focus::NewMults => report.new_dxcc.contains(&index) || report.new_zone.contains(&index),
            Focus::Unknown => qso.entity.is_none(),
            Focus::Run(run) => qso.run == Some(*run),
        }
    }
}

/// Fixed band -> categorical slot, so a band keeps its color under any filter.
pub fn band_color(band: &str) -> &'static str {
    match band {
        "160m" => "var(--s1)",
        "80m" => "var(--s2)",
        "40m" => "var(--s3)",
        "20m" => "var(--s4)",
        "15m" => "var(--s5)",
        "10m" => "var(--s6)",
        "6m" | "4m" | "2m" | "70cm" => "var(--s7)",
        _ => "var(--s8)",
    }
}

pub fn continent_name(code: &str) -> &'static str {
    match code {
        "EU" => "Europe",
        "NA" => "North America",
        "SA" => "South America",
        "AS" => "Asia",
        "AF" => "Africa",
        "OC" => "Oceania",
        "AN" => "Antarctica",
        _ => "Unknown",
    }
}

/// Minutes as "35:14".
pub fn hm(minutes: i64) -> String {
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

pub fn when(t: &NaiveDateTime) -> String {
    t.format("%a %H:%M").to_string()
}

pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

pub fn pct(part: u32, whole: u32) -> String {
    if whole == 0 {
        "–".into()
    } else {
        format!("{:.1}%", part as f64 * 100.0 / whole as f64)
    }
}

#[derive(Properties, PartialEq)]
pub struct TileProps {
    pub label: AttrValue,
    pub value: AttrValue,
    #[prop_or_default]
    pub note: AttrValue,
}

#[function_component(Tile)]
pub fn tile(p: &TileProps) -> Html {
    html! {
        <div class="tile">
            <div class="tile-label">{ &p.label }</div>
            <div class="tile-value">{ &p.value }</div>
            if !p.note.is_empty() {
                <div class="tile-note">{ &p.note }</div>
            }
        </div>
    }
}

/// One row of a horizontal bar list.
#[derive(Clone, PartialEq)]
pub struct BarItem {
    pub label: String,
    pub value: u32,
    pub note: String,
    pub focus: Option<Focus>,
}

#[derive(Properties, PartialEq)]
pub struct BarsProps {
    pub items: Vec<BarItem>,
    pub on_focus: Callback<Focus>,
}

/// Horizontal bars with the label and value as text beside each bar.
#[function_component(Bars)]
pub fn bars(p: &BarsProps) -> Html {
    let max = p.items.iter().map(|i| i.value).max().unwrap_or(1).max(1);
    html! {
        <ul class="bars">
            { for p.items.iter().map(|item| {
                let width = item.value as f64 * 100.0 / max as f64;
                let onclick = item.focus.clone().map(|f| {
                    let cb = p.on_focus.clone();
                    Callback::from(move |_: MouseEvent| cb.emit(f.clone()))
                });
                let title = item.focus.as_ref().map(|_| "Show these QSOs in the log");
                html! {
                    <li class={classes!(onclick.is_some().then_some("clickable"))} {onclick} title={title}>
                        <span class="bar-label">{ &item.label }</span>
                        <span class="bar-track"><span class="bar-fill" style={format!("width:{:.1}%", width)}></span></span>
                        <span class="bar-value">{ item.value }
                            if !item.note.is_empty() { <span class="muted">{ format!(" {}", item.note) }</span> }
                        </span>
                    </li>
                }
            }) }
        </ul>
    }
}

/// A count cell shaded by magnitude; zero renders as a muted dot.
pub fn count_cell(value: u32, max: u32) -> Html {
    if value == 0 {
        html! { <td class="num zero">{"·"}</td> }
    } else {
        let v = value as f64 / max.max(1) as f64;
        html! { <td class="num heat" style={format!("--v:{:.2}", v)}>{ value }</td> }
    }
}

pub fn band_header(band: &str) -> Html {
    html! {
        <th class="num">
            <span class="swatch" style={format!("background:{}", band_color(band))}></span>{ band }
        </th>
    }
}
