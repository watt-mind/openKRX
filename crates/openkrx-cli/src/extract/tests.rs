//! Unix: the check-to-create window, exercised rather than described.
//!
//! Every other test of the output layer is a subprocess test in
//! `tests/extract.rs`, because the contract worth testing is the observable
//! one. These two cannot be: the thing under test is what happens *inside* one
//! run, between the moment preflight accepted a destination and the moment the
//! first file is created, and no subprocess can be interrupted there.
//!
//! [`super::run_between`] is the seam, and it is an ordinary private function
//! of this module: a closure called once in that window, and the chooser for
//! the path-resolution arm. `super::run` passes a closure that does nothing
//! and the real chooser, so what these tests drive is the shipped code with no
//! `cfg(test)` branch and no feature in it.
//!
//! They are `cfg(unix)`, because the arm under test is: the destination is
//! held open and every path resolved from it on every Unix target, by the
//! kernel where `openat2` is available and by the component walk elsewhere.
//! The two that drive the `openat2` probe are Linux-only, because no other
//! target has a stronger mode to fall back from. The portable arm's own rules
//! are held by the subprocess tests on all three platforms.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use openkrx_core::synthetic::{Archive, Entry};
use openkrx_core::{Limits, archive};

use super::resolver::Resolver;
use crate::exit::{OUTPUT_NOT_A_DIRECTORY, OUTPUT_SYMLINK_IN_PATH};

/// Distinguishes two scratch directories created in the same process.
static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A directory this test owns, removed when it is dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// Create a uniquely named directory under the temporary root.
    fn new(label: &str) -> Self {
        let sequence = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let process = std::process::id();
        let path = std::env::temp_dir().join(format!("openkrx-{label}-{process}-{sequence}"));
        std::fs::create_dir(&path).expect("create a scratch directory of this test's own");
        Self(path)
    }

    /// A subdirectory of it, created.
    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir(&path).expect("create a directory");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One package holding one file one directory deep, built in memory.
///
/// Two components are the minimum that has an ancestor to swap: the run
/// creates `payload/`, and the file goes inside it.
fn package() -> Vec<u8> {
    Archive::of(vec![Entry::stored(
        b"payload/inner.bin",
        b"synthetic bytes",
    )])
    .build()
}

/// One package holding one file directly in the destination.
///
/// The plan it produces has no directory in it at all, so a run that fails at
/// the file has created exactly one thing — the marker — and the undo pass's
/// counts are unambiguous.
fn flat_package() -> Vec<u8> {
    Archive::of(vec![Entry::stored(b"inner.bin", b"synthetic bytes")]).build()
}

/// Extract `package()` into `destination`, running `between` in the window.
fn extract(
    destination: &Path,
    between: &mut dyn FnMut(),
    resolver: impl Fn(&Path) -> Result<Resolver, crate::exit::Failure>,
) -> Result<crate::commands::extract::ExtractData, super::Refusal> {
    extract_package(&package(), destination, between, resolver)
}

/// The same, for a package the caller built itself.
fn extract_package(
    bytes: &[u8],
    destination: &Path,
    between: &mut dyn FnMut(),
    resolver: impl Fn(&Path) -> Result<Resolver, crate::exit::Failure>,
) -> Result<crate::commands::extract::ExtractData, super::Refusal> {
    let inventory = archive::inventory(bytes, &Limits::DEFAULT).expect("a readable archive");
    super::run_between(&inventory, destination, between, resolver)
}

#[test]
fn an_ancestor_replaced_after_preflight_is_refused_rather_than_followed() {
    let scratch = Scratch::new("race");
    let destination = scratch.dir("destination");
    let outside = scratch.dir("outside");

    // The window: the run has created `destination/payload` and is about to
    // create `destination/payload/inner.bin` inside it. A principal with write
    // access to the destination removes that directory and puts a symbolic
    // link to a directory of its own in its place. Without kernel-enforced
    // resolution the next `create_new` follows the link and writes outside the
    // destination the caller named.
    let planted = destination.join("payload");
    let target = outside.clone();
    let mut between = move || {
        std::fs::remove_dir(&planted).expect("remove the directory the run created");
        std::os::unix::fs::symlink(&target, &planted).expect("plant a symbolic link");
    };

    let refusal = extract(&destination, &mut between, Resolver::open)
        .expect_err("a swapped ancestor is refused");
    assert_eq!(
        refusal.failure.code, OUTPUT_SYMLINK_IN_PATH,
        "the kernel refused to resolve through the link, under the code the \
path rule already has"
    );
    assert!(
        tree_is_empty(&outside),
        "nothing was written through the link, outside the destination"
    );
    assert!(
        !destination.join(super::MARKER_NAME).exists(),
        "the undo pass removed this run's marker"
    );
}

#[test]
fn a_leaf_replaced_by_a_directory_after_preflight_is_refused() {
    let scratch = Scratch::new("leaf");
    let destination = scratch.dir("destination");

    // The same window, with the leaf itself taken: exclusive creation refuses
    // it on every platform, and the point here is that the Linux arm answers
    // with a refusal rather than an escape.
    let planted = destination.join("payload").join("inner.bin");
    let mut between = move || {
        std::fs::create_dir(&planted).expect("take the leaf path");
    };

    let refusal =
        extract(&destination, &mut between, Resolver::open).expect_err("a taken leaf is refused");
    assert_eq!(
        refusal.failure.category,
        crate::exit::Category::Output,
        "an output refusal, not a decode failure"
    );
}

#[test]
fn a_destination_replaced_before_the_resolver_opens_it_is_refused_not_fallen_back() {
    let scratch = Scratch::new("destination-race");
    let destination = scratch.dir("destination");
    let outside = scratch.dir("outside");

    // The other window: preflight has just accepted `destination` as a real
    // directory, and it is replaced by a symbolic link to a directory of
    // someone else's before the resolver opens it. The resolver's own chooser
    // is the seam — it runs at exactly that moment — so the swap happens
    // inside it, and `open_root` then meets the link rather than the
    // directory preflight saw. `O_DIRECTORY | O_NOFOLLOW` makes that an
    // `ELOOP` rather than an open through the link.
    //
    // The regression this pins is the *handling* of that failure. Treating it
    // as "no descriptor, carry on portably" would send the run to the arm
    // that joins the destination path onto every planned component and
    // resolves it by name — straight through the link that had just been
    // planted — and report nothing worse than a fallback flag. It is a
    // refusal, and preflight's own `output.destination_symlink` is not
    // reachable from here because preflight had already passed.
    let swap = |path: &Path| {
        std::fs::remove_dir(path).expect("remove the real destination");
        std::os::unix::fs::symlink(&outside, path).expect("plant a symbolic link");
        Resolver::open(path)
    };

    let refusal =
        extract(&destination, &mut || (), swap).expect_err("a replaced destination is refused");
    // Which of the two path codes the kernel's answer maps onto is the
    // kernel's business: `O_DIRECTORY` and `O_NOFOLLOW` both refuse a symbolic
    // link here, and Linux reports whichever check it reaches first —
    // `ENOTDIR` in practice, `ELOOP` where the link check wins. Both are
    // codes the path rule already has, both exit 9, and neither names a path.
    // What this test holds is that it is a refusal at all.
    assert!(
        matches!(
            refusal.failure.code,
            OUTPUT_NOT_A_DIRECTORY | OUTPUT_SYMLINK_IN_PATH
        ),
        "a replaced destination is refused under a path code, not reported as \
a fallback; got {}",
        refusal.failure.code
    );
    assert_eq!(refusal.failure.category, crate::exit::Category::Output);
    assert!(
        tree_is_empty(&outside),
        "nothing at all was written through the link"
    );
}

#[test]
fn the_undo_pass_removes_through_the_descriptor_and_not_through_the_name() {
    let scratch = Scratch::new("undo");
    let destination = scratch.dir("destination");
    let decoy = scratch.dir("decoy");
    let moved = scratch.0.join("moved");

    // The decoy is dressed as a destination a run is busy with: a marker and
    // a directory, neither of them this run's. If the undo pass resolved its
    // own paths by name it would arrive here — after the swap below — and
    // remove exactly these two.
    std::fs::write(decoy.join(super::MARKER_NAME), b"not this run's")
        .expect("dress the decoy with a marker");
    std::fs::create_dir(decoy.join("payload")).expect("dress the decoy with a directory");

    // The window: the run holds the destination open and has created its
    // marker inside it. A principal with write access takes the leaf, so the
    // write is about to fail and the undo pass is about to run — and in the
    // same moment moves the real destination aside and leaves a symbolic link
    // to the decoy standing at its name.
    let swapped = destination.clone();
    let aside = moved.clone();
    let target = decoy.clone();
    let mut between = move || {
        std::fs::create_dir(swapped.join("inner.bin")).expect("take the leaf");
        std::fs::rename(&swapped, &aside).expect("move the real destination aside");
        std::os::unix::fs::symlink(&target, &swapped).expect("plant a link at its name");
    };

    let refusal = extract_package(&flat_package(), &destination, &mut between, Resolver::open)
        .expect_err("a taken leaf is refused");
    assert_eq!(
        refusal.cleanup.removed, 1,
        "the marker this run created was removed"
    );
    assert_eq!(
        refusal.cleanup.left_in_place, 0,
        "and nothing was left over"
    );
    assert!(
        !moved.join(super::MARKER_NAME).exists(),
        "removed from the directory this run actually wrote into, which the \
descriptor still names after the rename"
    );
    assert!(
        decoy.join(super::MARKER_NAME).exists() && decoy.join("payload").is_dir(),
        "and not from whatever now answers to the destination's name, where a \
removal by path would have gone"
    );
    assert!(
        moved.join("inner.bin").is_dir(),
        "what this run did not create is still there: the undo pass removes \
its own records and nothing else"
    );
}

/// Whether `directory` holds no entry, which is how each race test says
/// "nothing was written here" without naming what it expected.
fn tree_is_empty(directory: &Path) -> bool {
    std::fs::read_dir(directory)
        .expect("read the directory back")
        .next()
        .is_none()
}

#[cfg(target_os = "linux")]
#[test]
fn a_kernel_without_openat2_falls_back_once_and_reports_it() {
    let scratch = Scratch::new("fallback");
    let destination = scratch.dir("destination");

    // The `ENOSYS` branch: a kernel before 5.6, or a seccomp filter that hides
    // the call. The probe answers "no", the run takes the portable arm, and
    // the report says so instead of leaving the caller to guess from a kernel
    // version.
    let data = extract(&destination, &mut || (), |path| {
        Resolver::with_kernel_support(path, |_| false)
    })
    .expect("the portable path still extracts");

    assert!(
        data.path_resolution_fallback,
        "the run says it resolved more weakly than it asked to"
    );
    assert_eq!(data.files_written, 1);
    assert_eq!(data.directories_created, 1);
    assert!(data.marker_removed);
    assert_eq!(
        std::fs::read(destination.join("payload").join("inner.bin"))
            .expect("the file the run wrote"),
        b"synthetic bytes",
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_kernel_with_openat2_reports_no_fallback() {
    let scratch = Scratch::new("beneath");
    let destination = scratch.dir("destination");

    let data = extract(&destination, &mut || (), Resolver::open).expect("an ordinary extraction");
    assert!(
        !data.path_resolution_fallback,
        "a supported kernel gave up nothing, so the report says nothing was \
given up; if this fails, the runner's kernel has no openat2"
    );
    assert_eq!(data.files_written, 1);
}

/// The same statement for a Unix that has no `openat2` to fall back from:
/// the walk is not a fallback, and a run that took it says nothing was given
/// up. Linux has its own pair of probe tests for the other half of this.
#[cfg(not(target_os = "linux"))]
#[test]
fn the_component_walk_is_not_reported_as_a_fallback() {
    let scratch = Scratch::new("walk");
    let destination = scratch.dir("destination");

    let data = extract(&destination, &mut || (), Resolver::open).expect("an ordinary extraction");
    assert!(
        !data.path_resolution_fallback,
        "the walk resolves beneath the destination descriptor, so the run \
gave nothing up and reports nothing given up"
    );
    assert_eq!(data.files_written, 1);
    assert_eq!(data.directories_created, 1);
    assert!(data.marker_removed);
}

#[test]
fn a_directory_component_that_is_a_file_is_still_not_a_directory() {
    let scratch = Scratch::new("notdir");
    let destination = scratch.dir("destination");

    // `ENOTDIR` from the kernel maps onto the code the portable arm produces
    // from its own `symlink_metadata` read, so the two arms answer alike.
    let planted = destination.join("payload");
    let mut between = move || {
        std::fs::remove_dir(&planted).expect("remove the directory the run created");
        std::fs::write(&planted, b"in the way").expect("put a file in its place");
    };

    let refusal = extract(&destination, &mut between, Resolver::open)
        .expect_err("a file ancestor is refused");
    assert_eq!(refusal.failure.code, OUTPUT_NOT_A_DIRECTORY);
}
