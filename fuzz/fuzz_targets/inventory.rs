#![no_main]

use libfuzzer_sys::fuzz_target;
use openkrx_core::{Limits, archive};

// The whole assertion is "no panic": the inventory may accept or refuse any
// byte string, but it must never unwind. Every accepted entry is decoded once
// more through `entry_bytes`, which is the only path that inflates data.
fuzz_target!(|data: &[u8]| {
    if let Ok(inventory) = archive::inventory(data, &Limits::DEFAULT) {
        for index in 0..inventory.len() {
            let _ = inventory.entry_bytes(index as u32);
        }
    }
});
