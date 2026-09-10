#![no_main]

use libfuzzer_sys::fuzz_target;
use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::{Limits, archive};

// Planning must not unwind, and it asserts one invariant of its own: a produced
// plan holds only destination components a caller can join blindly. That is the
// planner's documented promise — no empty, relative or separator-bearing
// component ever reaches a destination — and it is an invariant of this crate,
// not a conformance claim about the KRX format.
fuzz_target!(|data: &[u8]| {
    let Ok(inventory) = archive::inventory(data, &Limits::DEFAULT) else {
        return;
    };
    let Ok(plan) = extract::plan(&inventory, &ExtractLimits::DEFAULT) else {
        return;
    };
    for item in plan.items() {
        let components = item.components();
        assert!(
            !components.is_empty(),
            "a planned item names no destination at all"
        );
        assert!(
            !components[0].is_empty(),
            "a planned destination starts with an empty component, which is an absolute path"
        );
        for component in components {
            assert!(!component.is_empty(), "a planned component is empty");
            assert_ne!(
                component, ".",
                "a planned component is the current directory"
            );
            assert_ne!(
                component, "..",
                "a planned component is the parent directory"
            );
            assert!(
                !component.contains('/') && !component.contains('\\'),
                "a planned component carries a path separator"
            );
        }
    }
});
