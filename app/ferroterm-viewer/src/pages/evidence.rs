//! The evidence screen: what this build passes, and what it measured.
//!
//! Every other screen reads the running server. This one reads nothing: the
//! figures were taken out of the repository's committed files when the bundle
//! was built, so they describe the build a deployment is running rather than
//! the deployment itself. Each figure names the file it came from.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::NOT_DECLARED;
use crate::evidence::Conformance;
use crate::evidence::Latency;
use crate::evidence::Run;
use crate::evidence::SystemRun;
use crate::evidence::embedded;
use crate::evidence::relative_width;

/// The classes every table on this screen carries.
const TABLE: &str = "w-full border-collapse text-left text-sm";

/// The classes a column heading carries.
const HEAD: &str = "py-2 pr-3 font-medium align-bottom";

/// The classes a body cell carries.
const CELL: &str = "py-2 pr-3 align-top";

/// The classes a cell holding a figure carries.
const FIGURE: &str = "py-2 pr-3 align-top tabular-nums whitespace-nowrap";

/// The classes a file path carries.
const PATH: &str = "font-mono text-xs break-all text-slate-600 dark:text-slate-300";

/// Draws the conformance and benchmark evidence the bundle carries.
///
/// Nothing here is reactive, and nothing here is fetched. The figures were
/// fixed when the bundle was built, so the screen draws a constant.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn EvidencePage() -> impl IntoView {
    let heading = view! {
        <Title text="Evidence" />
        <h1 class="text-2xl font-semibold">"The evidence this build ships"</h1>
    }
    .into_any();

    let evidence = embedded();
    let preamble = preamble(evidence.release);
    let conformance = conformance_section(evidence.conformance);
    let latency = latency_section(evidence.latency);
    let run = run_section(evidence.run);

    view! {
        {heading}
        {preamble}
        {conformance}
        {latency}
        {run}
    }
}

/// What the screen states, and what it does not.
fn preamble(release: &'static str) -> AnyView {
    view! {
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "These figures describe FerroTERM " <span class="font-medium">{release}</span>
            ", the build this bundle was compiled from. They say nothing about the server answering this page: what it loaded and what it answers now are on the other screens."
        </p>
        <p class="mt-2 text-sm text-slate-600 dark:text-slate-300">
            "This screen issues no request. Every number below was read out of a file the repository commits when the bundle was built, and each one names that file, so you can open it and check the number yourself."
        </p>
    }
    .into_any()
}

/// The HL7 terminology ecosystem suite, mode by mode.
fn conformance_section(conformance: Conformance) -> AnyView {
    let rows: Vec<AnyView> = conformance
        .modes
        .iter()
        .map(|mode| {
            let share = mode.share();
            view! {
                <tr class="border-b border-slate-200 dark:border-slate-800">
                    <th scope="row" class="py-2 pr-3 text-left font-mono text-xs font-normal">
                        {mode.name}
                    </th>
                    <td class=CELL>
                        <span class="font-mono text-xs">{mode.surface}</span>
                    </td>
                    <td class=FIGURE>{mode.passed} " of " {mode.ran}</td>
                    <td class="w-40 py-2 pr-3 align-top">
                        <span class="tabular-nums">{share} "%"</span>
                        {proportion(share)}
                    </td>
                    <td class=CELL>
                        <span class=PATH>{mode.source}</span>
                    </td>
                </tr>
            }
            .into_any()
        })
        .collect();

    let total = conformance.suite_total;
    let total_source = conformance.total_source;
    let table_source = conformance.table_source;

    view! {
        <section class="mt-8" aria-labelledby="conformance-heading">
            <h2 id="conformance-heading" class="text-lg font-medium">
                "The HL7 terminology ecosystem suite"
            </h2>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "The suite groups its cases into modes, and a run picks one. Each row is one mode run against one served FHIR root. The `general` mode runs "
                {total}
                " cases and needs nothing to run, so continuous integration runs it on every push; the other modes need licensed content or a code system this server does not serve, so they are run by hand before a release."
            </p>
            <div class="mt-3 overflow-x-auto">
                <table class=TABLE>
                    <caption class="pb-1 text-left text-xs font-medium tracking-wide uppercase">
                        "The cases each mode passes, from the committed pass lists"
                    </caption>
                    <thead>
                        <tr class="border-b border-slate-300 dark:border-slate-700">
                            <th scope="col" class=HEAD>
                                "Mode"
                            </th>
                            <th scope="col" class=HEAD>
                                "Surface"
                            </th>
                            <th scope="col" class=HEAD>
                                "Passed"
                            </th>
                            <th scope="col" class=HEAD>
                                "Share"
                            </th>
                            <th scope="col" class=HEAD>
                                "From"
                            </th>
                        </tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </div>
            <p class="mt-2 text-xs text-slate-600 dark:text-slate-300">
                "The case counts come from " <span class=PATH>{table_source}</span>
                " and the suite total from " <span class=PATH>{total_source}</span>
                ". The build stops when a pass list and that table disagree."
            </p>
        </section>
    }
    .into_any()
}

/// The latency bars, and the run recorded against each.
fn latency_section(latency: Latency) -> AnyView {
    let widest = latency
        .bars
        .iter()
        .map(|bar| bar.measured_us)
        .max()
        .unwrap_or_default();
    let rows: Vec<AnyView> = latency
        .bars
        .iter()
        .map(|bar| {
            let headroom = bar.headroom().map_or_else(
                || "faster than the run could time".to_owned(),
                |times| format!("{times} times under it"),
            );
            view! {
                <tr class="border-b border-slate-200 dark:border-slate-800">
                    <th scope="row" class="py-2 pr-3 text-left font-mono text-xs font-normal">
                        {bar.bench}
                    </th>
                    <td class=FIGURE>{bar.max_us} " µs"</td>
                    <td class="w-48 py-2 pr-3 align-top">
                        <span class="tabular-nums whitespace-nowrap">{bar.measured_us} " µs"</span>
                        {proportion(relative_width(bar.measured_us, widest))}
                    </td>
                    <td class=FIGURE>{headroom}</td>
                    <td class=CELL>{bar.claim}</td>
                </tr>
            }
            .into_any()
        })
        .collect();

    let machine = latency.machine;
    let source = latency.source;

    view! {
        <section class="mt-10" aria-labelledby="latency-heading">
            <h2 id="latency-heading" class="text-lg font-medium">
                "The latency the project claims"
            </h2>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "A bar is the claim, and it never moves to match a slower run. The measurement beside it records what one machine answered, so the room a run has is visible. The recorded run was taken on "
                <span class="font-medium">{machine}</span> "."
            </p>
            <div class="mt-3 overflow-x-auto">
                <table class=TABLE>
                    <caption class="pb-1 text-left text-xs font-medium tracking-wide uppercase">
                        "Each benchmark, its bar, and the run recorded against it"
                    </caption>
                    <thead>
                        <tr class="border-b border-slate-300 dark:border-slate-700">
                            <th scope="col" class=HEAD>
                                "Benchmark"
                            </th>
                            <th scope="col" class=HEAD>
                                "Bar"
                            </th>
                            <th scope="col" class=HEAD>
                                "Measured"
                            </th>
                            <th scope="col" class=HEAD>
                                "Room"
                            </th>
                            <th scope="col" class=HEAD>
                                "What the bar claims"
                            </th>
                        </tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </div>
            <p class="mt-2 text-xs text-slate-600 dark:text-slate-300">
                "Every figure in this table comes from " <span class=PATH>{source}</span>
                ". A bar beside a measurement is drawn against the slowest measurement in the table, so the shape reads; the microseconds beside it are the figure."
            </p>
        </section>
    }
    .into_any()
}

/// The newest committed benchmark run, system by system.
fn run_section(run: Run) -> AnyView {
    let systems: Vec<AnyView> = run.systems.iter().map(system_view).collect();
    let name = run.name;
    let source = run.source;
    view! {
        <section class="mt-10" aria-labelledby="run-heading">
            <h2 id="run-heading" class="text-lg font-medium">
                "The newest benchmark run"
            </h2>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "One record per code system the run loaded, from " <span class=PATH>{source}</span>
                ", the run named " <span class="font-medium">{name}</span>
                ". A record states the machine it was taken on and the FerroTERM version that answered it, because a timing without both says nothing."
            </p>
            {systems}
        </section>
    }
    .into_any()
}

/// One code system's record within the run.
fn system_view(system: &SystemRun) -> AnyView {
    let facts = facts_view(system);
    let timings = timings_view(system);
    view! {
        <article class="mt-6 rounded-md border border-slate-200 p-4 dark:border-slate-800">
            <h3 class="text-base font-medium">
                {system.system} " " <span class="font-normal">{system.system_version}</span>
            </h3>
            <p class=PATH>{system.system_uri}</p>
            {facts}
            {timings}
        </article>
    }
    .into_any()
}

/// What the record says about the artifact and the process that served it.
fn facts_view(system: &SystemRun) -> AnyView {
    let ingest = system
        .ingest_seconds
        .map_or_else(|| NOT_DECLARED.to_owned(), |seconds| format!("{seconds} s"));
    let release = system.release.unwrap_or("built outside this run");
    view! {
        <dl class="mt-3 grid gap-x-4 gap-y-1 text-sm sm:grid-cols-[10rem_1fr]">
            <dt class="font-medium">"Concepts"</dt>
            <dd class="tabular-nums">{system.concepts}</dd>
            <dt class="font-medium">"Offline build"</dt>
            <dd class="tabular-nums">{ingest}</dd>
            <dt class="font-medium">"Built from"</dt>
            <dd class="font-mono text-xs break-all">{release}</dd>
            <dt class="font-medium">"Ready after"</dt>
            <dd class="tabular-nums">{system.ready_seconds} " s"</dd>
            <dt class="font-medium">"Resident memory"</dt>
            <dd class="tabular-nums">{system.resident_memory}</dd>
            <dt class="font-medium">"Served as"</dt>
            <dd class="font-mono text-xs">"/" {system.fhir}</dd>
            <dt class="font-medium">"Answered by"</dt>
            <dd>{system.built_by}</dd>
            <dt class="font-medium">"Machine"</dt>
            <dd>{system.machine}</dd>
            <dt class="font-medium">"Taken at"</dt>
            <dd class="font-mono text-xs break-all">{system.taken_at}</dd>
            <dt class="font-medium">"Record"</dt>
            <dd class=PATH>{system.source}</dd>
        </dl>
    }
    .into_any()
}

/// The operations the run timed, one row each.
fn timings_view(system: &SystemRun) -> AnyView {
    if system.operations.is_empty() {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This record timed no operation."
            </p>
        }
        .into_any();
    }
    let rows: Vec<AnyView> = system
        .operations
        .iter()
        .map(|timing| {
            view! {
                <tr class="border-b border-slate-200 dark:border-slate-800">
                    <th scope="row" class="py-2 pr-3 text-left font-mono text-xs font-normal">
                        {timing.operation}
                    </th>
                    <td class=FIGURE>{timing.status}</td>
                    <td class=FIGURE>{timing.cold_ms}</td>
                    <td class=FIGURE>{timing.p50_ms}</td>
                    <td class=FIGURE>{timing.p95_ms}</td>
                    <td class=FIGURE>{timing.p99_ms}</td>
                    <td class=FIGURE>{timing.warm_requests}</td>
                </tr>
            }
            .into_any()
        })
        .collect();
    let caption = format!("What {} answered, in milliseconds", system.system);
    view! {
        <div class="mt-4 overflow-x-auto">
            <table class=TABLE>
                <caption class="pb-1 text-left text-xs font-medium tracking-wide uppercase">
                    {caption}
                </caption>
                <thead>
                    <tr class="border-b border-slate-300 dark:border-slate-700">
                        <th scope="col" class=HEAD>
                            "Operation"
                        </th>
                        <th scope="col" class=HEAD>
                            "Status"
                        </th>
                        <th scope="col" class=HEAD>
                            "Cold"
                        </th>
                        <th scope="col" class=HEAD>
                            "Median"
                        </th>
                        <th scope="col" class=HEAD>
                            "95th"
                        </th>
                        <th scope="col" class=HEAD>
                            "99th"
                        </th>
                        <th scope="col" class=HEAD>
                            "Warm requests"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// A bar drawn beside a figure, at `percent` of the cell.
///
/// It draws the figure the cell already states in words, so it carries no
/// meaning of its own and is hidden from assistive technology
/// (<https://www.w3.org/TR/wai-aria-1.2/#aria-hidden>). Nothing on this screen
/// is readable only as a shape or only as a colour.
fn proportion(percent: u32) -> AnyView {
    view! {
        <span
            aria-hidden="true"
            class="mt-1 block h-1.5 w-full rounded bg-slate-200 dark:bg-slate-800"
        >
            <span
                class="block h-1.5 rounded bg-brand-600 dark:bg-brand-400"
                style=format!("width:{percent}%")
            ></span>
        </span>
    }
    .into_any()
}
