//! Checked little-endian reads over a borrowed archive image.
//!
//! Every read is bounds-checked and reports truncation at a named structure, so
//! no parsing step can index past the caller-supplied slice or wrap an offset.

use crate::error::{ArchiveError, Structure};

/// A bounds-checked forward reader positioned inside the archive image.
pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    structure: Structure,
    entry: Option<u32>,
}

impl<'a> Cursor<'a> {
    /// Create a cursor over `bytes`, reporting truncation at `structure`.
    pub(crate) const fn new(bytes: &'a [u8], structure: Structure, entry: Option<u32>) -> Self {
        Self {
            bytes,
            position: 0,
            structure,
            entry,
        }
    }

    /// Current offset from the start of the borrowed slice.
    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    /// The truncation error this cursor reports.
    pub(crate) const fn truncated(&self) -> ArchiveError {
        ArchiveError::Truncated {
            at: self.structure,
            entry: self.entry,
        }
    }

    /// Read `count` bytes, or report truncation.
    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], ArchiveError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| self.truncated())?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| self.truncated())?;
        self.position = end;
        Ok(slice)
    }

    /// Read a little-endian `u16`.
    pub(crate) fn u16(&mut self) -> Result<u16, ArchiveError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Read a little-endian `u32`.
    pub(crate) fn u32(&mut self) -> Result<u32, ArchiveError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read and verify a four-byte record signature.
    pub(crate) fn signature(&mut self, expected: u32) -> Result<(), ArchiveError> {
        if self.u32()? == expected {
            Ok(())
        } else {
            Err(ArchiveError::Malformed {
                kind: crate::error::MalformedKind::RecordSignature,
                entry: self.entry,
            })
        }
    }
}

/// Convert a `u64` archive offset into an index into the image.
pub(crate) fn offset(
    value: u64,
    structure: Structure,
    entry: Option<u32>,
) -> Result<usize, ArchiveError> {
    usize::try_from(value).map_err(|_| ArchiveError::Truncated {
        at: structure,
        entry,
    })
}
