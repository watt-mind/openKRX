//! Linux: every destination path resolved by the kernel, beneath one
//! descriptor.
//!
//! The portable writer checks a path and then creates it. Between those two
//! steps a principal with write access to the destination can replace a
//! component with a symbolic link, and the creation follows it out of the
//! destination the caller named. No portable standard-library call closes
//! that window: `symlink_metadata` answers a question about the past, and
//! `O_NOFOLLOW` covers only the last component.
//!
//! `openat2(2)`, which Linux 5.6 added, does close it. The destination is
//! opened **once**, before anything is written, and every path after that is
//! resolved relative to that descriptor with three resolve flags:
//!
//! - `RESOLVE_BENEATH` — the resolution may not leave the directory the
//!   descriptor names, so `..`, an absolute path and a mount crossing are
//!   refused by the kernel rather than checked for by this crate.
//! - `RESOLVE_NO_SYMLINKS` — no component may be a symbolic link, which is
//!   the check-to-create race itself: the kernel decides at the moment of the
//!   operation, not one syscall earlier.
//! - `RESOLVE_NO_MAGICLINKS` — nor may it be one of the kernel's own
//!   pseudo-links, such as an entry under `/proc/self/fd`.
//!
//! Only a resolved directory descriptor and a **single** name component are
//! ever handed to `mkdirat` or `openat`, and both refuse to follow a link at
//! that last component: `mkdir` fails with `EEXIST` over a symbolic link, and
//! `O_CREAT | O_EXCL` fails the same way. So no absolute path is re-resolved
//! after preflight, and no name from the package is ever pasted into one.
//!
//! This module holds the syscalls and nothing else. Which refusal each
//! failure becomes is [`super::resolver`]'s, so that the two platforms answer
//! with the same `output.*` codes. `rustix` is the only dependency, with
//! default features off so that its `linux_raw` backend is used and no C
//! library is linked; the workspace forbids `unsafe`, and nothing here needs
//! any.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags, mkdirat, open, openat, openat2, unlinkat};
use rustix::io::Errno;

/// The resolve flags every `openat2` in this module asks for.
///
/// They are asked for together, and a kernel that does not understand all
/// three is treated as a kernel without `openat2` at all: a partial guarantee
/// reported as the full one would be worse than the portable path, which at
/// least says what it is.
const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_MAGICLINKS);

/// `0o777`, as `create_dir` asks for it; the process umask reduces it.
const DIRECTORY_MODE: Mode = Mode::RWXU.union(Mode::RWXG).union(Mode::RWXO);

/// `0o666`, as `File::create` asks for it; the process umask reduces it.
const FILE_MODE: Mode = Mode::RUSR
    .union(Mode::WUSR)
    .union(Mode::RGRP)
    .union(Mode::WGRP)
    .union(Mode::ROTH)
    .union(Mode::WOTH);

/// An open directory: the only thing paths are resolved against.
///
/// Its descriptor is closed when it is dropped. It carries no path, and there
/// is deliberately no way to ask it for one: a path is what this module
/// exists to stop the process from using.
#[derive(Debug)]
pub struct Dir(OwnedFd);

/// Open `destination` itself, refusing to follow a link at it.
///
/// The one call in this module that takes a path from the caller rather than
/// a component from the plan, and the last moment the destination itself is
/// checked. `O_NOFOLLOW` with `O_DIRECTORY` means a destination that is a
/// symbolic link fails here with `ELOOP`, and one that is not a directory
/// with `ENOTDIR`, rather than being opened through. Preflight has already
/// refused both, so a failure here means the destination changed in between —
/// which is a refusal for the caller to report, never a reason to fall back
/// to resolving that same path by name.
///
/// A symbolic link is refused by both flags at once, and which error Linux
/// reports depends on which check its path resolution reaches first: in
/// practice `ENOTDIR`, from `O_DIRECTORY`, rather than the `ELOOP` that
/// `O_NOFOLLOW` alone would give. The caller maps both onto codes the path
/// rule already has, so nothing turns on the choice.
///
/// # Errors
///
/// Whatever `open(2)` reported.
pub fn open_root(destination: &Path) -> Result<Dir, Errno> {
    let flags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);
    open(destination, flags, Mode::empty()).map(Dir)
}

/// Whether this kernel answers `openat2` with the three resolve flags.
///
/// One probe, made once per run, resolving the destination onto itself. A
/// kernel before 5.6 answers `ENOSYS`; a seccomp filter that hides the call
/// answers `EPERM`; a kernel that has the call but not a flag answers
/// `EINVAL` or `E2BIG`. Any other failure is read the same way, because a
/// destination this run cannot even reopen by name is not one it should go on
/// resolving through — the caller falls back to the portable path and reports
/// that it did.
#[must_use]
pub fn supported(root: &Dir) -> bool {
    resolve(root, &[]).is_ok()
}

/// Resolve `components` beneath `root`, refusing every link on the way.
///
/// An empty slice reopens `root` itself, which is what a plan item directly
/// in the destination needs and is the shape the probe uses.
///
/// # Errors
///
/// `ELOOP` for a symbolic link in any component, `ENOTDIR` for a component
/// that is not a directory, `EXDEV` for a resolution that would leave the
/// destination, `ENOENT` for a component that is not there, and whatever else
/// `openat2(2)` reported.
pub fn resolve(root: &Dir, components: &[String]) -> Result<Dir, Errno> {
    let mut path = String::from(".");
    for component in components {
        path.push('/');
        path.push_str(component);
    }
    let flags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::CLOEXEC);
    openat2(&root.0, path.as_str(), flags, Mode::empty(), RESOLVE).map(Dir)
}

/// Create one directory named `name` directly inside `parent`.
///
/// # Errors
///
/// `EEXIST` when anything is already there — including a symbolic link, which
/// `mkdirat` never follows — and whatever else `mkdirat(2)` reported.
pub fn create_dir(parent: &Dir, name: &str) -> Result<(), Errno> {
    mkdirat(&parent.0, name, DIRECTORY_MODE)
}

/// Create one file named `name` directly inside `parent`, exclusively.
///
/// `O_CREAT | O_EXCL | O_NOFOLLOW` on a single component: it refuses rather
/// than truncating, and it does not follow a symbolic link at the name.
///
/// # Errors
///
/// `EEXIST` when anything is already there, and whatever else `openat(2)`
/// reported.
pub fn create_file(parent: &Dir, name: &str) -> Result<File, Errno> {
    let flags = OFlags::WRONLY
        .union(OFlags::CREATE)
        .union(OFlags::EXCL)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);
    openat(&parent.0, name, flags, FILE_MODE).map(File::from)
}

/// Remove one file named `name` directly inside `parent`.
///
/// # Errors
///
/// Whatever `unlinkat(2)` reported.
pub fn remove_file(parent: &Dir, name: &str) -> Result<(), Errno> {
    unlinkat(&parent.0, name, AtFlags::empty())
}
