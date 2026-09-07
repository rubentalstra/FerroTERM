// SPDX-License-Identifier: BUSL-1.1
//! The browser session every journey drives, and the waits it is built from.

use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use thirtyfour::LoggingPrefsLogLevel;
use thirtyfour::prelude::*;
use thirtyfour::stringmatch::Needle;

/// Names the server under test, as a URL without a trailing slash.
pub const BASE_URL_ENV: &str = "FERROTERM_UI_E2E_BASE_URL";

/// Names the WebDriver endpoint the journeys drive the browser through.
pub const WEBDRIVER_ENV: &str = "FERROTERM_UI_E2E_WEBDRIVER";

/// The WebDriver endpoint used when [`WEBDRIVER_ENV`] is unset.
const DEFAULT_WEBDRIVER: &str = "http://127.0.0.1:4444";

/// How long a wait keeps trying before the journey fails.
const WAIT: Duration = Duration::from_secs(30);

/// How often a wait re-reads the page while it waits.
const POLL: Duration = Duration::from_millis(100);

/// The level chromedriver reports for `console.error` and for an uncaught
/// panic, which `console_error_panic_hook` turns into one.
const SEVERE: &str = "SEVERE";

/// The one severe entry a journey passes over.
///
// NOTE: A browser requests the site icon on its own, so a document that asks
// for nothing still draws the probe and its 404
// (<https://html.spec.whatwg.org/multipage/links.html#rel-icon>).
const UNPROMPTED: &str = "/favicon.ico";

/// The flags the browser is started with.
///
/// `--headless=new` is Chrome's current headless mode; the other two are what
/// a browser inside a container needs, because the kernel sandbox and the
/// default 64 MB `/dev/shm` are both unavailable there
/// (<https://developer.chrome.com/docs/chromium/headless>).
const CHROME_ARGS: [&str; 4] = [
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--window-size=1280,900",
];

/// The server under test, or `None` when nothing names one.
pub fn server() -> Option<String> {
    match std::env::var(BASE_URL_ENV) {
        Ok(base) => Some(base.trim_end_matches('/').to_owned()),
        Err(_absent) => {
            println!(
                "skipped: {BASE_URL_ENV} is unset, so there is no served viewer to drive. \
                 scripts/ui-e2e.sh sets it, and the ui-e2e CI job always runs that."
            );
            None
        }
    }
}

/// A browser session on the configured WebDriver endpoint.
pub async fn session() -> WebDriver {
    let endpoint =
        std::env::var(WEBDRIVER_ENV).unwrap_or_else(|_absent| DEFAULT_WEBDRIVER.to_owned());
    let mut capabilities = DesiredCapabilities::chrome();
    for argument in CHROME_ARGS {
        capabilities
            .add_arg(argument)
            .expect("chrome accepts a command-line argument");
    }
    // chromedriver records the browser log only for a session that asked for
    // it at creation, and that log is where an uncaught panic lands.
    capabilities
        .set_browser_log_level(LoggingPrefsLogLevel::All)
        .expect("chrome accepts a logging preference");
    WebDriver::new(&endpoint, capabilities)
        .await
        .unwrap_or_else(|error| panic!("no browser session at {endpoint}: {error}"))
}

/// One browser session pointed at the server under test.
#[derive(Debug)]
pub struct Journey {
    /// The session every step drives.
    driver: WebDriver,
}

impl Journey {
    /// Opens `path` on `base` and returns the journey over it.
    pub async fn open(driver: WebDriver, base: &str, path: &str) -> Self {
        let address = format!("{base}{path}");
        driver
            .goto(address.clone())
            .await
            .unwrap_or_else(|error| panic!("the browser could not open {address}: {error}"));
        Self { driver }
    }

    /// Opens `address` in the same session, as a fresh load.
    ///
    /// A click navigation and a typed address reach the router differently, so
    /// a journey that only clicks never exercises the address a reader shares.
    pub async fn reopen(&self, address: &str) {
        self.driver
            .goto(address)
            .await
            .unwrap_or_else(|error| panic!("the browser could not open {address}: {error}"));
    }

    /// The first element matching `selector`, once the bundle has rendered it.
    ///
    /// The wait is the library's own poller over the element condition, so
    /// nothing here sleeps for a fixed time and hopes.
    pub async fn element(&self, selector: By, what: &str) -> WebElement {
        match self.driver.query(selector).wait(WAIT, POLL).first().await {
            Ok(element) => element,
            Err(error) => panic!("{}", self.failure(what, &error).await),
        }
    }

    /// The text of the element matching `selector` once `needle` matches it.
    ///
    /// Waiting on the text rather than on the element alone is what makes a
    /// re-render observable: the element is already on the page while it still
    /// carries the previous version's text.
    pub async fn text_becoming<N>(&self, selector: By, needle: N, what: &str) -> String
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let found = self
            .driver
            .query(selector)
            .wait(WAIT, POLL)
            .with_text(needle)
            .first()
            .await;
        match found {
            Ok(element) => element
                .text()
                .await
                .unwrap_or_else(|error| panic!("reading {what}: {error}")),
            Err(error) => panic!("{}", self.failure(what, &error).await),
        }
    }

    /// How many elements match `selector` right now.
    ///
    /// The count is a reading of the page as it stands, so a journey takes it
    /// only after a wait has proven the section it describes has settled.
    pub async fn count(&self, selector: By) -> usize {
        self.driver
            .find_all(selector)
            .await
            .map(|found| found.len())
            .unwrap_or_default()
    }

    /// The address the browser is on.
    pub async fn address(&self) -> String {
        self.driver
            .current_url()
            .await
            .expect("the browser reports the address it is on")
            .to_string()
    }

    /// The address, once it carries `needle`.
    ///
    /// A key press that navigates does so a paint after the press, so an
    /// address read straight afterwards is the one before it. The wait polls
    /// the same way the element waits do, and reports what the browser logged
    /// when the address never arrives.
    pub async fn address_carrying(&self, needle: &str, what: &str) -> String {
        let deadline = Instant::now() + WAIT;
        loop {
            let address = self.address().await;
            if address.contains(needle) {
                return address;
            }
            if Instant::now() >= deadline {
                let console = self.console_errors().await;
                let logged = if console.is_empty() {
                    "the browser logged nothing severe".to_owned()
                } else {
                    format!("the browser logged:\n  {}", console.join("\n  "))
                };
                panic!(
                    "waiting for {what} failed: the address is still `{address}`, \
                     which carries no `{needle}`\n{logged}"
                );
            }
            tokio::time::sleep(POLL).await;
        }
    }

    /// Everything the browser logged as severe since the last read.
    ///
    /// Each read drains the buffer, so a journey reads it only where it
    /// reports it. A failed request stays in scope: an asset or a code the
    /// viewer could not fetch is exactly the defect a rendering test is here
    /// to catch, so only the browser's own icon probe is passed over.
    async fn console_errors(&self) -> Vec<String> {
        match self.driver.browser_log().await {
            Ok(entries) => entries
                .iter()
                .filter(|entry| entry.level == SEVERE && !entry.message.contains(UNPROMPTED))
                .map(|entry| {
                    let source = entry.source.as_deref().unwrap_or("unattributed");
                    format!("[{source}] {}", entry.message)
                })
                .collect(),
            Err(error) => vec![format!("the browser log could not be read: {error}")],
        }
    }

    /// The message for a wait that ran out, with what the browser logged.
    ///
    /// A boot failure shows up as an element that never appears, so the
    /// console goes into the message: without it the report says only that a
    /// selector did not match.
    async fn failure(&self, what: &str, error: &WebDriverError) -> String {
        let console = self.console_errors().await;
        let logged = if console.is_empty() {
            "the browser logged nothing severe".to_owned()
        } else {
            format!("the browser logged:\n  {}", console.join("\n  "))
        };
        format!("waiting for {what} failed: {error}\n{logged}")
    }

    /// Fails the journey when the browser logged anything severe.
    pub async fn no_console_errors(&self) {
        let console = self.console_errors().await;
        assert!(
            console.is_empty(),
            "the browser logged {} severe entries, and a rendered page that logs one is a defect:\n  {}",
            console.len(),
            console.join("\n  ")
        );
    }

    /// Sets the window, and with it the viewport a shot is taken of.
    ///
    /// The session starts at the size [`CHROME_ARGS`] fixes; a capture asks for
    /// a taller one so a whole screen lands in one image
    /// (<https://www.w3.org/TR/webdriver2/#set-window-rect>).
    pub async fn resize(&self, width: u32, height: u32) {
        self.driver
            .set_window_rect(0, 0, width, height)
            .await
            .unwrap_or_else(|error| panic!("resizing to {width}x{height}: {error}"));
    }

    /// How tall the rendered document is, in CSS pixels.
    ///
    /// The body's own box is the measure, read over WebDriver
    /// (<https://www.w3.org/TR/webdriver2/#get-element-rect>), so nothing here
    /// runs a script in the page.
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a WebDriver rect is f64 and `as` is the only defined float-to-integer conversion; it saturates at both ends, the value is rounded and floored at zero here, and the caller clamps it into the shot bounds"
    )]
    pub async fn page_height(&self) -> WebDriverResult<u32> {
        let body = self.driver.find(By::Css("body")).await?;
        Ok(body.rect().await?.height.round().max(0.0) as u32)
    }

    /// Writes a PNG of what the browser is showing to `path`.
    ///
    /// The image is the viewport, whose size the session fixes on the command
    /// line, so a capture taken twice is the same size twice. WebDriver hands
    /// the picture back as base64 over the wire
    /// (<https://www.w3.org/TR/webdriver2/#take-screenshot>), so the file is
    /// written where the journeys run rather than inside the browser.
    pub async fn shot(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("creating {}: {error}", parent.display()));
        }
        self.driver
            .screenshot(path)
            .await
            .unwrap_or_else(|error| panic!("writing {}: {error}", path.display()));
        println!("captured {}", path.display());
    }
}
