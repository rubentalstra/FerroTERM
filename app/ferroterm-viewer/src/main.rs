//! The FerroTERM viewer: a FHIR terminology browser that runs in the browser.
//!
//! The bundle is served by the FerroTERM server it queries, and every request
//! it makes is an ordinary FHIR request to that server's public API. It links
//! no crate from this workspace, so anything the viewer can do, a client can
//! do.

mod app;
mod components;
mod fhir;
mod pages;
mod paging;
mod routes;
mod settings;
mod storage;
mod theme;
mod url;

/// Installs the panic hook and mounts the application.
fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
