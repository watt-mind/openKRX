#![no_main]

use libfuzzer_sys::fuzz_target;
use openkrx_core::{MetadataLimits, metadata};

// The whole assertion is "no panic": the parser may accept or refuse any byte
// string, but it must never unwind.
fuzz_target!(|data: &[u8]| {
    let _ = metadata::parse(data, &MetadataLimits::DEFAULT);
});
