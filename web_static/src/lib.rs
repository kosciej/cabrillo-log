mod charts;
mod common;
mod map;
mod views;

use common::{Data, Focus};
use stats::report::{ContestLog, Report, ReportFilter};
use std::rc::Rc;
use views::{Callsigns, Geography, LogView, Overview, Rates};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{DragEvent, File, HtmlInputElement};
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Overview,
    Rates,
    Geography,
    Callsigns,
    Log,
}

const TABS: [(Tab, &str, &str); 5] = [
    (Tab::Overview, "overview", "Overview"),
    (Tab::Rates, "rates", "Rates & time"),
    (Tab::Geography, "geography", "Geography"),
    (Tab::Callsigns, "callsigns", "Callsigns"),
    (Tab::Log, "log", "Log"),
];

impl Tab {
    fn from_hash(hash: &str) -> Tab {
        let hash = hash.trim_start_matches('#');
        TABS.iter()
            .find(|(_, h, _)| *h == hash)
            .map(|(t, _, _)| *t)
            .unwrap_or(Tab::Overview)
    }

    fn hash(self) -> &'static str {
        TABS.iter().find(|(t, _, _)| *t == self).unwrap().1
    }
}

fn current_hash() -> String {
    web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default()
}

fn set_hash(tab: Tab) {
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash(tab.hash());
    }
}

/// Loaded log, compared by pointer so it can serve as a memo dependency.
#[derive(Clone)]
struct Loaded(Rc<ContestLog>);

impl PartialEq for Loaded {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

async fn read_log(file: File) -> Result<ContestLog, String> {
    let text = JsFuture::from(file.text())
        .await
        .map_err(|e| format!("Could not read the file: {:?}", e))?
        .as_string()
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err("The file is empty.".into());
    }
    let log = cabrillo_log::CabrilloLog::parse(&text)
        .map_err(|e| format!("This doesn't look like a valid Cabrillo log: {}", e))?;
    if log.qsos.is_empty() {
        return Err("The log contains no QSO lines.".into());
    }
    Ok(ContestLog::from_cabrillo(&log))
}

#[function_component(App)]
fn app() -> Html {
    let loaded = use_state(|| None::<Loaded>);
    let filter = use_state(ReportFilter::default);
    let tab = use_state(|| Tab::from_hash(&current_hash()));
    let focus = use_state(|| None::<Focus>);
    let loading = use_state(|| false);
    let error = use_state(String::new);
    let dragging = use_state(|| false);
    let file_name = use_state(String::new);

    // Follow the URL hash, so back/forward and shared links pick the tab.
    {
        let tab = tab.clone();
        use_effect_with_deps(
            move |_| {
                let listener = Closure::<dyn Fn()>::new(move || tab.set(Tab::from_hash(&current_hash())));
                let window = web_sys::window().unwrap();
                let _ = window.add_event_listener_with_callback("hashchange", listener.as_ref().unchecked_ref());
                move || {
                    let _ = window.remove_event_listener_with_callback("hashchange", listener.as_ref().unchecked_ref());
                }
            },
            (),
        );
    }

    let report: Rc<Report> = use_memo(
        |(loaded, filter)| match loaded {
            Some(l) => l.0.report(filter),
            None => Report::default(),
        },
        ((*loaded).clone(), (*filter).clone()),
    );

    let open_file = {
        let (loaded, filter, focus, loading, error, file_name) = (
            loaded.clone(), filter.clone(), focus.clone(), loading.clone(), error.clone(), file_name.clone(),
        );
        Callback::from(move |file: File| {
            let (loaded, filter, focus, loading, error, file_name) = (
                loaded.clone(), filter.clone(), focus.clone(), loading.clone(), error.clone(), file_name.clone(),
            );
            file_name.set(file.name());
            loading.set(true);
            error.set(String::new());
            wasm_bindgen_futures::spawn_local(async move {
                match read_log(file).await {
                    Ok(log) => {
                        filter.set(ReportFilter::default());
                        focus.set(None);
                        loaded.set(Some(Loaded(Rc::new(log))));
                    }
                    Err(e) => error.set(e),
                }
                loading.set(false);
            });
        })
    };

    let on_input = {
        let open_file = open_file.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(file) = input.files().and_then(|f| f.get(0)) {
                open_file.emit(file);
            }
            input.set_value("");
        })
    };
    let on_drop = {
        let (open_file, dragging) = (open_file.clone(), dragging.clone());
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            dragging.set(false);
            if let Some(file) = e.data_transfer().and_then(|d| d.files()).and_then(|f| f.get(0)) {
                open_file.emit(file);
            }
        })
    };
    let on_drag_over = {
        let dragging = dragging.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            if !*dragging {
                dragging.set(true);
            }
        })
    };
    let on_drag_leave = {
        let dragging = dragging.clone();
        Callback::from(move |_: DragEvent| dragging.set(false))
    };

    let file_button = html! {
        <label class="file-button">
            { if loaded.is_some() { "Open another log" } else { "Choose a log file" } }
            <input type="file" accept=".log,.txt,.cbr,.cabrillo" onchange={on_input} />
        </label>
    };

    let body = match &*loaded {
        None => html! {
            <div class={classes!("dropzone", dragging.then_some("dragging"))}
                ondrop={on_drop.clone()} ondragover={on_drag_over.clone()} ondragleave={on_drag_leave.clone()}>
                <h1>{"Contest log analyzer"}</h1>
                <p>{"Drop a Cabrillo log here to see rates, off-times, multipliers, countries, zones, beam headings and callsign statistics."}</p>
                { file_button.clone() }
                <p class="hint">{"The log is processed entirely in your browser. Nothing is uploaded."}</p>
            </div>
        },
        Some(l) => {
            let log = l.0.clone();
            let on_focus = {
                let (focus, tab) = (focus.clone(), tab.clone());
                Callback::from(move |f: Focus| {
                    focus.set(Some(f));
                    tab.set(Tab::Log);
                    set_hash(Tab::Log);
                    if let Some(w) = web_sys::window() {
                        w.scroll_to_with_x_and_y(0.0, 0.0);
                    }
                })
            };
            let on_band = {
                let filter = filter.clone();
                Callback::from(move |band: Option<String>| {
                    filter.set(ReportFilter { band, ..(*filter).clone() });
                })
            };
            let data = Data { log: log.clone(), report: report.clone(), on_focus, on_band: on_band.clone() };
            let on_clear = {
                let focus = focus.clone();
                Callback::from(move |_| focus.set(None))
            };

            let view = match *tab {
                Tab::Overview => html! { <Overview data={data} /> },
                Tab::Rates => html! { <Rates data={data} /> },
                Tab::Geography => html! { <Geography data={data} /> },
                Tab::Callsigns => html! { <Callsigns data={data} /> },
                Tab::Log => html! { <LogView data={data} focus={(*focus).clone()} on_clear={on_clear} /> },
            };

            html! {
                <div ondrop={on_drop.clone()} ondragover={on_drag_over.clone()}>
                    <nav class="nav">
                        <div class="nav-inner">
                            <div class="tabs" role="tablist">
                                { for TABS.iter().map(|(t, h, label)| {
                                    let tab_state = tab.clone();
                                    let t = *t;
                                    let onclick = Callback::from(move |e: MouseEvent| {
                                        e.prevent_default();
                                        tab_state.set(t);
                                        set_hash(t);
                                    });
                                    html! {
                                        <a href={format!("#{}", h)} role="tab" aria-selected={(*tab == t).to_string()}
                                            class={classes!("tab", (*tab == t).then_some("active"))} {onclick}>{ *label }</a>
                                    }
                                }) }
                            </div>
                            <FilterBar log={Loaded(log)} filter={(*filter).clone()} on_change={{
                                let filter = filter.clone();
                                Callback::from(move |f| filter.set(f))
                            }} />
                        </div>
                    </nav>
                    <main class="content">
                        if report.qsos.is_empty() {
                            <section class="card"><p>{"No QSOs match this filter."}</p></section>
                        } else {
                            { view }
                        }
                    </main>
                </div>
            }
        }
    };

    html! {
        <>
            <header class="topbar">
                <div class="brand">
                    <span class="brand-name">{"Cabrillo log"}</span>
                    if let Some(l) = &*loaded {
                        <span class="brand-log">{ format!("{} · {}", if l.0.info.callsign.is_empty() { "?" } else { &l.0.info.callsign }, if l.0.info.contest.is_empty() { (*file_name).clone() } else { l.0.info.contest.clone() }) }</span>
                    }
                </div>
                if loaded.is_some() { { file_button } }
            </header>
            if !error.is_empty() {
                <div class="error">{ &*error }</div>
            }
            if *loading {
                <div class="loading">{ format!("Analyzing {}…", *file_name) }</div>
            }
            { body }
        </>
    }
}

#[derive(Properties, PartialEq)]
struct FilterProps {
    log: Loaded,
    filter: ReportFilter,
    on_change: Callback<ReportFilter>,
}

/// Band, mode and transmitter filters that apply to every view.
#[function_component(FilterBar)]
fn filter_bar(p: &FilterProps) -> Html {
    let log = &p.log.0;
    let group = |label: &str, values: Vec<(String, usize)>, current: &Option<String>, set: Rc<dyn Fn(Option<String>) -> ReportFilter>| {
        if values.len() < 2 {
            return html! {};
        }
        let total: usize = values.iter().map(|(_, n)| n).sum();
        let button = |value: Option<String>, text: String, count: usize| {
            let active = *current == value;
            let on_change = p.on_change.clone();
            let set = set.clone();
            html! {
                <button class={classes!("seg", active.then_some("active"))} aria-pressed={active.to_string()}
                    onclick={Callback::from(move |_| on_change.emit(set(value.clone())))}>
                    { text }<span class="seg-count">{ count }</span>
                </button>
            }
        };
        html! {
            <div class="filter-group" role="group" aria-label={label.to_string()}>
                <span class="filter-label">{ label }</span>
                { button(None, "All".into(), total) }
                { for values.into_iter().map(|(v, n)| button(Some(v.clone()), v, n)) }
            </div>
        }
    };
    let count = |pred: &dyn Fn(&stats::report::LogQso) -> bool| log.qsos.iter().filter(|q| pred(q)).count();

    let bands = log.bands().into_iter().map(|b| { let n = count(&|q| q.band == b); (b, n) }).collect();
    let modes = log.modes().into_iter().map(|m| { let n = count(&|q| q.mode == m); (m, n) }).collect();
    let txs = log.transmitters().into_iter().map(|t| { let n = count(&|q| q.tx.as_ref() == Some(&t)); (t, n) }).collect();
    let f = p.filter.clone();
    let (f1, f2, f3) = (f.clone(), f.clone(), f.clone());

    html! {
        <div class="filters">
            { group("Band", bands, &f.band, Rc::new(move |band| ReportFilter { band, ..f1.clone() })) }
            { group("Mode", modes, &f.mode, Rc::new(move |mode| ReportFilter { mode, ..f2.clone() })) }
            { group("TX", txs, &f.tx, Rc::new(move |tx| ReportFilter { tx, ..f3.clone() })) }
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    yew::Renderer::<App>::new().render();
}
