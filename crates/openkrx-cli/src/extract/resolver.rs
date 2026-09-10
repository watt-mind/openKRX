//! How this run turns a plan's path components into a created file.
//!
//! The writer decides *what* to create and in which order; this module is the
//! one place that decides *how* a path is resolved, and it is the only place
//! either platform arm appears. There are two arms.
//!
//! - **Beneath a directory descriptor**, on Linux with `openat2(2)`. The
//!   destination is opened once, and every path after that is resolved by the
//!   kernel relative to that descriptor under `RESOLVE_BENEATH`,
//!   `RESOLVE_NO_SYMLINKS` and `RESOLVE_NO_MAGICLINKS`. A component swapped
//!   for a symbolic link after preflight is refused at the moment of the
//!   operation rather than followed. See [`super::linux_fd`].
//! - **Portable**, everywhere else, and on a Linux kernel that has no
//!   `openat2`. `create_dir` and `OpenOptions::create_new` against a path,
//!   with `symlink_metadata` before and after, which is what openKRX has
//!   always done: it defends against what is already at the destination, and
//!   not against a principal writing to it concurrently.
//!
//! Both arms answer with the same `output.*` codes, and neither carries a
//! path into a diagnostic. Which arm ran is reported once, as
//! `path_resolution_fallback` in the `extract` result: `true` only when this
//! run asked the kernel for the stronger resolution and could not have it.

use std::fs::{File, OpenOptions};
use std::path::Path;

use super::MARKER_NAME;
use super::preflight::{is_link, join};
use crate::exit::{
    Failure, OUTPUT_IO, OUTPUT_NOT_A_DIRECTORY, OUTPUT_PARTIAL_MARKER_PRESENT,
    OUTPUT_SYMLINK_IN_PATH,
};

#[cfg(target_os = "linux")]
use super::linux_fd::{self, Dir};
#[cfg(target_os = "linux")]
use rustix::io::Errno;

/// What creating a planned directory found at its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directory {
    /// This run created it, so this run records it and may undo it.
    Created,
    /// It was already there, as a real directory. This run did not create it,
    /// so this run must never count it or remove it.
    AlreadyThere,
}

/// A directory creation that failed, and what it left behind.
///
/// The two facts are separate because a creation can succeed and only then be
/// found unacceptable: the portable arm creates the directory and reads it
/// back, and what it reads back may be a link someone put there in between.
/// The directory is this run's either way, so the ledger has to record it
/// before the refusal is returned — otherwise the undo pass would leave
/// behind the one thing this run did create.
#[derive(Debug)]
pub struct DirectoryFailure {
    /// The refusal, with its stable code.
    pub failure: Failure,
    /// Whether this run created the directory before the refusal. The caller
    /// records it in the ledger when this is `true`, and never otherwise: a
    /// directory that was already there is not this run's to remove.
    pub created: bool,
}

impl DirectoryFailure {
    /// A refusal that created nothing, which is every case but one.
    const fn untouched(failure: Failure) -> Self {
        Self {
            failure,
            created: false,
        }
    }
}

/// How this run resolves the paths it creates, for its whole lifetime.
///
/// It is decided once, after the destination has passed preflight, and never
/// re-decided: a run that started beneath a descriptor finishes there, and a
/// run that fell back says so in its report rather than mixing the two.
#[derive(Debug)]
pub enum Resolver {
    /// Linux with `openat2`: the destination, held open.
    #[cfg(target_os = "linux")]
    Beneath(Dir),
    /// Paths, checked and then created.
    Portable,
}

impl Resolver {
    /// Choose the arm for `destination`, which preflight has already accepted.
    ///
    /// On Linux this opens the destination as a descriptor and probes
    /// `openat2` once. A kernel without the call falls back to the portable
    /// arm exactly once, and the fallback is reported.
    ///
    /// # Errors
    ///
    /// `output.symlink_in_path`, `output.not_a_directory` or `output.io` when
    /// the destination cannot be opened as a real directory. That is a
    /// **refusal**, never a fallback: see [`Resolver::with_kernel_support`].
    #[cfg(target_os = "linux")]
    pub fn open(destination: &Path) -> Result<Self, Failure> {
        Self::with_kernel_support(destination, linux_fd::supported)
    }

    /// Elsewhere there is one arm, and nothing to choose between.
    ///
    /// # Errors
    ///
    /// Never. The result shape is the Linux one so that the caller is written
    /// once.
    #[cfg(not(target_os = "linux"))]
    pub const fn open(_destination: &Path) -> Result<Self, Failure> {
        Ok(Self::Portable)
    }

    /// The seam the fallback test drives: `supported` stands in for the probe.
    ///
    /// Separated from [`Resolver::open`] so that the `ENOSYS` branch — a
    /// kernel older than 5.6, or a seccomp filter hiding the call — is
    /// reachable from a unit test on a kernel that does have `openat2`,
    /// without an environment variable, a feature or a `cfg(test)` hook on
    /// the shipped path.
    ///
    /// **The two ways this can go wrong are not the same, and are not
    /// treated alike.** A kernel that cannot answer `openat2` is a weaker
    /// guarantee, so the run continues on the portable arm and says so. A
    /// destination that cannot be *opened* is a refusal: `open_root` asks for
    /// `O_DIRECTORY | O_NOFOLLOW`, so the failure means the destination is no
    /// longer the real directory preflight accepted — replaced by a symbolic
    /// link (`ELOOP`) or by something that is not a directory (`ENOTDIR`)
    /// in between. Falling back there would hand that very path to the
    /// portable arm, which joins and resolves straight through the
    /// replacement: the escape this module exists to close, reported as
    /// nothing worse than a flag.
    ///
    /// # Errors
    ///
    /// `output.symlink_in_path`, `output.not_a_directory` or `output.io`,
    /// through the same mapping every other resolution failure uses.
    #[cfg(target_os = "linux")]
    pub fn with_kernel_support(
        destination: &Path,
        supported: impl Fn(&Dir) -> bool,
    ) -> Result<Self, Failure> {
        let root = linux_fd::open_root(destination).map_err(|error| walked(&error))?;
        if supported(&root) {
            Ok(Self::Beneath(root))
        } else {
            Ok(Self::Portable)
        }
    }

    /// Whether this run asked for kernel-enforced resolution and did not get it.
    ///
    /// `false` on a platform that has no stronger mode to fall back *from*:
    /// nothing was given up there, so nothing is reported. It is the answer to
    /// "did this run resolve more weakly than it tried to?", not to "which
    /// platform is this?".
    #[cfg(target_os = "linux")]
    #[must_use]
    pub const fn fell_back(&self) -> bool {
        matches!(self, Self::Portable)
    }

    /// Elsewhere: no stronger mode was asked for, so none was fallen back from.
    #[cfg(not(target_os = "linux"))]
    #[must_use]
    pub const fn fell_back(&self) -> bool {
        false
    }

    /// Create the marker that says an extraction into this destination is running.
    ///
    /// # Errors
    ///
    /// `output.partial_marker_present` when another run got there first, and
    /// `output.io` when the destination refuses the file.
    pub fn marker(&self, destination: &Path) -> Result<(), Failure> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Beneath(root) => match linux_fd::create_file(root, MARKER_NAME) {
                Ok(_) => Ok(()),
                Err(Errno::EXIST) => Err(Failure::output(OUTPUT_PARTIAL_MARKER_PRESENT)),
                Err(_) => Err(Failure::output(OUTPUT_IO)),
            },
            Self::Portable => {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(destination.join(MARKER_NAME))
                    .map_err(|error| {
                        if error.kind() == std::io::ErrorKind::AlreadyExists {
                            Failure::output(OUTPUT_PARTIAL_MARKER_PRESENT)
                        } else {
                            Failure::output(OUTPUT_IO)
                        }
                    })?;
                Ok(())
            }
        }
    }

    /// Remove the marker, which is the last step of a successful run.
    ///
    /// # Errors
    ///
    /// `output.io`.
    pub fn remove_marker(&self, destination: &Path) -> Result<(), Failure> {
        let removed = match self {
            #[cfg(target_os = "linux")]
            Self::Beneath(root) => linux_fd::remove_file(root, MARKER_NAME).map_err(|_| ()),
            Self::Portable => std::fs::remove_file(destination.join(MARKER_NAME)).map_err(|_| ()),
        };
        removed.map_err(|()| Failure::output(OUTPUT_IO))
    }

    /// Create one planned directory, or accept the real directory already there.
    ///
    /// # Errors
    ///
    /// A [`DirectoryFailure`] carrying `output.symlink_in_path` when a
    /// component is a link or the resolution would leave the destination,
    /// `output.not_a_directory` when one is not a directory, and `output.io`
    /// for anything else — and saying whether this run had created the
    /// directory before the refusal, so that the caller can record it.
    pub fn directory(
        &self,
        destination: &Path,
        components: &[String],
    ) -> Result<Directory, DirectoryFailure> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Beneath(root) => beneath_directory(root, components),
            Self::Portable => portable_directory(destination, components),
        }
    }

    /// Create one planned file, exclusively, and hand back the open handle.
    ///
    /// # Errors
    ///
    /// `output.symlink_in_path` or `output.not_a_directory` from an ancestor
    /// that is no longer what preflight saw, and `output.io` from the creation
    /// itself; each carries `entry`.
    pub fn file(
        &self,
        destination: &Path,
        components: &[String],
        entry: u32,
    ) -> Result<File, Failure> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Beneath(root) => beneath_file(root, components, entry),
            Self::Portable => OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(join(destination, components))
                .map_err(|_| Failure::output_at(OUTPUT_IO, entry)),
        }
    }
}

/// The portable arm: create it, then read back what is there.
///
/// The re-read costs one `lstat` per planned directory and is the writer's own
/// confirmation that it is about to descend into a real directory rather than
/// through a link. It closes nothing against a concurrent writer — that is
/// what the other arm is for — but it does catch what preflight could not have
/// seen because it was not yet there.
fn portable_directory(
    destination: &Path,
    components: &[String],
) -> Result<Directory, DirectoryFailure> {
    let path = join(destination, components);
    let outcome = match std::fs::create_dir(&path) {
        Ok(()) => Directory::Created,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Directory::AlreadyThere,
        Err(_) => {
            return Err(DirectoryFailure::untouched(Failure::output(OUTPUT_IO)));
        }
    };
    // Everything from here on may refuse a directory this run has already
    // created, so each refusal carries that fact rather than dropping it.
    let created = outcome == Directory::Created;
    let refuse = |code: &'static str| DirectoryFailure {
        failure: Failure::output(code),
        created,
    };
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| refuse(OUTPUT_IO))?;
    if is_link(&metadata) {
        return Err(refuse(OUTPUT_SYMLINK_IN_PATH));
    }
    if !metadata.is_dir() {
        return Err(refuse(OUTPUT_NOT_A_DIRECTORY));
    }
    Ok(outcome)
}

/// The Linux arm: resolve the parent, then one name inside it.
#[cfg(target_os = "linux")]
fn beneath_directory(root: &Dir, components: &[String]) -> Result<Directory, DirectoryFailure> {
    let (leaf, ancestors) = split(components);
    let parent = linux_fd::resolve(root, ancestors)
        .map_err(|error| DirectoryFailure::untouched(walked(&error)))?;
    // Nothing below can refuse a directory this arm created: `mkdirat` either
    // makes it, in which case the kernel resolves it at the next step, or it
    // does not, in which case this run created nothing to record.
    match linux_fd::create_dir(&parent, leaf) {
        Ok(()) => Ok(Directory::Created),
        // Something is already there, and `mkdirat` never followed a link to
        // decide that. Preflight accepted it as a real directory; the kernel
        // is asked again, now, and answers `ELOOP` for a link and `ENOTDIR`
        // for anything that is not a directory.
        Err(Errno::EXIST) => linux_fd::resolve(root, components)
            .map(|_| Directory::AlreadyThere)
            .map_err(|error| DirectoryFailure::untouched(walked(&error))),
        Err(_) => Err(DirectoryFailure::untouched(Failure::output(OUTPUT_IO))),
    }
}

/// The Linux arm for a file: resolve the parent, then create one name in it.
#[cfg(target_os = "linux")]
fn beneath_file(root: &Dir, components: &[String], entry: u32) -> Result<File, Failure> {
    let (leaf, ancestors) = split(components);
    let parent = linux_fd::resolve(root, ancestors)
        .map_err(|error| Failure::output_at(code(&error), entry))?;
    linux_fd::create_file(&parent, leaf).map_err(|_| Failure::output_at(OUTPUT_IO, entry))
}

/// A planned path's last component, and the ancestors leading to it.
///
/// The planner never produces an empty path, so the split always succeeds.
#[cfg(target_os = "linux")]
fn split(components: &[String]) -> (&str, &[String]) {
    let (leaf, ancestors) = components
        .split_last()
        .expect("a planned path has at least one component");
    (leaf.as_str(), ancestors)
}

/// The refusal an `openat2` failure on the way to a path becomes.
#[cfg(target_os = "linux")]
fn walked(error: &Errno) -> Failure {
    Failure::output(code(error))
}

/// The stable code for a failure the kernel reported while resolving.
///
/// `EXDEV` is `RESOLVE_BENEATH` refusing a resolution that would have left the
/// destination. The planner has already refused `..`, an absolute name and a
/// separator, so the only way a planned component can reach for the outside is
/// through a link someone put there, and it is reported as one.
#[cfg(target_os = "linux")]
const fn code(error: &Errno) -> &'static str {
    match *error {
        Errno::LOOP | Errno::XDEV => OUTPUT_SYMLINK_IN_PATH,
        Errno::NOTDIR => OUTPUT_NOT_A_DIRECTORY,
        _ => OUTPUT_IO,
    }
}
