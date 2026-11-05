//! Look for an embedded .eml in arbitrary PDF bytes, as `doteml verify` does.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = lopdf::Document::load_mem(data) {
        let _ = doteml::verify::extract_embedded_eml(&doc);
    }
});
