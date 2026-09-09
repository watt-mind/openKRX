//! What an entry declares itself to be, derived from the central directory.
//!
//! A ZIP archive states an entry's nature in two fields that this crate would
//! otherwise ignore: the high byte of `version made by`, which names the host
//! system that wrote the record, and `external file attributes`, whose meaning
//! depends on that host. They are the only place an archive can declare a
//! symlink, a device node or a directory, so a layer that plans filesystem
//! output has to read them.
//!
//! The mapping is deliberately narrow. An entry whose host system this reader
//! does not know is [`EntryKind::Unknown`] rather than assumed to be a regular
//! file, and a Unix entry whose mode is neither a regular file, a directory nor
//! a symlink is [`EntryKind::Special`] rather than ignored. Refusing to guess is
//! the point: `SECURITY.md` requires special files and link escapes to be
//! rejected, and a guess in this function would be a silent hole in that rule.

/// Host system 3: Unix. The high 16 attribute bits carry `st_mode`.
const HOST_UNIX: u16 = 3;
/// Host systems whose attributes carry MS-DOS/FAT attribute bits.
///
/// 0 is MS-DOS and OS/2 FAT, 10 is Windows NTFS, 11 is MVS/OpenVMS as PKWARE
/// numbers it, and 14 is VFAT. All four write the FAT attribute byte.
const HOSTS_FAT: [u16; 4] = [0, 10, 11, 14];
/// FAT attribute bit 4: the entry is a directory.
const FAT_DIRECTORY: u32 = 0x10;
/// `st_mode` file-type mask.
const S_IFMT: u32 = 0xf000;
/// `st_mode` regular file.
const S_IFREG: u32 = 0x8000;
/// `st_mode` directory.
const S_IFDIR: u32 = 0x4000;
/// `st_mode` symbolic link.
const S_IFLNK: u32 = 0xa000;

/// What an archive entry declares itself to be.
///
/// This is a reading of the entry's own declaration, never a filesystem fact:
/// nothing here has been opened, and no path exists yet.
///
/// The enum is `#[non_exhaustive]`, because a host system this reader currently
/// maps to [`EntryKind::Unknown`] may later gain a mapping of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntryKind {
    /// An ordinary file: a Unix `S_IFREG` mode, or a FAT entry without the
    /// directory bit.
    RegularFile,
    /// A directory: a name ending in `/`, a Unix `S_IFDIR` mode, or a FAT entry
    /// carrying the directory bit. It declares no content of its own.
    DirectoryMarker,
    /// A symbolic link: a Unix `S_IFLNK` mode. Its content is a link target.
    Symlink,
    /// A Unix mode that is neither a regular file, a directory nor a symlink:
    /// a device node, socket, FIFO, or a mode this reader does not recognise.
    Special,
    /// A host system this reader does not map. Nothing about the entry's nature
    /// can be read from its attributes.
    Unknown,
}

/// Classify one central-directory record.
///
/// A name ending in `/` is a [`EntryKind::DirectoryMarker`] whatever the
/// attributes say: the inventory already rejects such a name when the entry
/// declares content, so the two facts cannot contradict each other in an
/// accepted archive.
pub(crate) fn classify(version_made_by: u16, external_attributes: u32, name: &[u8]) -> EntryKind {
    if name.last() == Some(&b'/') {
        return EntryKind::DirectoryMarker;
    }
    let host = version_made_by >> 8;
    if host == HOST_UNIX {
        return match (external_attributes >> 16) & S_IFMT {
            S_IFREG => EntryKind::RegularFile,
            S_IFDIR => EntryKind::DirectoryMarker,
            S_IFLNK => EntryKind::Symlink,
            _ => EntryKind::Special,
        };
    }
    if HOSTS_FAT.contains(&host) {
        return if external_attributes & FAT_DIRECTORY == 0 {
            EntryKind::RegularFile
        } else {
            EntryKind::DirectoryMarker
        };
    }
    EntryKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::{EntryKind, classify};

    /// Build a `version made by` value for `host` at an arbitrary version.
    const fn made_by(host: u16) -> u16 {
        (host << 8) | 20
    }

    /// Build external attributes carrying `mode` as the Unix `st_mode`.
    const fn unix_mode(mode: u32) -> u32 {
        mode << 16
    }

    #[test]
    fn a_trailing_separator_is_a_directory_whatever_the_attributes_say() {
        assert_eq!(
            classify(made_by(3), unix_mode(0xa1ff), b"dir/"),
            EntryKind::DirectoryMarker
        );
        assert_eq!(
            classify(made_by(99), 0, b"dir/"),
            EntryKind::DirectoryMarker
        );
    }

    #[test]
    fn unix_modes_map_to_their_file_types() {
        assert_eq!(
            classify(made_by(3), unix_mode(0o100_644), b"a"),
            EntryKind::RegularFile
        );
        assert_eq!(
            classify(made_by(3), unix_mode(0o040_755), b"a"),
            EntryKind::DirectoryMarker
        );
        assert_eq!(
            classify(made_by(3), unix_mode(0o120_777), b"a"),
            EntryKind::Symlink
        );
        assert_eq!(
            classify(made_by(3), unix_mode(0o020_666), b"a"),
            EntryKind::Special
        );
        assert_eq!(classify(made_by(3), 0, b"a"), EntryKind::Special);
    }

    #[test]
    fn fat_hosts_read_the_directory_bit_and_others_are_unknown() {
        for host in [0_u16, 10, 11, 14] {
            assert_eq!(classify(made_by(host), 0x20, b"a"), EntryKind::RegularFile);
            assert_eq!(
                classify(made_by(host), 0x10, b"a"),
                EntryKind::DirectoryMarker
            );
        }
        for host in [1_u16, 7, 19, 255] {
            assert_eq!(classify(made_by(host), 0x10, b"a"), EntryKind::Unknown);
        }
    }
}
