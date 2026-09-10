//! Unix: every destination path resolved beneath one directory descriptor.
//!
//! The portable writer checks a path and then creates it. Between those two
//! steps a principal with write access to the destination can replace a
//! component with a symbolic link, and the creation follows it out of the
//! destination the caller named. No portable standard-library call closes
//! that window: `symlink_metadata` answers a question about the past, and
//! `O_NOFOLLOW` covers only the last component.
//!
//! What closes it is never naming an absolute path again. The destination is
//! opened **once**, before anything is written, and every path after that is
//! resolved from that descriptor. How the resolution is made is the one thing
//! that differs between Unix targets, and it is the only `cfg` in this module.
//!
//! - **Linux 5.6 and newer** has `openat2(2)`, which resolves a whole
//!   relative path in the kernel under three resolve flags:
//!   `RESOLVE_BENEATH`, so the resolution may not leave the directory the
//!   descriptor names and `..`, an absolute path and a mount crossing are
//!   refused by the kernel rather than checked for by this crate;
//!   `RESOLVE_NO_SYMLINKS`, so no component may be a symbolic link, which is
//!   the check-to-create race itself, decided at the moment of the operation
//!   rather than one syscall earlier; and `RESOLVE_NO_MAGICLINKS`, so nor may
//!   it be one of the kernel's own pseudo-links, such as an entry under
//!   `/proc/self/fd`.
//! - **Every other Unix target** has no such call, so this module walks the
//!   components itself: one `openat(parent, component,
//!   O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` per component, each
//!   relative to the descriptor the step before it returned. A component
//!   that is a symbolic link when it is met is refused by `O_NOFOLLOW`; a
//!   component swapped after it has been opened cannot redirect anything,
//!   because what the walk holds from then on is the directory itself and not
//!   its name. The two differences from the kernel's own resolution are
//!   stated on the `walk` function itself, which exists only on those
//!   targets.
//!
//! Only a resolved directory descriptor and a **single** name component are
//! ever handed to `mkdirat`, `openat` or `unlinkat`, and none of them follows
//! a link at that last component: `mkdir` fails with `EEXIST` over a symbolic
//! link, and `O_CREAT | O_EXCL | O_NOFOLLOW` fails the same way. So no
//! absolute path is re-resolved after preflight — not to create, and not to
//! undo — and no name from the package is ever pasted into one.
//!
//! This module holds the syscalls and nothing else. Which refusal each
//! failure becomes is [`super::resolver`]'s, so that every platform answers
//! with the same `output.*` codes. `rustix` is the only dependency, with
//! default features off so that its `linux_raw` backend is used on Linux and
//! no C library is linked there; the workspace forbids `unsafe`, and nothing
//! here needs any.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{AtFlags, Mode, OFlags, mkdirat, open, openat, unlinkat};
use rustix::io::Errno;

#[cfg(target_os = "linux")]
use rustix::fs::{ResolveFlags, openat2};

#[cfg(not(target_os = "linux"))]
use rustix::fs::{FileType, statat};

/// The resolve flags every `openat2` in this module asks for.
///
/// They are asked for together, and a kernel that does not understand all
/// three is treated as a kernel without `openat2` at all: a partial guarantee
/// reported as the full one would be worse than the portable path, which at
/// least says what it is.
#[cfg(target_os = "linux")]
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

/// The flags a directory is opened with: read-only, a directory or nothing,
/// and never inherited across an exec.
const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::CLOEXEC);

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
/// A symbolic link is refused by both flags at once, and which error the
/// kernel reports depends on which check its path resolution reaches first:
/// on Linux, in practice `ENOTDIR`, from `O_DIRECTORY`, rather than the
/// `ELOOP` that `O_NOFOLLOW` alone would give. The caller maps both onto
/// codes the path rule already has, so nothing turns on the choice.
///
/// # Errors
///
/// Whatever `open(2)` reported.
pub fn open_root(destination: &Path) -> Result<Dir, Errno> {
    open(
        destination,
        DIRECTORY_FLAGS.union(OFlags::NOFOLLOW),
        Mode::empty(),
    )
    .map(Dir)
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
///
/// It exists on Linux alone: no other Unix target has a stronger mode to
/// probe for, so none of them has anything to fall back *from*.
#[cfg(target_os = "linux")]
#[must_use]
pub fn supported(root: &Dir) -> bool {
    resolve(root, &[]).is_ok()
}

/// Resolve `components` beneath `root`, refusing every link on the way.
///
/// An empty slice reopens `root` itself, which is what a plan item directly
/// in the destination needs and is the shape the Linux probe uses.
///
/// # Errors
///
/// `ELOOP` for a symbolic link in any component, `ENOTDIR` for a component
/// that is not a directory, `EXDEV` for a resolution that would leave the
/// destination, `ENOENT` for a component that is not there, and whatever else
/// `openat2(2)` reported.
#[cfg(target_os = "linux")]
pub fn resolve(root: &Dir, components: &[String]) -> Result<Dir, Errno> {
    let mut path = String::from(".");
    for component in components {
        path.push('/');
        path.push_str(component);
    }
    openat2(
        &root.0,
        path.as_str(),
        DIRECTORY_FLAGS,
        Mode::empty(),
        RESOLVE,
    )
    .map(Dir)
}

/// Resolve `components` beneath `root`, one component at a time.
///
/// # Errors
///
/// The same errors the Linux arm answers with, from `openat(2)` rather than
/// from `openat2(2)`. See `walk` for what the walk does and does not
/// promise.
#[cfg(not(target_os = "linux"))]
pub fn resolve(root: &Dir, components: &[String]) -> Result<Dir, Errno> {
    walk(root, components)
}

/// The component walk: `openat` per component, from the descriptor before it.
///
/// Each step opens one name in the directory the previous step returned, so
/// the process never re-resolves a path and never hands a multi-component
/// string to the kernel. `O_NOFOLLOW` refuses a component that is a symbolic
/// link at the moment it is met, and a component replaced *after* it has been
/// opened cannot redirect the rest of the walk, because from then on what the
/// walk holds is the directory itself rather than its name. Depth is bounded
/// by the planner's component ceiling, and one descriptor per level is open
/// at a time.
///
/// Two differences from what `openat2` gives on Linux are worth stating
/// rather than glossing over. The walk cannot refuse a **mount** planted at a
/// component the way `RESOLVE_BENEATH` does, so a principal who can mount
/// inside the destination can still place output on another filesystem; and
/// it has no `RESOLVE_NO_MAGICLINKS`, which costs nothing here, because those
/// pseudo-links live under `/proc` and a caller's destination is not there.
/// Neither is the check-to-create race this closes.
///
/// `.`, `..`, an empty component and one carrying a separator are refused as
/// `EXDEV` before any syscall is made. The planner has already refused all
/// four, and this is the assertion of that rule rather than a second opinion
/// about it: a component that reaches here is a plain name, or the plan is
/// not what it claims to be.
///
/// # Errors
///
/// `ELOOP` for a symbolic link in any component, `ENOTDIR` for a component
/// that is not a directory, `EXDEV` for a component that is not a plain name,
/// `ENOENT` for a component that is not there, and whatever else `openat(2)`
/// reported.
#[cfg(not(target_os = "linux"))]
fn walk(root: &Dir, components: &[String]) -> Result<Dir, Errno> {
    let mut current = openat(&root.0, ".", DIRECTORY_FLAGS, Mode::empty()).map(Dir)?;
    for component in components {
        if !is_plain_name(component) {
            return Err(Errno::XDEV);
        }
        match openat(
            &current.0,
            component.as_str(),
            DIRECTORY_FLAGS.union(OFlags::NOFOLLOW),
            Mode::empty(),
        ) {
            Ok(fd) => current = Dir(fd),
            Err(error) => return Err(refine(&current, component, error)),
        }
    }
    Ok(current)
}

/// Whether `component` is a single ordinary name, and not a way out.
#[cfg(not(target_os = "linux"))]
fn is_plain_name(component: &str) -> bool {
    !component.is_empty()
        && component != "."
        && component != ".."
        && !component.contains('/')
        && !component.contains('\0')
}

/// Tell a symbolic link from a plain non-directory, when the error cannot.
///
/// `O_NOFOLLOW` with `O_DIRECTORY` refuses a symbolic link twice over, and
/// which error a Unix reports for it is that kernel's own business: Linux
/// answers `ELOOP` where Darwin answers `ENOTDIR` for the same planted link.
/// Both are codes the path rule already has, but the two mean different
/// things to whoever reads the refusal — "a link is in the way" against
/// "something that is not a directory is in the way" — and openKRX answers
/// one code per condition on every platform rather than one per kernel.
///
/// So an `ENOTDIR` is asked about once, with one `fstatat` that does not
/// follow links, on the error path only. A component that turns out to be a
/// symbolic link is reported as `ELOOP`; anything else keeps the error the
/// kernel gave, and so does a failure of the question itself.
#[cfg(not(target_os = "linux"))]
fn refine(parent: &Dir, component: &str, error: Errno) -> Errno {
    if error != Errno::NOTDIR {
        return error;
    }
    match statat(&parent.0, component, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Symlink => Errno::LOOP,
        _ => error,
    }
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

/// Remove one **empty** directory named `name` directly inside `parent`.
///
/// `AT_REMOVEDIR` is `rmdir`: it removes nothing recursively and refuses a
/// directory that is not empty, so a file another principal put inside a
/// directory this run created keeps that directory alive rather than going
/// with it. It also refuses anything that is not a directory, with `ENOTDIR`,
/// which is how the undo pass declines to unlink a symbolic link standing
/// where a directory of this run's used to be.
///
/// # Errors
///
/// Whatever `unlinkat(2)` reported.
pub fn remove_dir(parent: &Dir, name: &str) -> Result<(), Errno> {
    unlinkat(&parent.0, name, AtFlags::REMOVEDIR)
}
