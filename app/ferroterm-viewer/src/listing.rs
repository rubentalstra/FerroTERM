//! The chrome the resource listing screens share: the address, and the pager.
//!
//! `GET [base]/{type}` answers one `searchset` with every match in it: no
//! FHIR specification requires a server to page a search, and this one does
//! not (<https://hl7.org/fhir/R4B/http.html#search>). So the pages a reader
//! walks are the viewer's own, over the answer that arrived, and each screen
//! says so. The arithmetic and the address live here, outside every
//! component, so plain unit tests pin them.

use leptos::ev::SubmitEvent;
use leptos::html::Input;
use leptos::prelude::*;

use crate::components::NOT_DECLARED;
use crate::components::field::Field;
use crate::components::field::help_toggle;
use crate::components::field::row;
use crate::components::field::text_field;
use crate::components::icon;
use crate::components::icon::Glyph;
use crate::components::icon::Icon;
use crate::fhir::searchset::SearchFilter;
use crate::fhir::version::FhirVersion;
use crate::paging::Page;
use crate::routes::UI_BASE;
use crate::routes::VERSION_PARAM;
use crate::styles;
use crate::url::RequestUrl;

/// The address parameter carrying the canonical the search filters on.
const URL_PARAM: &str = "url";

/// The address parameter carrying the resource version the search filters on.
const RESOURCE_VERSION_PARAM: &str = "version";

/// The address parameter carrying the page the reader is on.
const PAGE_PARAM: &str = "page";

/// The address parameter carrying the id of the resource being read.
const ID_PARAM: &str = "id";

/// What a listing screen's address says: the filter, the page, and the read.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ListParams {
    /// The search parameters the listing sends.
    pub(crate) filter: SearchFilter,
    /// The id of the resource the detail pane reads, empty for none.
    pub(crate) id: String,
    /// The 1-based page the reader asked for.
    page: u32,
    /// How many rows a page holds, from the reader's stored preference.
    size: u32,
}

impl ListParams {
    /// Reads the parameters out of the address.
    ///
    /// `query` looks one parameter up, which is what a `ParamsMap` does and
    /// what a test can do without a browser. A page that is not a whole
    /// number above zero is a link a reader typed, so it reads as the first
    /// page rather than refusing the whole screen.
    pub(crate) fn read(query: &dyn Fn(&str) -> Option<String>, size: u32) -> Self {
        let text = |name: &str| query(name).unwrap_or_default().trim().to_owned();
        Self {
            filter: SearchFilter {
                url: text(URL_PARAM),
                version: text(RESOURCE_VERSION_PARAM),
            },
            id: text(ID_PARAM),
            page: query(PAGE_PARAM)
                .and_then(|typed| typed.trim().parse::<u32>().ok())
                .filter(|page| *page > 0)
                .unwrap_or(1),
            size: size.max(1),
        }
    }

    /// The 1-based page the address asked for.
    pub(crate) fn page(&self) -> u32 {
        self.page
    }

    /// How many rows a page holds.
    pub(crate) fn size(&self) -> u32 {
        self.size
    }

    /// These parameters with another filter, back at the first page.
    ///
    /// A changed filter selects a different answer, so the walk starts again
    /// and the resource being read is left behind: an id from the previous
    /// search need not be in the new one.
    pub(crate) fn searching(&self, filter: SearchFilter) -> Self {
        Self {
            filter,
            id: String::new(),
            page: 1,
            size: self.size,
        }
    }

    /// These parameters, reading one resource by its id.
    pub(crate) fn reading(&self, id: &str) -> Self {
        Self {
            id: id.to_owned(),
            ..self.clone()
        }
    }

    /// These parameters, moved onto another page.
    pub(crate) fn on(&self, page: u32) -> Self {
        Self {
            page: page.max(1),
            ..self.clone()
        }
    }

    /// The viewer address these parameters are, for a link or a navigation.
    ///
    /// Every value stays in the query, which a click navigation carries
    /// through untouched while it unescapes a path segment a second time
    /// (`leptos_router` 0.8.15 `src/location/mod.rs`).
    pub(crate) fn address(&self, path: &str, version: FhirVersion) -> String {
        self.address_with(path, version, &[])
    }

    /// The same address, with `extra` parameters the screen also carries.
    ///
    /// A screen that puts a second form in the same address round-trips its
    /// parameters here, so walking a page or opening a resource does not throw
    /// away a run the reader has going.
    pub(crate) fn address_with(
        &self,
        path: &str,
        version: FhirVersion,
        extra: &[(&str, &str)],
    ) -> String {
        let mut url = RequestUrl::new()
            .segment(UI_BASE.trim_start_matches('/'))
            .segment(path)
            .query(VERSION_PARAM, version.segment());
        if !self.filter.url.is_empty() {
            url = url.query(URL_PARAM, &self.filter.url);
        }
        if !self.filter.version.is_empty() {
            url = url.query(RESOURCE_VERSION_PARAM, &self.filter.version);
        }
        if self.page > 1 {
            url = url.query(PAGE_PARAM, &self.page.to_string());
        }
        if !self.id.is_empty() {
            url = url.query(ID_PARAM, &self.id);
        }
        for (name, value) in extra {
            if !value.is_empty() {
                url = url.query(name, value);
            }
        }
        url.render("")
    }
}

/// What a search answered, as the sentence a live region reads.
///
/// `counted` is `Bundle.total`, "the total number of matches" a search found
/// (<https://hl7.org/fhir/R4B/http.html#search>), which can exceed the entries
/// this answer carries when the server paged the search itself. This viewer
/// walks its own pages over what arrived, so a larger total is stated as a
/// partial answer rather than printed beside a row count it disagrees with.
pub(crate) fn count_sentence(view: Window, noun: &str, counted: Option<u32>) -> String {
    let shown = view.summary(noun);
    let Some(total) = counted else {
        return format!("{shown} This root counted {NOT_DECLARED}.");
    };
    let carried = u32::try_from(view.matched).unwrap_or(u32::MAX);
    if total > carried {
        return format!(
            "{shown} This root counted {total} and sent {carried}, so this is part of the answer."
        );
    }
    format!("{shown} This root counted {total}.")
}

/// The rows one page draws, and what the reader is told about them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Window {
    /// The page actually drawn, which may be earlier than the one asked for.
    page: Page,
    /// The index of the first row drawn.
    start: usize,
    /// How many rows are drawn.
    rows: usize,
    /// How many rows the answer holds in all.
    matched: usize,
}

/// The window `asked` covers over `matched` rows of `size` each.
///
/// The page is clamped to the last one that holds a row, so what is drawn and
/// what is announced always describe each other. That matters under
/// `<Transition>`, where the address advances a paint before the rows do.
pub(crate) fn window(asked: u32, size: u32, matched: usize) -> Window {
    let size = size.max(1);
    let total = u32::try_from(matched).unwrap_or(u32::MAX);
    let wanted = Page::at(asked.saturating_sub(1).saturating_mul(size), size);
    let page = if wanted.offset() >= total {
        Page::at(0, size).last(total)
    } else {
        wanted
    };
    let start = usize::try_from(page.offset()).unwrap_or(usize::MAX);
    Window {
        page,
        start,
        rows: matched.saturating_sub(start).min(size_as_usize(size)),
        matched,
    }
}

/// A page size as an index count, saturating where the target is narrower.
fn size_as_usize(size: u32) -> usize {
    usize::try_from(size).unwrap_or(usize::MAX)
}

impl Window {
    /// The indexes of the rows this page draws.
    pub(crate) fn indexes(self) -> impl Iterator<Item = usize> {
        (self.start..).take(self.rows)
    }

    /// Where this page sits in the walk.
    pub(crate) fn position(self) -> String {
        let total = u32::try_from(self.matched).unwrap_or(u32::MAX);
        format!(
            "Page {} of {}",
            self.page.number(),
            self.page.total_pages(total)
        )
    }

    /// Which rows of the answer this page holds, for a live region.
    ///
    /// `noun` is the plural a reader reads, in the middle of a sentence.
    pub(crate) fn summary(self, noun: &str) -> String {
        if self.rows == 0 {
            return format!("No {noun} on this page.");
        }
        let first = self.start.saturating_add(1);
        let last = self.start.saturating_add(self.rows);
        format!("Showing {noun} {first} to {last} of {}.", self.matched)
    }
}

/// The filter, as a form that navigates rather than reloading the page.
///
/// The router installs no `submit` listener, so the submit is handled by the
/// caller and turned into a navigation. The handler arrives boxed, so both
/// screens share one emitted body rather than one monomorphized copy each
/// (<https://doc.rust-lang.org/book/ch10-01-syntax.html>). Each control is seeded from the
/// address through `prop:value`, which follows a back navigation and leaves
/// what the reader is typing alone, and is read back at submit.
pub(crate) fn filter_form(
    canonical_hint: &'static str,
    canonical: NodeRef<Input>,
    resource_version: NodeRef<Input>,
    params: Signal<ListParams>,
    submit: Box<dyn FnMut(SubmitEvent)>,
) -> AnyView {
    let canonical_field = Field {
        id: "filter-url",
        name: "url",
        label: "Canonical",
        hint: canonical_hint,
    };
    let version_field = Field {
        id: "filter-version",
        name: "version",
        label: "Version",
        hint: "The version search parameter. Left empty, every version this root holds matches.",
    };
    view! {
        <form class="mt-loose grid gap-loose" on:submit=submit>
            {row(
                vec![
                    text_field(
                        canonical_field,
                        canonical,
                        Memo::new(move |_| params.with(|params| params.filter.url.clone())),
                    ),
                    text_field(
                        version_field,
                        resource_version,
                        Memo::new(move |_| params.with(|params| params.filter.version.clone())),
                    ),
                ],
            )}
            <div class="flex flex-wrap items-center gap-default">
                <button type="submit" class=styles::SUBMIT>
                    <Icon glyph=icon::SEARCH />
                    "Search"
                </button>
                {help_toggle()}
            </div>
        </form>
    }
    .into_any()
}

/// One canonical in a listing row: a link that opens it, or a statement.
///
/// A resource that carries no id cannot be read back, so it is stated rather
/// than drawn as a link that would answer nothing. `Resource.id` is optional
/// on the wire (<https://hl7.org/fhir/R4B/resource.html#id>), and a searchset
/// entry is free to carry only its `fullUrl`, so the unlinked arm stays: the
/// screen renders what arrived rather than assuming what a root sends.
pub(crate) fn canonical_cell(canonical: &str, href: Option<String>) -> AnyView {
    let canonical = canonical.to_owned();
    match href {
        Some(href) => view! {
            <a href=href class="text-accent underline">
                {canonical}
            </a>
        }
        .into_any(),
        None => view! {
            {canonical}
            <span class="sr-only">", which carries no id to read it by"</span>
        }
        .into_any(),
    }
}

/// The page controls, which are the address of another page.
///
/// Each control is a link, so a page is shareable and the browser walks the
/// list with its back button. A control that leads nowhere reads "unavailable"
/// in visible text, because a reader who cannot tell the two tints apart has
/// nothing else to go on
/// (<https://www.w3.org/TR/WCAG22/#use-of-color>).
pub(crate) fn pager_view(
    label: &'static str,
    view: Window,
    params: &ListParams,
    path: &'static str,
    version: FhirVersion,
    extra: &[(&str, &str)],
) -> AnyView {
    let total = u32::try_from(view.matched).unwrap_or(u32::MAX);
    let here = view.page.number();
    let step = |target: Option<Page>, glyph: Glyph, text: &'static str| -> AnyView {
        match target {
            Some(page) if page.number() != here => {
                let href = params.on(page.number()).address_with(path, version, extra);
                view! {
                    <a href=href class=styles::BUTTON>
                        <Icon glyph=glyph />
                        {text}
                    </a>
                }
                .into_any()
            }
            Some(_) | None => view! {
                <span class=styles::BUTTON_DISABLED>
                    <Icon glyph=glyph />
                    {text}
                    " (unavailable)"
                </span>
            }
            .into_any(),
        }
    };
    view! {
        <nav aria-label=label class="mt-default flex flex-wrap items-center gap-default">
            {step(Some(Page::at(0, view.page.count())), icon::PAGE_FIRST, "First page")}
            {step(view.page.previous(), icon::PAGE_PREVIOUS, "Previous page")}
            <p class="text-body font-medium">{view.position()}</p>
            {step(view.page.next(total), icon::PAGE_NEXT, "Next page")}
            {step(Some(view.page.last(total)), icon::PAGE_LAST, "Last page")}
        </nav>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the address, which reads the same way a `ParamsMap` does.
    fn map<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + use<'a> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn an_address_that_names_nothing_reads_as_the_first_unfiltered_page() {
        let params = ListParams::read(&map(&[]), 25);
        assert_eq!(params.page(), 1);
        assert_eq!(params.size(), 25);
        assert_eq!(
            params.filter,
            SearchFilter::default(),
            "nothing was asked for, so nothing narrows the search"
        );
        assert!(params.id.is_empty());
    }

    #[test]
    fn a_page_that_is_not_a_whole_number_reads_as_the_first_one() {
        assert_eq!(ListParams::read(&map(&[("page", "x")]), 25).page(), 1);
        assert_eq!(
            ListParams::read(&map(&[("page", "0")]), 25).page(),
            1,
            "a page before the first one is a link a reader typed"
        );
    }

    #[test]
    fn the_address_carries_every_parameter_the_screen_reads_back() {
        let params = ListParams::read(
            &map(&[
                ("url", "https://terminology.example/ValueSet/x?a=b"),
                ("version", "2031"),
                ("page", "3"),
                ("id", "vs-1"),
            ]),
            25,
        );
        assert_eq!(
            params.address("valuesets", FhirVersion::R4B),
            "/ui/valuesets?fhir=r4b&url=https%3A%2F%2Fterminology.example%2FValueSet%2Fx%3Fa%3Db\
             &version=2031&page=3&id=vs-1",
            "a canonical carrying its own query string cannot truncate the address"
        );
    }

    #[test]
    fn the_first_page_is_left_out_of_the_address() {
        let params = ListParams::read(&map(&[]), 25);
        assert_eq!(
            params.address("conceptmaps", FhirVersion::R5),
            "/ui/conceptmaps?fhir=r5",
            "an address says what was asked for, and nothing was"
        );
    }

    #[test]
    fn searching_again_starts_the_walk_over_and_closes_the_open_resource() {
        let params = ListParams::read(&map(&[("page", "4"), ("id", "vs-1")]), 25);
        let searched = params.searching(SearchFilter {
            url: "https://x.example/v".to_owned(),
            version: String::new(),
        });
        assert_eq!(searched.page(), 1);
        assert!(
            searched.id.is_empty(),
            "an id from the previous search need not be in the new one"
        );
    }

    #[test]
    fn opening_a_resource_keeps_the_page_the_reader_is_on() {
        let params = ListParams::read(&map(&[("page", "2")]), 25);
        let reading = params.reading("vs-9");
        assert_eq!(reading.page(), 2);
        assert_eq!(reading.id, "vs-9");
    }

    #[test]
    fn a_screen_that_carries_a_second_form_round_trips_its_parameters() {
        let params = ListParams::read(&map(&[("page", "2")]), 25);
        assert_eq!(
            params.address_with(
                "conceptmaps",
                FhirVersion::R4B,
                &[
                    ("code", "x"),
                    ("target", ""),
                    ("system", "https://x.example/s")
                ]
            ),
            "/ui/conceptmaps?fhir=r4b&page=2&code=x&system=https%3A%2F%2Fx.example%2Fs",
            "a parameter the reader left empty is left out rather than sent empty"
        );
    }

    #[test]
    fn the_sentence_states_what_is_drawn_and_what_the_root_counted() {
        assert_eq!(
            count_sentence(window(1, 25, 3), "value sets", Some(3)),
            "Showing value sets 1 to 3 of 3. This root counted 3."
        );
        assert_eq!(
            count_sentence(window(1, 25, 0), "concept maps", None),
            "No concept maps on this page. This root counted not declared."
        );
    }

    #[test]
    fn a_search_the_server_paged_itself_is_stated_as_part_of_the_answer() {
        assert_eq!(
            count_sentence(window(1, 25, 25), "value sets", Some(500)),
            "Showing value sets 1 to 25 of 25. This root counted 500 and sent 25, \
             so this is part of the answer.",
            "a total larger than the entries carried means the server paged the search"
        );
    }

    #[test]
    fn a_window_draws_the_rows_of_the_page_it_is_on() {
        let view = window(2, 10, 25);
        assert_eq!(
            view.indexes().collect::<Vec<usize>>(),
            (10..20).collect::<Vec<usize>>()
        );
        assert_eq!(view.position(), "Page 2 of 3");
        assert_eq!(
            view.summary("value sets"),
            "Showing value sets 11 to 20 of 25."
        );
    }

    #[test]
    fn a_short_last_page_draws_only_the_rows_it_has() {
        let view = window(3, 10, 25);
        assert_eq!(view.indexes().count(), 5);
        assert_eq!(
            view.summary("value sets"),
            "Showing value sets 21 to 25 of 25."
        );
    }

    #[test]
    fn a_page_past_the_end_is_clamped_so_the_rows_and_the_count_agree() {
        let view = window(9, 10, 25);
        assert_eq!(
            view.position(),
            "Page 3 of 3",
            "the reader is told about the page that is drawn, never one that is not"
        );
        assert_eq!(view.indexes().count(), 5);
    }

    #[test]
    fn an_empty_answer_is_one_page_rather_than_none() {
        let view = window(1, 10, 0);
        assert_eq!(view.position(), "Page 1 of 1");
        assert_eq!(view.indexes().count(), 0);
        assert_eq!(
            view.summary("Concept maps"),
            "No Concept maps on this page."
        );
    }
}
