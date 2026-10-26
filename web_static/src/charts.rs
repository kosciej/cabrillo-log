use crate::common::{Data, Focus, ViewProps, band_color};
use yew::prelude::*;

const W: f64 = 1000.0;
const H: f64 = 260.0;
const LEFT: f64 = 40.0;
const RIGHT: f64 = 8.0;
const TOP: f64 = 10.0;
const BOTTOM: f64 = 30.0;

/// Round `max` up to a tick step of 1, 2 or 5 x 10^k giving about four ticks.
fn tick_step(max: u32) -> u32 {
    let raw = (max.max(1) as f64 / 4.0).max(1.0);
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    step as u32
}

/// QSOs per clock hour, stacked by band. Hover for details, click to open the hour in the log.
#[function_component(HourChart)]
pub fn hour_chart(p: &ViewProps) -> Html {
    let hover = use_state(|| None::<usize>);
    let report = &p.data.report;
    let hours = &report.hours;
    if hours.is_empty() {
        return html! {};
    }

    let max = hours.iter().map(|h| h.total).max().unwrap_or(1).max(1);
    let step = tick_step(max);
    let top = max.div_ceil(step) * step;
    let plot_w = W - LEFT - RIGHT;
    let plot_h = H - TOP - BOTTOM;
    let slot = plot_w / hours.len() as f64;
    let gap = (slot * 0.18).clamp(1.0, 6.0);
    let y = |v: f64| TOP + plot_h - v / top as f64 * plot_h;
    let label_every = hours.len().div_ceil(16).max(1);

    let ticks = (0..=top / step).map(|i| {
        let v = i * step;
        html! {
            <g>
                <line class="grid" x1={LEFT.to_string()} x2={(W - RIGHT).to_string()}
                    y1={y(v as f64).to_string()} y2={y(v as f64).to_string()} />
                <text class="axis" x={(LEFT - 6.0).to_string()} y={(y(v as f64) + 4.0).to_string()} text-anchor="end">{ v }</text>
            </g>
        }
    });

    let bars = hours.iter().enumerate().map(|(i, h)| {
        let x = LEFT + i as f64 * slot + gap / 2.0;
        let bw = (slot - gap).max(1.0);
        let mut acc = 0.0;
        let segments = h.per_band.iter().enumerate().filter(|(_, c)| **c > 0).map(|(bi, c)| {
            let y0 = y(acc);
            acc += *c as f64;
            let y1 = y(acc);
            // 1px surface gap between stacked segments
            let height = (y0 - y1 - 1.0).max(0.5);
            html! {
                <rect x={x.to_string()} y={y1.to_string()} width={bw.to_string()} height={height.to_string()}
                    fill={band_color(&report.bands[bi])} />
            }
        }).collect::<Html>();

        let hover_set = hover.clone();
        let hover_clear = hover.clone();
        let on_focus = p.data.on_focus.clone();
        let start = h.start;
        let label = (i % label_every == 0).then(|| {
            let text = if h.start.format("%H").to_string() == "00" || i == 0 {
                h.start.format("%a %H").to_string()
            } else {
                h.start.format("%H").to_string()
            };
            html! { <text class="axis" x={(x + bw / 2.0).to_string()} y={(H - 10.0).to_string()} text-anchor="middle">{ text }</text> }
        });

        html! {
            <g class={classes!("hour", (*hover == Some(i)).then_some("hovered"))}>
                { segments }
                { for label }
                <rect class="hit" x={(LEFT + i as f64 * slot).to_string()} y={TOP.to_string()}
                    width={slot.to_string()} height={plot_h.to_string()}
                    onmouseenter={Callback::from(move |_| hover_set.set(Some(i)))}
                    onmouseleave={Callback::from(move |_| hover_clear.set(None))}
                    onclick={Callback::from(move |_| on_focus.emit(Focus::Hour(start)))} />
            </g>
        }
    });

    let tooltip = (*hover).and_then(|i| hours.get(i).map(|h| (i, h))).map(|(i, h)| {
        let left = (LEFT + (i as f64 + 0.5) * slot) / W * 100.0;
        let align = if left > 70.0 { "right" } else { "left" };
        html! {
            <div class={classes!("chart-tip", align)} style={format!("left:{:.2}%", left)}>
                <div class="tip-title">{ h.start.format("%a %d %b, %H:00–%H:59").to_string() }</div>
                <div class="tip-row strong"><span>{"QSOs"}</span><span>{ h.total }</span></div>
                { for h.per_band.iter().enumerate().filter(|(_, c)| **c > 0).map(|(bi, c)| html! {
                    <div class="tip-row">
                        <span><span class="swatch" style={format!("background:{}", band_color(&report.bands[bi]))}></span>{ &report.bands[bi] }</span>
                        <span>{ c }</span>
                    </div>
                }) }
                <div class="tip-row muted"><span>{"Running total"}</span><span>{ h.cumulative }</span></div>
                if !h.new_dxcc.is_empty() || !h.new_zones.is_empty() {
                    <div class="tip-row muted"><span>{"New mults"}</span><span>{ h.new_dxcc.len() + h.new_zones.len() }</span></div>
                }
                <div class="tip-hint">{"Click to open in log"}</div>
            </div>
        }
    });

    html! {
        <div class="chart">
            <Legend data={p.data.clone()} />
            <div class="chart-body">
                <svg viewBox={format!("0 0 {} {}", W, H)} role="img" aria-label="QSOs per hour by band">
                    { for ticks }
                    <line class="baseline" x1={LEFT.to_string()} x2={(W - RIGHT).to_string()}
                        y1={(TOP + plot_h).to_string()} y2={(TOP + plot_h).to_string()} />
                    { for bars }
                </svg>
                { for tooltip }
            </div>
        </div>
    }
}

/// Band legend; clicking a band applies it as the global filter.
#[function_component(Legend)]
pub fn legend(p: &ViewProps) -> Html {
    let bands = &p.data.report.bands;
    if bands.len() < 2 {
        return html! {};
    }
    html! {
        <div class="legend">
            { for bands.iter().map(|b| {
                let on_band = p.data.on_band.clone();
                let band = b.clone();
                html! {
                    <button class="legend-item" title={format!("Show only {}", b)}
                        onclick={Callback::from(move |_| on_band.emit(Some(band.clone())))}>
                        <span class="swatch" style={format!("background:{}", band_color(b))}></span>{ b }
                    </button>
                }
            }) }
        </div>
    }
}

const SECTORS: [&str; 16] = [
    "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW", "NW",
    "NNW",
];

/// QSOs by beam heading, as a 16-sector rose. Radius is proportional to sqrt(count)
/// so the wedge area tracks the QSO count.
#[function_component(HeadingRose)]
pub fn heading_rose(p: &ViewProps) -> Html {
    let headings = &p.data.report.headings;
    let max = *headings.iter().max().unwrap_or(&1).max(&1) as f64;
    let (cx, cy, rmax) = (160.0, 160.0, 125.0);
    let point = |deg: f64, r: f64| {
        let a = deg.to_radians();
        (cx + r * a.sin(), cy - r * a.cos())
    };
    let wedges = headings.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| {
        let r = rmax * (*c as f64 / max).sqrt();
        let mid = i as f64 * 22.5;
        let (x1, y1) = point(mid - 10.5, r);
        let (x2, y2) = point(mid + 10.5, r);
        let d = format!(
            "M{cx},{cy} L{x1:.1},{y1:.1} A{r:.1},{r:.1} 0 0 1 {x2:.1},{y2:.1} Z"
        );
        html! {
            <path class="wedge" d={d}>
                <title>{ format!("{} ({:.0}°): {} QSOs", SECTORS[i], mid, c) }</title>
            </path>
        }
    });
    let rings = [0.25f64, 0.5, 1.0].map(|f| {
        html! { <circle class="grid" cx={cx.to_string()} cy={cy.to_string()} r={(rmax * f.sqrt()).to_string()} fill="none" /> }
    });
    let labels = [("N", 0.0), ("E", 90.0), ("S", 180.0), ("W", 270.0)].map(|(l, deg)| {
        let (x, y) = point(deg, rmax + 18.0);
        html! { <text class="axis" x={x.to_string()} y={(y + 4.0).to_string()} text-anchor="middle">{ l }</text> }
    });
    html! {
        <svg class="rose" viewBox="0 0 320 320" role="img" aria-label="QSOs by beam heading">
            { for rings }
            { for wedges }
            { for labels }
        </svg>
    }
}

/// CQ zones 1-40: worked zones shaded by QSO count, missing zones outlined.
#[function_component(ZoneGrid)]
pub fn zone_grid(p: &ViewProps) -> Html {
    let data: &Data = &p.data;
    let zones = &data.report.zones;
    let max = zones.iter().map(|z| z.total).max().unwrap_or(1);
    html! {
        <div class="zone-grid">
            { for (1..=40u32).map(|zone| {
                match zones.iter().find(|z| z.zone == zone) {
                    Some(z) => html! {
                        <button class="zone worked" style={format!("--v:{:.2}", z.total as f64 / max as f64)}
                            title={format!("Zone {}: {} QSOs, first {}", zone, z.total, z.first.format("%a %H:%M"))}
                            onclick={data.focus(Focus::Zone(zone))}>
                            <span class="zone-no">{ zone }</span>
                            <span class="zone-count">{ z.total }</span>
                        </button>
                    },
                    None => html! {
                        <div class="zone missing" title={format!("Zone {}: not worked", zone)}>
                            <span class="zone-no">{ zone }</span>
                        </div>
                    },
                }
            }) }
        </div>
    }
}
