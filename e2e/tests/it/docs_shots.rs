// SPDX-License-Identifier: BUSL-1.1
//! The documentation capture pass: one canonical PNG per viewer screen, for
//! the book page at `website/book/src/operate/viewer.md`.
//!
//! This is a capture, not an assertion. It drives the same deployment the
//! journeys drive and writes the images the book embeds, so every screenshot
//! in the documentation is a picture of a server that was running.
//! It is gated on [`GATE_ENV`], which `scripts/ui-e2e.sh` sets only when it is
//! asked for the capture pass: an ordinary journey run, on a pull request or
//! on a laptop, rewrites no tracked image.
//!
//! Every shot is taken over the synthetic fixture in `e2e/fixtures/codesystems`,
//! which the script mounts. No licensed release is ever the subject of a
//! committed image, and none may be: SNOMED CT and the other licensed
//! terminologies are not distributed by this repository, and a screenshot of
//! their concepts would be derived edition data.

use std::path::Path;
use std::path::PathBuf;

use thirtyfour::prelude::*;
use thirtyfour::stringmatch::StringMatch;

use crate::harness::Journey;
use crate::harness::server;
use crate::harness::session;

/// Names the capture pass. Unset, the pass skips and says so.
const GATE_ENV: &str = "FERROTERM_UI_E2E_DOCS_SHOTS";

/// Names the directory the images are written to.
const SHOTS_DIR_ENV: &str = "FERROTERM_UI_E2E_SHOTS_DIR";

/// Where the images live when nothing names a directory: the book's own
/// image directory, resolved from this crate's manifest.
const BOOK_IMAGES: &str = "../website/book/src/operate/img/viewer";

/// The FHIR version every capture is taken on, so the shots agree with each
/// other and with the page that embeds them.
///
/// R5 rather than the default, because the concept browser draws a tree only
/// for a version declaring `child-of`, and `filter-operator` defines that from
/// R5 onward (<https://hl7.org/fhir/R4B/codesystem-filter-operator.html>). A
/// capture on an R4-family root would show the browser with nothing to walk.
const CAPTURED_VERSION: &str = "r5";

/// A code the mounted fixture concept map maps.
///
/// The translate runner takes a code and the system it belongs to, and neither
/// is offered by the screen itself, so the capture types one. It is fixture
/// content; a deployment without the fixture fails this pass rather than
/// committing a picture of a refusal.
const MAPPED_CODE: &str = "ca-leaf";

/// A code system row's link into that system's own screen.
const SYSTEM_LINK: &str = "section[aria-labelledby='systems-heading'] tbody a";

/// A code system row whose version declares the direct-child operator.
///
/// The row is chosen by what the capability statement declares rather than by
/// which system it names, the way the journeys choose it, so the pass names no
/// code system. A row offers the concept browser only where its version
/// declares the direct-child operator, so the browse link is the mark.
const WALKABLE_ROW: &str =
    "//tr[.//a[contains(@href, '/ui/browse')]]//a[starts-with(@href, '/ui/systems/')]";

/// The code system screen's capability pane.
const CAPABILITY_PANE: &str = "section[aria-labelledby='system-capability-heading']";

/// The code system screen's published-resource pane.
const PUBLISHED_PANE: &str = "section[aria-labelledby='system-published-heading']";

/// The link from a code system's screen into the concept browser.
const BROWSE_LINK: &str = "nav[aria-label^='Screens for'] a[href*='/ui/browse']";

/// A concept the search answered, as the link that reads it.
const SEARCH_RESULT: &str = "section[aria-labelledby='browse-search-heading'] ul li a";

/// The tree once its first level has been answered.
const TREE_SETTLED: &str = "li[role='treeitem'], #browse-tree-empty";

/// One row of the tree.
const TREE_ROW: &str = "li[role='treeitem']";

/// The concept pane once the lookup has been answered.
const CONCEPT_DISPLAY: &str = "section[aria-labelledby='browse-concept-heading'] h3";

/// A parent of the concept being read, as the link that moves onto it.
const PARENT_LINK: &str = "nav[aria-label='Parents of this concept'] a";

/// The canonical of the first listed value set, which is the row's own heading.
const VALUE_SET_ROW: &str = "section[aria-labelledby='valuesets-heading'] tbody th";

/// The canonical a listed value set carries beside its name.
const VALUE_SET_CANONICAL: &str = "section[aria-labelledby='valuesets-heading'] tbody th span";

/// The command bar's own control, on every screen.
const COMMAND_BAR: &str = "#command-bar";

/// The command bar's submit control.
const COMMAND_SUBMIT: &str = "form[role='search'] button[type='submit']";

/// One offer the find screen made.
const OFFER: &str = "section[aria-labelledby='find-offers-heading'] li a";

/// The expansion runner's canonical control.
const EXPAND_URL: &str = "#expand-url";

/// The expansion runner's submit control.
///
/// Scoped to the main region, because the command bar in the header is a form
/// on every screen and an unscoped selector reaches it first.
const EXPAND_SUBMIT: &str = "main form button[type='submit']";

/// The expansion answer, whose arrival is what a shot of the runner waits for.
const EXPANSION: &str = "section[aria-labelledby='expansion-heading']";

/// One expanded concept.
const EXPANDED_ROW: &str = "section[aria-labelledby='expansion-heading'] tbody tr";

/// The concept maps this root holds, as the section a shot waits for.
const CONCEPT_MAPS: &str = "section[aria-labelledby='conceptmaps-heading']";

/// The translate runner's system control.
const TRANSLATE_SYSTEM: &str = "#translate-system";

/// The translate runner's code control.
const TRANSLATE_CODE: &str = "#translate-code";

/// The translate runner's submit control.
const TRANSLATE_SUBMIT: &str = "section[aria-labelledby='translate-heading'] button[type='submit']";

/// The translate runner's answer.
const TRANSLATE_ANSWER: &str = "section[aria-labelledby='translate-answer-heading']";

/// The sentence a translated code is answered with.
const TRANSLATED: &str = "The server translated the code.";

/// The validate runner's code system control.
const VALIDATE_SYSTEM: &str = "#validate-system";

/// The validate runner's code control.
const VALIDATE_CODE: &str = "#validate-code";

/// The validate runner's submit control.
const VALIDATE_SUBMIT: &str = "section[aria-labelledby='validate-heading'] button[type='submit']";

/// The validate runner's answer.
const VALIDATE_ANSWER: &str = "section[aria-labelledby='validate-heading']";

/// The opening words of the verdict a validated code is answered with.
const VALIDATED: &str = "result:";

/// The subsumption runner, which the same screen carries below the validation.
const SUBSUMES_SECTION: &str = "section[aria-labelledby='subsumes-heading']";

/// The version comparison, once the four reads have filled it.
const COMPARISON: &str = "section[aria-labelledby='comparison-heading']";

/// One of the comparison's two tables.
const COMPARISON_TABLE: &str = "section[aria-labelledby='comparison-heading'] table";

/// How many tables the comparison draws, so a shot is never taken while one
/// root is still answering.
const COMPARISON_TABLES: usize = 2;

/// The evidence screen's conformance pane.
const CONFORMANCE_PANE: &str = "section[aria-labelledby='conformance-heading']";

/// The evidence screen's latency pane.
const LATENCY_PANE: &str = "section[aria-labelledby='latency-heading']";

/// The evidence screen's benchmark-run pane.
const RUN_PANE: &str = "section[aria-labelledby='run-heading']";

/// The settings screen's theme control.
const THEME_CONTROL: &str = "#viewer-theme";

/// The width every shot is taken at, in CSS pixels.
///
/// It is fixed, so every image is the same width and the book renders a
/// consistent column. The height is fitted per screen by [`shot`].
const WIDTH: u32 = 1280;

/// The shortest and tallest a shot may be, in CSS pixels.
///
/// The floor keeps a short screen from becoming a letterbox; the ceiling keeps
/// one long answer from becoming an image nobody can read.
const HEIGHT_BOUNDS: (u32, u32) = (700, 2400);

/// A window short enough that any screen overflows it, so the body reports the
/// height of the content rather than the height of the viewport.
const PROBE_WINDOW: u32 = 400;

/// How much taller than the content the second probe's window is.
///
/// The window a driver sets is the outer one and the shot is the viewport
/// inside it, so the two differ by whatever frame the browser carries. This
/// room has to exceed that frame for the second probe to measure it.
const CHROME_ROOM: u32 = 400;

/// Where the images are written.
fn shots_dir() -> PathBuf {
    match std::env::var(SHOTS_DIR_ENV) {
        Ok(named) => PathBuf::from(named),
        Err(_absent) => Path::new(env!("CARGO_MANIFEST_DIR")).join(BOOK_IMAGES),
    }
}

/// The address of one viewer screen on the version every shot is taken on.
fn address(base: &str, path: &str) -> String {
    format!("{base}/ui/{path}?fhir={CAPTURED_VERSION}")
}

/// Fits the window to the screen in front of it, then takes the shot.
///
/// A screenshot is the viewport, so a screen taller than the window is cut off
/// and a shorter one trails empty space. The viewer's shell is `min-h-screen`,
/// which makes the body the taller of the content and the viewport, and that
/// is what the two probes below take apart: the first reads the content, the
/// second reads the viewport and so the frame the window adds to it. The image
/// is then exactly as tall as the screen, whatever browser took it.
async fn shot(journey: &Journey, path: &Path) -> WebDriverResult<()> {
    journey.resize(WIDTH, PROBE_WINDOW).await;
    let content = journey.page_height().await?;
    let roomy = content.saturating_add(CHROME_ROOM);
    journey.resize(WIDTH, roomy).await;
    let frame = roomy.saturating_sub(journey.page_height().await?);
    let wanted = content.clamp(HEIGHT_BOUNDS.0, HEIGHT_BOUNDS.1);
    journey.resize(WIDTH, wanted.saturating_add(frame)).await;
    journey.shot(path).await;
    Ok(())
}

/// The overview: what this deployment serves, one row per served version.
async fn overview(journey: &Journey, dir: &Path) -> WebDriverResult<()> {
    journey
        .element(By::Css(SYSTEM_LINK), "a code system row on the overview")
        .await;
    shot(journey, &dir.join("overview.png")).await
}

/// One code system: what its capability statement declares, and the published
/// resource beside it.
///
/// The row is the one whose version declares the direct-child operator, so
/// the shot lands on a system with a hierarchy to browse afterwards. The
/// canonical the screen heads with is returned: it is the one the
/// translate runner is given further down, and reading it here keeps the pass
/// from naming a code system of its own.
async fn code_system(journey: &Journey, dir: &Path) -> WebDriverResult<String> {
    journey
        .element(
            By::XPath(WALKABLE_ROW),
            "a code system whose version declares the direct-child operator",
        )
        .await
        .click()
        .await?;
    journey
        .element(By::Css(CAPABILITY_PANE), "the capability pane")
        .await;
    journey
        .element(By::Css(PUBLISHED_PANE), "the published-resource pane")
        .await;
    shot(journey, &dir.join("code-system.png")).await?;
    let canonical = journey
        .element(By::Css("h1"), "the canonical the screen heads with")
        .await
        .text()
        .await?;
    Ok(canonical)
}

/// The concept browser: a search, one concept, and a level of its hierarchy.
///
/// Where the concept the search answers first is a leaf, the capture moves
/// onto one of its parents, because a tree with no row shows nothing of what
/// the screen is for.
async fn concept_browser(journey: &Journey, dir: &Path) -> WebDriverResult<()> {
    journey
        .element(By::Css(BROWSE_LINK), "the link into the concept browser")
        .await
        .click()
        .await?;
    journey
        .element(By::Css(SEARCH_RESULT), "a concept the search answered")
        .await
        .click()
        .await?;
    journey
        .address_carrying("code=", "the address to carry the concept that was read")
        .await;
    journey
        .element(By::Css(TREE_SETTLED), "the first level of the tree")
        .await;
    if journey.count(By::Css(TREE_ROW)).await == 0 {
        journey
            .element(By::Css(PARENT_LINK), "a parent of this leaf concept")
            .await
            .click()
            .await?;
        journey
            .element(By::Css(TREE_ROW), "a tree row below the parent")
            .await;
    }
    journey
        .element(By::Css(CONCEPT_DISPLAY), "the concept the screen read")
        .await;
    shot(journey, &dir.join("browse.png")).await?;
    Ok(())
}

/// The value sets this root publishes, as the search answered them.
///
/// The canonical of the first row is returned, so the runner below expands a
/// value set this deployment actually holds without the pass naming one.
async fn value_sets(journey: &Journey, dir: &Path, base: &str) -> WebDriverResult<String> {
    journey.reopen(&address(base, "valuesets")).await;
    journey
        .element(By::Css(VALUE_SET_ROW), "a published value set")
        .await;
    shot(journey, &dir.join("value-sets.png")).await?;
    // The row leads with the name and carries the canonical beside it, which
    // is the one the runner then expands.
    let canonical = canonical_of(
        &journey
            .element(
                By::Css(VALUE_SET_CANONICAL),
                "the canonical the row carries",
            )
            .await
            .text()
            .await?,
    );
    assert!(
        !canonical.is_empty(),
        "the row carries the canonical the runner then expands"
    );
    Ok(canonical)
}

/// The canonical a listed row carries, which is its first token.
///
/// A canonical carries no whitespace, and the row appends a sentence for a
/// screen reader when the resource it lists carries no id to open it by.
fn canonical_of(text: &str) -> String {
    text.split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(',')
        .to_owned()
}

/// The expansion runner, on the value set the previous screen listed.
///
/// The canonical is typed into the form rather than put in the address, so the
/// shot shows the runner the way a reader drives it. A screenshot is the
/// viewport, so the answer is scrolled into view before it is taken.
async fn expansion_runner(
    journey: &Journey,
    dir: &Path,
    base: &str,
    canonical: &str,
) -> WebDriverResult<()> {
    journey.reopen(&address(base, "expand")).await;
    journey
        .element(By::Css(EXPAND_URL), "the runner's canonical control")
        .await
        .send_keys(canonical)
        .await?;
    journey
        .element(By::Css(EXPAND_SUBMIT), "the runner's submit control")
        .await
        .click()
        .await?;
    journey
        .element(By::Css(EXPANDED_ROW), "an expanded concept")
        .await;
    journey
        .element(By::Css(EXPANSION), "the answer this run got")
        .await;
    shot(journey, &dir.join("expand.png")).await?;
    Ok(())
}

/// The command bar, and what this root can do with a canonical.
///
/// The bar is driven rather than the address typed, because the shot is of the
/// bar doing its job. The subject is a canonical this deployment serves, taken
/// from the overview, so the pass names no code system.
async fn find(journey: &Journey, dir: &Path, base: &str, system: &str) -> WebDriverResult<()> {
    journey.reopen(&address(base, "")).await;
    journey
        .element(By::Css(COMMAND_BAR), "the command bar")
        .await
        .send_keys(system)
        .await?;
    journey
        .element(By::Css(COMMAND_SUBMIT), "the command bar's submit control")
        .await
        .click()
        .await?;
    journey
        .element(By::Css(OFFER), "an offer this root can answer")
        .await;
    shot(journey, &dir.join("find.png")).await?;
    Ok(())
}

/// The concept maps this root publishes.
async fn concept_maps(journey: &Journey, dir: &Path, base: &str) -> WebDriverResult<()> {
    journey.reopen(&address(base, "conceptmaps")).await;
    journey
        .element(By::Css(CONCEPT_MAPS), "what this root publishes")
        .await;
    shot(journey, &dir.join("concept-maps.png")).await?;
    Ok(())
}

/// One code translated through the maps this root holds.
///
/// The map is left unnamed, which asks the server to translate through every
/// map it holds for the code, so the shot is what a run answers rather than
/// what one map says.
async fn translate(journey: &Journey, dir: &Path, base: &str, system: &str) -> WebDriverResult<()> {
    journey.reopen(&address(base, "translate")).await;
    journey
        .element(
            By::Css(TRANSLATE_SYSTEM),
            "the runner's code system control",
        )
        .await
        .send_keys(system)
        .await?;
    journey
        .element(By::Css(TRANSLATE_CODE), "the runner's code control")
        .await
        .send_keys(MAPPED_CODE)
        .await?;
    journey
        .element(By::Css(TRANSLATE_SUBMIT), "the runner's submit control")
        .await
        .click()
        .await?;
    journey
        .text_becoming(
            By::Css(TRANSLATE_ANSWER),
            StringMatch::new(TRANSLATED.to_owned()).partial(),
            "the runner to answer the code it was given",
        )
        .await;
    shot(journey, &dir.join("translate.png")).await?;
    Ok(())
}

/// The validate and subsume runners, on a code the fixture holds.
///
/// The code system and the code are the ones the earlier screens already
/// named, so the pass names no code system of its own. The subsumption runner
/// below is left unrun: it is a second form on the same screen, and a shot of
/// one answer and one empty form shows what the screen offers without claiming
/// a result nobody asked for.
async fn validate_runner(
    journey: &Journey,
    dir: &Path,
    base: &str,
    system: &str,
) -> WebDriverResult<()> {
    journey.reopen(&address(base, "validate")).await;
    journey
        .element(By::Css(VALIDATE_SYSTEM), "the runner's code system control")
        .await
        .send_keys(system)
        .await?;
    journey
        .element(By::Css(VALIDATE_CODE), "the runner's code control")
        .await
        .send_keys(MAPPED_CODE)
        .await?;
    journey
        .element(By::Css(VALIDATE_SUBMIT), "the runner's submit control")
        .await
        .click()
        .await?;
    journey
        .text_becoming(
            By::Css(VALIDATE_ANSWER),
            StringMatch::new(VALIDATED.to_owned()).partial(),
            "the runner to state a verdict on the code it was given",
        )
        .await;
    journey
        .element(By::Css(SUBSUMES_SECTION), "the subsumption runner below it")
        .await;
    shot(journey, &dir.join("validate.png")).await?;
    Ok(())
}

/// The four served roots side by side, as their capability statements differ.
async fn versions(journey: &Journey, dir: &Path, base: &str) -> WebDriverResult<()> {
    journey.reopen(&address(base, "versions")).await;
    journey
        .element(By::Css(COMPARISON), "the comparison the four reads fill")
        .await;
    journey
        .count_becoming(
            By::Css(COMPARISON_TABLE),
            COMPARISON_TABLES,
            "both comparison tables to draw before the shot is taken",
        )
        .await;
    shot(journey, &dir.join("versions.png")).await
}

/// The evidence this build carries: the suite, the latency bars, and the
/// newest committed benchmark run.
async fn evidence(journey: &Journey, dir: &Path, base: &str) -> WebDriverResult<()> {
    journey.reopen(&address(base, "evidence")).await;
    journey
        .element(By::Css(CONFORMANCE_PANE), "the conformance pane")
        .await;
    journey
        .element(By::Css(LATENCY_PANE), "the latency pane")
        .await;
    journey
        .element(By::Css(RUN_PANE), "the benchmark run pane")
        .await;
    shot(journey, &dir.join("evidence.png")).await
}

/// Settings: what this browser remembers, and nothing the server holds.
async fn settings(journey: &Journey, dir: &Path, base: &str) -> WebDriverResult<()> {
    journey.reopen(&address(base, "settings")).await;
    journey
        .element(By::Css(THEME_CONTROL), "the theme control")
        .await;
    shot(journey, &dir.join("settings.png")).await
}

/// Captures one canonical image per viewer screen, in reading order.
///
/// One session takes every shot, so the images agree with each other. Each
/// screen is reached the way a reader reaches it, and every wait is on the
/// content the shot is of, so an image is never a picture of a screen that had
/// not answered yet.
#[tokio::test]
async fn the_documentation_screenshots_are_captured() {
    let Some(base) = server() else {
        return;
    };
    if std::env::var_os(GATE_ENV).is_none() {
        println!(
            "skipped: {GATE_ENV} is unset, so no tracked image is rewritten. \
             scripts/ui-e2e.sh sets it when it is asked for the capture pass."
        );
        return;
    }
    let dir = shots_dir();
    println!("capturing into {}", dir.display());
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey =
                Journey::open(driver, &base, &format!("/ui?fhir={CAPTURED_VERSION}")).await;
            overview(&journey, &dir).await?;
            let system = code_system(&journey, &dir).await?;
            concept_browser(&journey, &dir).await?;
            let canonical = value_sets(&journey, &dir, &base).await?;
            expansion_runner(&journey, &dir, &base, &canonical).await?;
            validate_runner(&journey, &dir, &base, &system).await?;
            find(&journey, &dir, &base, &system).await?;
            concept_maps(&journey, &dir, &base).await?;
            translate(&journey, &dir, &base, &system).await?;
            versions(&journey, &dir, &base).await?;
            evidence(&journey, &dir, &base).await?;
            settings(&journey, &dir, &base).await?;
            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the capture pass ran and the browser session ended cleanly");
}
