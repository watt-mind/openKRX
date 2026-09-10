#![no_main]

use libfuzzer_sys::fuzz_target;
use openkrx_core::{Limits, MetadataLimits, archive, profile};

// The whole assertion is "no panic": an inventory the reader accepted may
// produce any structural report, but running the checks over it must never
// unwind. `profile::check` re-reads the marker entry and the metadata document
// out of the inventory, so this target reaches the decoder a second time on
// input the inventory target only listed.
fuzz_target!(|data: &[u8]| {
    if let Ok(inventory) = archive::inventory(data, &Limits::DEFAULT) {
        let _ = profile::check(&inventory, &MetadataLimits::DEFAULT);
    }
});
