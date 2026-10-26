use crate::charts::{HeadingRose, HourChart, ZoneGrid};
use crate::common::*;
use crate::map;
use std::collections::HashMap;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[function_component(Overview)]
pub fn overview(p: &ViewProps) -> Html {
    let d = &p.data;
    let r = &d.report;
    let s = &r.summary;
    let info = &d.log.info;
    let best60 = r.best_rates.iter().find(|w| w.minutes == 60);
    let has_run = s.run_qsos.is_some();

    let details = [
        ("Contest", info.contest.clone()),
        ("Category", info.category.clone()),
        ("Operators", info.operators.clone()),
        ("Location", [info.locator.clone(), info.location.clone()].iter().filter(|v| !v.is_empty()).cloned().collect::<Vec<_>>().join(" · ")),
        ("Club", info.club.clone()),
        ("Claimed score", info.claimed_score.clone()),
        ("Software", info.created_by.clone()),
        ("Log span", match (s.first, s.last) {
            (Some(a), Some(b)) => format!("{} – {} UTC", a.format("%d %b %H:%M"), b.format("%d %b %H:%M")),
            _ => String::new(),
        }),
    ];

    let total = s.qsos;
    let continents: Vec<BarItem> = r.continents.iter().map(|c| BarItem {
        label: continent_name(c.continent).into(),
        value: c.total,
        note: format!("{} · {} DXCC", pct(c.total, total), c.countries),
        focus: Some(Focus::Continent(c.continent.into())),
    }).collect();
    let countries: Vec<BarItem> = r.countries.iter().take(10).map(|c| BarItem {
        label: c.country.into(),
        value: c.total,
        note: pct(c.total, total),
        focus: Some(Focus::Country(c.country.into())),
    }).collect();

    html! {
        <>
            <section class="card station">
                <div class="station-call">{ if info.callsign.is_empty() { "Unknown station".to_string() } else { info.callsign.clone() } }</div>
                <dl class="details">
                    { for details.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| html! {
                        <><dt>{ *k }</dt><dd>{ v }</dd></>
                    }) }
                </dl>
                if !info.soapbox.is_empty() {
                    <p class="soapbox">{ format!("“{}”", info.soapbox) }</p>
                }
            </section>

            if !d.log.skipped.is_empty() {
                <section class="card warning">
                    <h2>{ format!("{} QSO line{} could not be read", d.log.skipped.len(), if d.log.skipped.len() == 1 { "" } else { "s" }) }</h2>
                    <p class="hint">{"These lines are left out of all statistics."}</p>
                    <ul class="skipped">
                        { for d.log.skipped.iter().take(10).map(|l| html! { <li>{ l }</li> }) }
                    </ul>
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
                <p class="hint">{"Click a band to filter every view to it. Band mults = DXCC + zones worked on that band."}</p>
                <div class="table-wrap">
                    <table class="data">
                        <thead><tr>
                            <th>{"Band"}</th><th class="num">{"QSOs"}</th><th class="num">{"Share"}</th>
                            <th class="num">{"Dupes"}</th><th class="num">{"Calls"}</th><th class="num">{"DXCC"}</th>
                            <th class="num">{"Zones"}</th><th class="num">{"Band mults"}</th><th class="num">{"Best hour"}</th>
                            if has_run { <th class="num">{"Run"}</th> }
                        </tr></thead>
                        <tbody>
                            { for r.band_rows.iter().map(|b| {
                                let on_band = d.on_band.clone();
                                let band = b.band.clone();
                                html! {
                                    <tr class="clickable" onclick={Callback::from(move |_| on_band.emit(Some(band.clone())))}>
                                        <td><span class="swatch" style={format!("background:{}", band_color(&b.band))}></span>{ &b.band }</td>
                                        <td class="num">{ b.qsos }</td>
                                        <td class="num">{ pct(b.qsos, total) }</td>
                                        <td class="num">{ b.dupes }</td>
                                        <td class="num">{ b.unique_calls }</td>
                                        <td class="num">{ b.dxcc }</td>
                                        <td class="num">{ b.cq_zones }</td>
                                        <td class="num">{ b.dxcc + b.cq_zones }</td>
                                        <td class="num">{ b.best_hour }</td>
                                        if has_run { <td class="num">{ pct(b.run, b.qsos) }</td> }
                                    </tr>
                                }
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
                </div>
            </section>

            <div class="two-col">
                <section class="card">
                    <h2>{"Continents"}</h2>
                    <Bars items={continents} on_focus={d.on_focus.clone()} />
                </section>
                <section class="card">
                    <h2>{"Top countries"}</h2>
                    <Bars items={countries} on_focus={d.on_focus.clone()} />
                </section>
            </div>
        </>
    }
}

#[function_component(Rates)]
pub fn rates(p: &ViewProps) -> Html {
    let d = &p.data;
    let r = &d.report;
    let s = &r.summary;
    let best_hour = r.hours.iter().max_by_key(|h| h.total);
    let max_cell = r.hours.iter().flat_map(|h| h.per_band.iter().copied()).max().unwrap_or(1);
    let has_run = s.run_qsos.is_some();
    let has_tx = d.log.transmitters().len() > 1;

    html! {
        <>
            <section class="card">
                <h2>{"QSOs per hour"}</h2>
                <HourChart data={d.clone()} />
            </section>

            <section class="tiles">
                { for r.best_rates.iter().map(|w| html! {
                    <Tile label={format!("Best {} min", w.minutes)} value={format!("{} QSOs", w.qsos)}
                        note={format!("{:.0}/h · from {}", w.per_hour(), when(&w.start))} />
                }) }
                { for best_hour.map(|h| html! {
                    <Tile label="Best clock hour" value={format!("{} QSOs", h.total)} note={h.start.format("%a %H:00").to_string()} />
                }) }
                <Tile label="Average rate" value={format!("{:.0}/h", s.avg_rate)} note={format!("over {} on air", hm(s.operating_min))} />
            </section>

            if let Some(run) = s.run_qsos {
                <section class="card">
                    <h2>{"Run vs search & pounce"}</h2>
                    <p class="hint">{ format!(
                        "Estimated: a QSO counts as run when the QSO before or after it, on the same band and transmitter, was on exactly the same frequency. {} run ({}), {} S&P ({}). ",
                        run, pct(run, s.qsos), s.qsos - run, pct(s.qsos - run, s.qsos)) }
                        <button class="link" onclick={d.focus(Focus::Run(true))}>{"Run QSOs →"}</button>{"  "}
                        <button class="link" onclick={d.focus(Focus::Run(false))}>{"S&P QSOs →"}</button>
                    </p>
                    if !r.runs.is_empty() {
                        <h3>{"Longest runs"}</h3>
                        <p class="hint">{ format!("Stretches of {}+ QSOs on one frequency{}.", stats::report::MIN_RUN_QSOS,
                            if r.runs.len() > 20 { format!(", top 20 of {}", r.runs.len()) } else { String::new() }) }</p>
                        <div class="table-wrap">
                            <table class="data compact">
                                <thead><tr>
                                    <th>{"Band"}</th><th class="num">{"kHz"}</th>
                                    if has_tx { <th>{"TX"}</th> }
                                    <th>{"From"}</th><th>{"To"}</th><th class="num">{"Length"}</th>
                                    <th class="num">{"QSOs"}</th><th class="num">{"Rate"}</th>
                                </tr></thead>
                                <tbody>
                                    { for r.runs.iter().take(20).map(|run| html! {
                                        <tr>
                                            <td><span class="swatch" style={format!("background:{}", band_color(&run.band))}></span>{ &run.band }</td>
                                            <td class="num">{ &run.freq }</td>
                                            if has_tx { <td>{ run.tx.clone().unwrap_or_default() }</td> }
                                            <td>{ when(&run.start) }</td><td>{ when(&run.end) }</td>
                                            <td class="num">{ hm(run.minutes()) }</td>
                                            <td class="num strong">{ run.qsos }</td>
                                            <td class="num">{ format!("{:.0}/h", run.per_hour()) }</td>
                                        </tr>
                                    }) }
                                </tbody>
                            </table>
                        </div>
                    }
                </section>
            }

            <section class="card">
                <h2>{ format!("Off-times ({})", r.breaks.len()) }</h2>
                <p class="hint">{ format!("Gaps of {} minutes or more between QSOs{}. Total off-time {} of a {} span.",
                    stats::report::BREAK_MINUTES,
                    if d.report.bands.len() == 1 { " on this band" } else { "" },
                    hm(s.break_min), hm(s.participation_min)) }</p>
                if !r.breaks.is_empty() {
                    <div class="table-wrap">
                        <table class="data compact">
                            <thead><tr><th>{"From"}</th><th>{"To"}</th><th class="num">{"Length"}</th></tr></thead>
                            <tbody>
                                { for r.breaks.iter().map(|b| html! {
                                    <tr>
                                        <td>{ when(&b.start) }</td><td>{ when(&b.end) }</td>
                                        <td class="num">{ hm(b.minutes) }</td>
                                    </tr>
                                }) }
                            </tbody>
                        </table>
                    </div>
                }
            </section>

            <section class="card">
                <h2>{"Hour by hour"}</h2>
                <p class="hint">{"Click an hour to see its QSOs. "}
                    <button class="link" onclick={d.focus(Focus::NewMults)}>{"Show every new-multiplier QSO →"}</button>
                </p>
                <div class="table-wrap">
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
                                    <tr class={classes!("clickable", (h.total == 0).then_some("idle"))} onclick={d.focus(Focus::Hour(h.start))}>
                                        <td class="nowrap">{ h.start.format("%a %H:00").to_string() }</td>
                                        { for h.per_band.iter().map(|c| count_cell(*c, max_cell)) }
                                        <td class="num strong">{ h.total }</td>
                                        <td class="num">{ h.cumulative }</td>
                                        if has_run { <td class="num">{ if h.total > 0 { pct(h.run, h.total) } else { String::new() } }</td> }
                                        <td class="mults" title={all_mults}>{ mults }</td>
                                    </tr>
                                }
                            }) }
                        </tbody>
                    </table>
                </div>
            </section>
        </>
    }
}

#[derive(Clone, Copy, PartialEq)]
enum CountrySort {
    Country,
    Prefix,
    Continent,
    Zone,
    Band(usize),
    Total,
    Calls,
    Distance,
    First,
}

#[function_component(Geography)]
pub fn geography(p: &ViewProps) -> Html {
    let d = &p.data;
    let r = &d.report;
    let sort = use_state(|| (CountrySort::Total, true));
    let search = use_state(String::new);

    {
        let data = d.clone();
        use_effect_with_deps(
            move |_| {
                map::mount("map");
                map::show(&data);
                || ()
            },
            d.clone(),
        );
    }

    let mut countries: Vec<_> = r.countries.iter()
        .filter(|c| {
            let q = search.to_lowercase();
            q.is_empty() || c.country.to_lowercase().contains(&q) || c.prefix.to_lowercase().contains(&q)
        })
        .collect();
    let (key, desc) = *sort;
    countries.sort_by(|a, b| {
        let o = match key {
            CountrySort::Country => a.country.cmp(b.country),
            CountrySort::Prefix => a.prefix.cmp(b.prefix),
            CountrySort::Continent => a.continent.cmp(b.continent),
            CountrySort::Zone => a.cq_zone.cmp(&b.cq_zone),
            CountrySort::Band(i) => a.per_band[i].cmp(&b.per_band[i]),
            CountrySort::Total => a.total.cmp(&b.total),
            CountrySort::Calls => a.unique_calls.cmp(&b.unique_calls),
            CountrySort::Distance => a.distance_km.unwrap_or(0.0).total_cmp(&b.distance_km.unwrap_or(0.0)),
            CountrySort::First => a.first.cmp(&b.first),
        };
        if desc { o.reverse() } else { o }
    });

    let header = |label: Html, k: CountrySort, numeric: bool| {
        let sort = sort.clone();
        let active = sort.0 == k;
        let arrow = if active { if sort.1 { " ↓" } else { " ↑" } } else { "" };
        let onclick = Callback::from(move |_| {
            // numbers sort largest first, text A-Z first
            let desc = if sort.0 == k { !sort.1 } else { numeric };
            sort.set((k, desc));
        });
        html! {
            <th class={classes!("sortable", numeric.then_some("num"), active.then_some("active"))} {onclick}>
                { label }{ arrow }
            </th>
        }
    };
    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| search.set(e.target_unchecked_into::<HtmlInputElement>().value()))
    };
    let max_cell = r.countries.iter().flat_map(|c| c.per_band.iter().copied()).max().unwrap_or(1);
    let max_cont = r.continents.iter().flat_map(|c| c.per_band.iter().copied()).max().unwrap_or(1);
    let missing: Vec<String> = (1..=40u32).filter(|z| !r.zones.iter().any(|w| w.zone == *z)).map(|z| z.to_string()).collect();
    let station_note = match &d.log.station {
        Some(s) if s.from_locator => "Distances and headings from the log's grid locator to each country's centre.",
        Some(_) => "No grid locator in the log: distances and headings use your country's centre, so they are approximate.",
        None => "Station location unknown: no distances or headings.",
    };

    html! {
        <>
            <section class="card">
                <h2>{"Map"}</h2>
                <p class="hint">{"Circle size follows the number of QSOs with each country."}</p>
                <div id="map"></div>
            </section>

            <section class="card">
                <div class="card-head">
                    <h2>{ format!("Countries ({})", r.countries.len()) }</h2>
                    <input class="search" type="search" placeholder="Find country or prefix" value={(*search).clone()} oninput={on_search} />
                </div>
                <p class="hint">{"Sort by any column. Click a country to see its QSOs."}</p>
                <div class="table-wrap tall">
                    <table class="data compact">
                        <thead><tr>
                            { header(html!{"Country"}, CountrySort::Country, false) }
                            { header(html!{"Prefix"}, CountrySort::Prefix, false) }
                            { header(html!{"Cont."}, CountrySort::Continent, false) }
                            { header(html!{"Zone"}, CountrySort::Zone, true) }
                            { for r.bands.iter().enumerate().map(|(i, b)| header(html!{
                                <><span class="swatch" style={format!("background:{}", band_color(b))}></span>{ b }</>
                            }, CountrySort::Band(i), true)) }
                            { header(html!{"Total"}, CountrySort::Total, true) }
                            { header(html!{"Calls"}, CountrySort::Calls, true) }
                            { header(html!{"km"}, CountrySort::Distance, true) }
                            { header(html!{"First QSO"}, CountrySort::First, false) }
                        </tr></thead>
                        <tbody>
                            { for countries.iter().map(|c| html! {
                                <tr class="clickable" onclick={d.focus(Focus::Country(c.country.into()))}>
                                    <td>{ c.country }</td>
                                    <td>{ c.prefix }</td>
                                    <td>{ c.continent }</td>
                                    <td class="num">{ c.cq_zone }</td>
                                    { for c.per_band.iter().map(|v| count_cell(*v, max_cell)) }
                                    <td class="num strong">{ c.total }</td>
                                    <td class="num">{ c.unique_calls }</td>
                                    <td class="num">{ c.distance_km.map(|k| thousands(k as u64)).unwrap_or_default() }</td>
                                    <td class="nowrap">{ when(&c.first) }</td>
                                </tr>
                            }) }
                        </tbody>
                    </table>
                </div>
            </section>

            <section class="card">
                <h2>{"Continents by band"}</h2>
                <div class="table-wrap">
                    <table class="data">
                        <thead><tr>
                            <th>{"Continent"}</th>
                            { for r.bands.iter().map(|b| band_header(b)) }
                            <th class="num">{"Total"}</th><th class="num">{"DXCC"}</th>
                        </tr></thead>
                        <tbody>
                            { for r.continents.iter().map(|c| html! {
                                <tr class="clickable" onclick={d.focus(Focus::Continent(c.continent.into()))}>
                                    <td>{ continent_name(c.continent) }</td>
                                    { for c.per_band.iter().map(|v| count_cell(*v, max_cont)) }
                                    <td class="num strong">{ c.total }</td>
                                    <td class="num">{ c.countries }</td>
                                </tr>
                            }) }
                        </tbody>
                    </table>
                </div>
            </section>

            <div class="two-col">
                <section class="card">
                    <h2>{ format!("CQ zones · {} of 40", r.zones.len()) }</h2>
                    <p class="hint">{ if d.log.zones_from_exchange { "Zones as received in the exchange." } else { "Zones from the country file; stations in large countries may be in other zones." } }</p>
                    <ZoneGrid data={d.clone()} />
                    if !missing.is_empty() {
                        <p class="hint">{ format!("Not worked: {}", missing.join(", ")) }</p>
                    }
                </section>
                <section class="card">
                    <h2>{"Beam headings"}</h2>
                    <HeadingRose data={d.clone()} />
                    <p class="hint">{ station_note }</p>
                </section>
            </div>
        </>
    }
}

#[function_component(Callsigns)]
pub fn callsigns(p: &ViewProps) -> Html {
    let d = &p.data;
    let r = &d.report;
    let s = &r.summary;
    let search = use_state(String::new);
    let multi_band = r.stations.iter().filter(|st| st.bands.len() > 1).count();

    let per_bands: Vec<BarItem> = r.bands_per_station.iter().map(|(n, c)| BarItem {
        label: if *n == 1 { "1 band".into() } else { format!("{} bands", n) },
        value: *c,
        note: pct(*c, s.unique_calls),
        focus: None,
    }).collect();
    let lengths: Vec<BarItem> = r.call_lengths.iter().map(|(n, c)| BarItem {
        label: format!("{} chars", n),
        value: *c,
        note: pct(*c, s.unique_calls),
        focus: None,
    }).collect();
    let prefixes: Vec<BarItem> = r.prefixes.iter().take(15).map(|(prefix, c)| BarItem {
        label: prefix.clone(),
        value: *c,
        note: String::new(),
        focus: Some(Focus::Prefix(prefix.clone())),
    }).collect();

    let q = search.to_uppercase();
    let matching: Vec<_> = r.stations.iter().filter(|st| q.is_empty() || st.call.contains(&q)).collect();
    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| search.set(e.target_unchecked_into::<HtmlInputElement>().value()))
    };

    html! {
        <>
            <section class="tiles">
                <Tile label="Unique calls" value={thousands(s.unique_calls as u64)} note={format!("{:.2} QSOs per station", s.qsos as f64 / s.unique_calls.max(1) as f64)} />
                if r.bands.len() > 1 {
    <Tile label="Worked on 2+ bands" value={thousands(multi_band as u64)} note={pct(multi_band as u32, s.unique_calls)} />
                }
                <Tile label="Prefixes (WPX)" value={thousands(s.prefixes as u64)} />
                <Tile label="Dupes" value={s.dupes.to_string()} note={pct(s.dupes, s.qsos)} />
            </section>

            <div class="two-col">
                if r.bands.len() > 1 {
                <section class="card">
                    <h2>{"Bands per station"}</h2>
                    <p class="hint">{"How many bands each station was worked on."}</p>
                    <Bars items={per_bands} on_focus={d.on_focus.clone()} />
                </section>
                }
                <section class="card">
                    <h2>{"Callsign length"}</h2>
                    <p class="hint">{"Unique calls by length, without portable suffixes."}</p>
                    <Bars items={lengths} on_focus={d.on_focus.clone()} />
                </section>
            </div>

            <div class="two-col">
                <section class="card">
                    <div class="card-head">
                        <h2>{"Stations"}</h2>
                        <input class="search" type="search" placeholder="Find callsign" value={(*search).clone()} oninput={on_search} />
                    </div>
                    <p class="hint">{"Most bands first. Click a call to see its QSOs."}</p>
                    <div class="table-wrap tall">
                        <table class="data compact">
                            <thead><tr><th>{"Call"}</th><th>{"Country"}</th><th class="num">{"QSOs"}</th><th>{"Bands"}</th></tr></thead>
                            <tbody>
                                { for matching.iter().take(200).map(|st| html! {
                                    <tr class="clickable" onclick={d.focus(Focus::Call(st.call.clone()))}>
                                        <td class="call">{ &st.call }</td>
                                        <td>{ st.country }</td>
                                        <td class="num">{ st.qsos }</td>
                                        <td class="nowrap">{ for st.bands.iter().map(|b| html! {
                                            <span class="chip" style={format!("--c:{}", band_color(b))}>{ b }</span>
                                        }) }</td>
                                    </tr>
                                }) }
                            </tbody>
                        </table>
                    </div>
                    if matching.len() > 200 {
                        <p class="hint">{ format!("Showing 200 of {}. Search to narrow down.", matching.len()) }</p>
                    }
                </section>
                <section class="card">
                    <h2>{"Top prefixes"}</h2>
                    <Bars items={prefixes} on_focus={d.on_focus.clone()} />
                </section>
            </div>

            <div class="two-col">
                <section class="card">
                    <h2>{ format!("Dupes ({})", s.dupes) }</h2>
                    <p class="hint">{"Same call worked again on the same band and mode."}</p>
                    if s.dupes > 0 {
                        <button class="link" onclick={d.focus(Focus::Dupes)}>{"Show dupes in log →"}</button>
                    }
                </section>
                <section class="card">
                    <h2>{ format!("Unknown country ({})", r.unknown_calls.len()) }</h2>
                    <p class="hint">{"Calls that no prefix in the country file matches. Often busted calls."}</p>
                    if !r.unknown_calls.is_empty() {
                        <p class="call-list">{ r.unknown_calls.join(" ") }</p>
                        <button class="link" onclick={d.focus(Focus::Unknown)}>{"Show in log →"}</button>
                    }
                </section>
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct LogProps {
    pub data: Data,
    pub focus: Option<Focus>,
    pub on_clear: Callback<()>,
}

const PAGE: usize = 250;

#[function_component(LogView)]
pub fn log_view(p: &LogProps) -> Html {
    let d = &p.data;
    let r = &d.report;
    let search = use_state(String::new);
    let limit = use_state(|| PAGE);
    {
        let limit = limit.clone();
        use_effect_with_deps(move |_| { limit.set(PAGE); || () }, (p.focus.clone(), (*search).clone(), d.clone()));
    }

    let q = search.to_uppercase();
    let rows: Vec<_> = d.qsos()
        .filter(|(i, qso)| p.focus.as_ref().is_none_or(|f| f.matches(*i, qso, r)))
        .filter(|(_, qso)| q.is_empty() || qso.call.contains(&q) || qso.country().to_uppercase().contains(&q))
        .collect();
    let has_tx = d.qsos().any(|(_, q)| q.tx.is_some());

    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| search.set(e.target_unchecked_into::<HtmlInputElement>().value()))
    };
    let on_more = {
        let limit = limit.clone();
        Callback::from(move |_| limit.set(*limit + PAGE * 4))
    };
    let on_clear = p.on_clear.clone();
    let mut per_call: HashMap<&str, u32> = HashMap::new();
    for (_, qso) in &rows {
        *per_call.entry(qso.call.as_str()).or_default() += 1;
    }

    html! {
        <section class="card">
            <div class="card-head">
                <h2>{ format!("Log · {} QSOs", thousands(rows.len() as u64)) }</h2>
                <input class="search" type="search" placeholder="Find callsign or country" value={(*search).clone()} oninput={on_search} />
            </div>
            <div class="focus-bar">
                if let Some(f) = &p.focus {
                    <span class="focus-chip">{ f.label() }
                        <button aria-label="Clear" onclick={Callback::from(move |_| on_clear.emit(()))}>{"×"}</button>
                    </span>
                } else {
                    <span class="hint">{"Tip: click any country, zone, hour or call in other views to narrow the log."}</span>
                }
            </div>
            <div class="table-wrap tall">
                <table class="data compact log">
                    <thead><tr>
                        <th class="num">{"#"}</th><th>{"UTC"}</th><th>{"Band"}</th><th class="num">{"kHz"}</th><th>{"Mode"}</th>
                        <th>{"Call"}</th><th>{"Sent"}</th><th>{"Rcvd"}</th><th>{"Country"}</th><th class="num">{"Zone"}</th>
                        if has_tx { <th>{"TX"}</th> }
                        if d.log.has_run_estimate { <th>{"Run"}</th> }
                        <th></th>
                    </tr></thead>
                    <tbody>
                        { for rows.iter().take(*limit).map(|(i, qso)| {
                            let new_dxcc = r.new_dxcc.contains(i);
                            let new_zone = r.new_zone.contains(i);
                            html! {
                                <tr class={classes!(qso.dupe.then_some("dupe"))}>
                                    <td class="num muted">{ qso.number }</td>
                                    <td class="nowrap">{ qso.time.format("%a %H:%M").to_string() }</td>
                                    <td><span class="swatch" style={format!("background:{}", band_color(&qso.band))}></span>{ &qso.band }</td>
                                    <td class="num">{ &qso.freq }</td>
                                    <td>{ &qso.mode }</td>
                                    <td class="call">
                                        <button class="link" onclick={d.focus(Focus::Call(qso.call.clone()))}
                                            title={format!("{} QSOs with {} in this view", per_call.get(qso.call.as_str()).unwrap_or(&0), qso.call)}>
                                            { &qso.call }
                                        </button>
                                    </td>
                                    <td class="nowrap">{ &qso.sent_exch }</td>
                                    <td class="nowrap">{ &qso.rcvd_exch }</td>
                                    <td>{ qso.country() }</td>
                                    <td class="num">{ if qso.cq_zone() > 0 { qso.cq_zone().to_string() } else { String::new() } }</td>
                                    if has_tx { <td>{ qso.tx.clone().unwrap_or_default() }</td> }
                                    if d.log.has_run_estimate { <td class="muted">{ match qso.run { Some(true) => "run", Some(false) => "S&P", None => "" } }</td> }
                                    <td class="nowrap">
                                        if qso.dupe { <span class="badge dupe-badge">{"dupe"}</span> }
                                        if new_dxcc { <span class="badge">{"new DXCC"}</span> }
                                        if new_zone { <span class="badge">{"new zone"}</span> }
                                    </td>
                                </tr>
                            }
                        }) }
                    </tbody>
                </table>
            </div>
            if rows.len() > *limit {
                <button class="more" onclick={on_more}>{ format!("Show more ({} of {})", *limit, rows.len()) }</button>
            }
        </section>
    }
}
