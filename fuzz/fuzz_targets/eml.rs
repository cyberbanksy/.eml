//! Parse arbitrary bytes as an email and build the page doteml would render.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let eml = doteml::parse_eml_bytes(data);
    let _ = doteml::html::build_html(&eml, false);
    let _ = doteml::html::build_html(&eml, true);
});
