//! Static, printable version of the report, rendered to an HTML string for
//! the standalone HTML and PDF exports.

use crate::charts::{HeadingRose, HourChart, ZoneGrid};
use crate::common::*;
use stats::report::{BREAK_MINUTES, MIN_RUN_QSOS, ReportFilter};
use yew::prelude::*;

/// Stations listed in the "Stations" table of the document.
const TOP_STATIONS: usize = 50;
const TOP_RUNS: usize = 20;
const TOP_PREFIXES: usize = 20;
const TOP_DUPES: usize = 60;

#[derive(Properties, PartialEq)]
pub struct DocProps {
    pub data: Data,
    pub filter: ReportFilter,
    pub generated: String,
}

/// Human-readable description of the active filter, e.g. "20m · CW".
pub fn filter_label(filter: &ReportFilter) -> String {
    let parts: Vec<String> = [
        filter.band.clone(),
        filter.mode.clone(),
        filter.tx.as_ref().map(|t| format!("TX {}", t)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        "All QSOs".into()
    } else {
        parts.join(" · ")
    }
}

#[function_component(ReportDocument)]
pub fn report_document(p: &DocProps) -> Html {
    let d = &p.data;
    let r = &d.report;
    let s = &r.summary;
    let info = &d.log.info;
    let total = s.qsos;
    let has_run = s.run_qsos.is_some();
    let has_tx = d.log.transmitters().len() > 1;
    let best60 = r.best_rates.iter().find(|w| w.minutes == 60);
    let max_hour_cell = r.hours.iter().flat_map(|h| h.per_band.iter().copied()).max().unwrap_or(1);
    let max_country_cell = r.countries.iter().flat_map(|c| c.per_band.iter().copied()).max().unwrap_or(1);
    let max_cont_cell = r.continents.iter().flat_map(|c| c.per_band.iter().copied()).max().unwrap_or(1);
    let missing: Vec<String> = (1..=40u32)
        .filter(|z| !r.zones.iter().any(|w| w.zone == *z))
        .map(|z| z.to_string())
        .collect();
    let bars = |items: Vec<BarItem>| html! { <Bars items={items} on_focus={Callback::noop()} /> };
    let details = [
        ("Contest", info.contest.clone()),
        ("Category", info.category.clone()),
        ("Operators", info.operators.clone()),
        ("Location", [info.locator.clone(), info.location.clone()].iter().filter(|v| !v.is_empty()).cloned().collect::<Vec<_>>().join(" · ")),
        ("Club", info.club.clone()),
        ("Claimed score", info.claimed_score.clone()),
        ("Software", info.created_by.clone()),
        ("Log span", match (s.first, s.last) {
            (Some(a), Some(b)) => format!("{} – {} UTC", a.format("%d %b %Y %H:%M"), b.format("%d %b %Y %H:%M")),
            _ => String::new(),
        }),
        ("Report covers", filter_label(&p.filter)),
    ];

    html! {
        <div class="report-doc">
            <header class="doc-head">
                <div class="station-call">{ if info.callsign.is_empty() { "Unknown station" } else { &info.callsign } }</div>
                <dl class="details">
                    { for details.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| html! { <><dt>{ *k }</dt><dd>{ v }</dd></> }) }
                </dl>
                if !info.soapbox.is_empty() {
                    <p class="soapbox">{ format!("“{}”", info.soapbox) }</p>
                }
            </header>

            if !d.log.skipped.is_empty() {
                <section class="card warning">
                    <h2>{ format!("{} QSO lines could not be read and are not included", d.log.skipped.len()) }</h2>
                    <ul class="skipped">{ for d.log.skipped.iter().take(20).map(|l| html! { <li>{ l }</li> }) }</ul>
                </section>
            }

            <section class="tiles">
                <Tile label="QSOs" value={thousands(s.qsos as u64)} note={format!("{} dupes ({})", s.dupes, pct(s.dupes, s.qsos))} />
                <Tile label="Unique calls" value={thousands(s.unique_calls as u64)} note={format!("{:.2} QSOs per station", s.qsos as f64 / s.unique_calls.max(1) as f64)} />
                <Tile label="DXCC · CQ zones" value={format!("{} · {}", s.dxcc, s.cq_zones)} note={format!("{} band mults · {} continents", s.band_mults, s.continents)} />
                <Tile label="Operating time" value={hm(s.operating_min)} note={format!("{} off · {} span", hm(s.break_min), hm(s.participation_min))} />
                <Tile label="Average rate" value={format!("{:.0}/h", s.avg_rate)} note="per operating hour" />
                <Tile label="Best 60 min" value={best60.map(|w| w.qsos.to_string()).unwrap_or("–".into())}
                    note={best60.map(|w| format!("from {}", when(&w.start))).unwrap_or_default()} />
                <Tile label="Avg distance" value={s.avg_km.map(|k| format!("{} km", thousands(k as u64))).unwrap_or("–".into())}
                    note={s.longest.as_ref().map(|(c, k)| format!("longest {} · {} km", c, thousands(*k as u64))).unwrap_or_default()} />
                <Tile label="Prefixes" value={thousands(s.prefixes as u64)} note={format!("{} unknown calls", s.unknown_calls)} />
                if let Some(run) = s.run_qsos {
                    <Tile label="Run · S&P" value={format!("{} · {}", pct(run, s.qsos), pct(s.qsos - run, s.qsos))} note="estimated from frequencies" />
                }
            </section>

            <section class="card">
                <h2>{"Bands"}</h2>
                <table class="data">
                    <thead><tr>
                        <th>{"Band"}</th><th class="num">{"QSOs"}</th><th class="num">{"Share"}</th>
                        <th class="num">{"Dupes"}</th><th class="num">{"Calls"}</th><th class="num">{"DXCC"}</th>
                        <th class="num">{"Zones"}</th><th class="num">{"Band mults"}</th><th class="num">{"Best hour"}</th>
                        if has_run { <th class="num">{"Run"}</th> }
                    </tr></thead>
                    <tbody>
                        { for r.band_rows.iter().map(|b| html! {
                            <tr>
                                <td><span class="swatch" style={format!("background:{}", band_color(&b.band))}></span>{ &b.band }</td>
                                <td class="num">{ b.qsos }</td><td class="num">{ pct(b.qsos, total) }</td>
                                <td class="num">{ b.dupes }</td><td class="num">{ b.unique_calls }</td>
                                <td class="num">{ b.dxcc }</td><td class="num">{ b.cq_zones }</td>
                                <td class="num">{ b.dxcc + b.cq_zones }</td><td class="num">{ b.best_hour }</td>
                                if has_run { <td class="num">{ pct(b.run, b.qsos) }</td> }
                            </tr>
                        }) }
                    </tbody>
                    if r.band_rows.len() > 1 {
                        <tfoot><tr>
                            <td>{"Total"}</td><td class="num">{ s.qsos }</td><td class="num">{"100%"}</td>
                            <td class="num">{ s.dupes }</td><td class="num">{ s.unique_calls }</td><td class="num">{ s.dxcc }</td>
                            <td class="num">{ s.cq_zones }</td><td class="num">{ s.band_mults }</td>
                            <td class="num">{ r.hours.iter().map(|h| h.total).max().unwrap_or(0) }</td>
                            if has_run { <td class="num">{ pct(s.run_qsos.unwrap_or(0), s.qsos) }</td> }
                        </tr></tfoot>
                    }
                </table>
            </section>

            <h1 class="doc-section">{"Rates & time"}</h1>

            <section class="card">
                <h2>{"QSOs per hour"}</h2>
                <HourChart data={d.clone()} />
            </section>

            <section class="tiles">
                { for r.best_rates.iter().map(|w| html! {
                    <Tile label={format!("Best {} min", w.minutes)} value={format!("{} QSOs", w.qsos)}
                        note={format!("{:.0}/h · from {}", w.per_hour(), when(&w.start))} />
                }) }
                <Tile label="Average rate" value={format!("{:.0}/h", s.avg_rate)} note={format!("over {} on air", hm(s.operating_min))} />
            </section>

            <section class="card">
                <h2>{"Hour by hour"}</h2>
                <table class="data compact">
                    <thead><tr>
                        <th>{"Hour (UTC)"}</th>
                        { for r.bands.iter().map(|b| band_header(b)) }
                        <th class="num">{"Total"}</th><th class="num">{"Running"}</th>
                        if has_run { <th class="num">{"Run"}</th> }
                        <th>{"New multipliers"}</th>
                    </tr></thead>
                    <tbody>
                        { for r.hours.iter().map(|h| {
                            let (mults, all_mults) = new_mults_text(h);
                            html! {
                                <tr class={classes!((h.total == 0).then_some("idle"))}>
                                    <td class="nowrap">{ h.start.format("%a %H:00").to_string() }</td>
                                    { for h.per_band.iter().map(|c| count_cell(*c, max_hour_cell)) }
                                    <td class="num strong">{ h.total }</td>
                                    <td class="num">{ h.cumulative }</td>
                                    if has_run { <td class="num">{ if h.total > 0 { pct(h.run, h.total) } else { String::new() } }</td> }
                                    <td class="mults" title={all_mults}>{ mults }</td>
                                </tr>
                            }
                        }) }
                    </tbody>
                </table>
            </section>

            <div class="two-col">
                <section class="card">
                    <h2>{ format!("Off-times ({})", r.breaks.len()) }</h2>
                    <p class="hint">{ format!("Gaps of {} minutes or more. Total {} of a {} span.", BREAK_MINUTES, hm(s.break_min), hm(s.participation_min)) }</p>
                    if !r.breaks.is_empty() {
                        <table class="data compact">
                            <thead><tr><th>{"From"}</th><th>{"To"}</th><th class="num">{"Length"}</th></tr></thead>
                            <tbody>
                                { for r.breaks.iter().map(|b| html! {
                                    <tr><td>{ when(&b.start) }</td><td>{ when(&b.end) }</td><td class="num">{ hm(b.minutes) }</td></tr>
                                }) }
                            </tbody>
                        </table>
                    }
                </section>
                if let Some(run) = s.run_qsos {
                    <section class="card">
                        <h2>{"Run vs search & pounce"}</h2>
                        <p class="hint">{ format!(
                            "Estimated from frequencies: {} run ({}), {} S&P ({}). Longest stretches of {}+ QSOs on one frequency:",
                            run, pct(run, s.qsos), s.qsos - run, pct(s.qsos - run, s.qsos), MIN_RUN_QSOS) }</p>
                        <table class="data compact">
                            <thead><tr>
                                <th>{"Band"}</th><th class="num">{"kHz"}</th>
                                if has_tx { <th>{"TX"}</th> }
                                <th>{"From"}</th><th class="num">{"Length"}</th><th class="num">{"QSOs"}</th><th class="num">{"Rate"}</th>
                            </tr></thead>
                            <tbody>
                                { for r.runs.iter().take(TOP_RUNS).map(|run| html! {
                                    <tr>
                                        <td><span class="swatch" style={format!("background:{}", band_color(&run.band))}></span>{ &run.band }</td>
                                        <td class="num">{ &run.freq }</td>
                                        if has_tx { <td>{ run.tx.clone().unwrap_or_default() }</td> }
                                        <td>{ when(&run.start) }</td>
                                        <td class="num">{ hm(run.minutes()) }</td>
                                        <td class="num strong">{ run.qsos }</td>
                                        <td class="num">{ format!("{:.0}/h", run.per_hour()) }</td>
                                    </tr>
                                }) }
                            </tbody>
                        </table>
                    </section>
                }
            </div>

            <h1 class="doc-section">{"Geography"}</h1>

            <section class="card">
                <h2>{"Continents by band"}</h2>
                <table class="data">
                    <thead><tr>
                        <th>{"Continent"}</th>
                        { for r.bands.iter().map(|b| band_header(b)) }
                        <th class="num">{"Total"}</th><th class="num">{"Share"}</th><th class="num">{"DXCC"}</th>
                    </tr></thead>
                    <tbody>
                        { for r.continents.iter().map(|c| html! {
                            <tr>
                                <td>{ continent_name(c.continent) }</td>
                                { for c.per_band.iter().map(|v| count_cell(*v, max_cont_cell)) }
                                <td class="num strong">{ c.total }</td>
                                <td class="num">{ pct(c.total, total) }</td>
                                <td class="num">{ c.countries }</td>
                            </tr>
                        }) }
                    </tbody>
                </table>
            </section>

            <div class="two-col">
                <section class="card">
                    <h2>{ format!("CQ zones · {} of 40", r.zones.len()) }</h2>
                    <p class="hint">{ if d.log.zones_from_exchange { "Zones as received in the exchange." } else { "Zones from the country file." } }</p>
                    <ZoneGrid data={d.clone()} />
                    if !missing.is_empty() {
                        <p class="hint">{ format!("Not worked: {}", missing.join(", ")) }</p>
                    }
                </section>
                <section class="card">
                    <h2>{"Beam headings"}</h2>
                    <HeadingRose data={d.clone()} />
                </section>
            </div>

            <section class="card">
                <h2>{ format!("Countries ({})", r.countries.len()) }</h2>
                <table class="data compact">
                    <thead><tr>
                        <th>{"Country"}</th><th>{"Prefix"}</th><th>{"Cont."}</th><th class="num">{"Zone"}</th>
                        { for r.bands.iter().map(|b| band_header(b)) }
                        <th class="num">{"Total"}</th><th class="num">{"Calls"}</th><th class="num">{"km"}</th><th>{"First QSO"}</th>
                    </tr></thead>
                    <tbody>
                        { for r.countries.iter().map(|c| html! {
                            <tr>
                                <td>{ c.country }</td><td>{ c.prefix }</td><td>{ c.continent }</td>
                                <td class="num">{ c.cq_zone }</td>
                                { for c.per_band.iter().map(|v| count_cell(*v, max_country_cell)) }
                                <td class="num strong">{ c.total }</td>
                                <td class="num">{ c.unique_calls }</td>
                                <td class="num">{ c.distance_km.map(|k| thousands(k as u64)).unwrap_or_default() }</td>
                                <td class="nowrap">{ when(&c.first) }</td>
                            </tr>
                        }) }
                    </tbody>
                </table>
            </section>

            <h1 class="doc-section">{"Callsigns"}</h1>

            <div class="two-col">
                if r.bands.len() > 1 {
                    <section class="card">
                        <h2>{"Bands per station"}</h2>
                        { bars(r.bands_per_station.iter().map(|(n, c)| BarItem {
                            label: if *n == 1 { "1 band".into() } else { format!("{} bands", n) },
                            value: *c, note: pct(*c, s.unique_calls), focus: None,
                        }).collect()) }
                    </section>
                }
                <section class="card">
                    <h2>{"Callsign length"}</h2>
                    { bars(r.call_lengths.iter().map(|(n, c)| BarItem {
                        label: format!("{} chars", n), value: *c, note: pct(*c, s.unique_calls), focus: None,
                    }).collect()) }
                </section>
            </div>

            <section class="card">
                <h2>{ format!("Stations (top {} by bands and QSOs)", TOP_STATIONS.min(r.stations.len())) }</h2>
                <table class="data compact">
                    <thead><tr><th>{"Call"}</th><th>{"Country"}</th><th class="num">{"QSOs"}</th><th>{"Bands"}</th></tr></thead>
                    <tbody>
                        { for r.stations.iter().take(TOP_STATIONS).map(|st| html! {
                            <tr>
                                <td class="call">{ &st.call }</td><td>{ st.country }</td><td class="num">{ st.qsos }</td>
                                <td class="nowrap">{ for st.bands.iter().map(|b| html! { <span class="chip" style={format!("--c:{}", band_color(b))}>{ b }</span> }) }</td>
                            </tr>
                        }) }
                    </tbody>
                </table>
            </section>

            <div class="two-col">
                <section class="card">
                    <h2>{"Top prefixes"}</h2>
                    { bars(r.prefixes.iter().take(TOP_PREFIXES).map(|(prefix, c)| BarItem {
                        label: prefix.clone(), value: *c, note: String::new(), focus: None,
                    }).collect()) }
                </section>
                <section class="card">
                    <h2>{ format!("Unknown country ({})", r.unknown_calls.len()) }</h2>
                    <p class="hint">{"Calls no prefix in the country file matches. Often busted calls."}</p>
                    if !r.unknown_calls.is_empty() {
                        <p class="call-list">{ r.unknown_calls.join(" ") }</p>
                    }
                </section>
            </div>

            <section class="card">
                <h2>{ format!("Dupes ({})", s.dupes) }</h2>
                if s.dupes > 0 {
                    <ul class="doc-dupes">
                        { for d.qsos().filter(|(_, q)| q.dupe).take(TOP_DUPES).map(|(_, q)| html! {
                            <li><span class="call">{ &q.call }</span>{ format!(" {} {}", q.band, q.time.format("%a %H:%M")) }</li>
                        }) }
                    </ul>
                    if s.dupes as usize > TOP_DUPES {
                        <p class="hint">{ format!("…and {} more. The Log tab in the app lists them all.", s.dupes as usize - TOP_DUPES) }</p>
                    }
                }
            </section>

            <footer class="doc-foot">
                { format!("Generated {} with Cabrillo log analyzer · ", p.generated) }
                <a href="https://kosciej.github.io/cabrillo-log/">{"kosciej.github.io/cabrillo-log"}</a>
            </footer>
        </div>
    }
}
