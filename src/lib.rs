//! Interact with sparse files provided by your system's file system
//!
//! Sparse files have holes. A "hole," in this case, is defined as a range of zeroes which need not
//! correspond to blocks which have actually been omitted from the file, though in practice it
//! almost certainly will.
//!
//! # Simultanious file access while iterating
//!
//! Many (but not all) platforms use iteration methods that internally are `seek()` type operations
//! that change the `File`'s cursor (so that reads/writes/seeks would occur from the new position).
//!
//! Using `read_at()` type operations based on the offsets returned by `SparseIter` are the only
//! portable option avaliable. Using `read()` or other file cursor adjusting functions durring
//! iteration will expose platform dependent behavior.
//!
//! Using any write may transform `Hole`s into `Data`, potentially after an iteration has already
//! examined that range. In general, writes while iterating will cause iteration to have behavior
//! that may silently change between `fs-sparse` releases and may differ between platforms and
//! filesystems.
//!
//! # Portability
//!
//!  - When using openzfs, you may need to set the zfs_dmu_offset_next_sync=1 option to get good
//!    reporting for holes.
//!    (see the [openzfs documentation](https://openzfs.github.io/openzfs-docs/Performance%20and%20Tuning/ZFS%20on%20Linux%20Module%20Parameters.html#zfs-dmu-offset-next-sync))
//!  - When using MacOS APFS, starting a sparse iter won't report the item the cursor is in the
//!    middle of (for details, see [this mailing list
//!    post](https://lists.gnu.org/archive/html/bug-gnulib/2018-09/msg00054.html)). Always starting
//!    iteration from the start of the file or from a previously returned Item offset should work.
//!    Mixing iterations and reads may not work properly
//!  - On Windows, files must specifically be marked as sparse (they have a seperate mode). If
//!    files are not sparse, this library indicates the entire file is one big `Data`.
//!  - On some systems, if one or more bytes with value 0 are _written_ to a file, it may still be
//!    considered a hole when read back. DO NOT assume that holes are locations that were never
//!    written to (looking at you, bmap-tools). This behavior is visible in (at least) zfsonlinux
//!    0.8.4.
//!
// # Portability (internal)
//
//  - Linux has an implicit hole at the end of the file, iow: SEEK_HOLE will return the end of the
//    file if no actual holes exist
//    - FIEMAP?
//  - Solaris?
//  - MacOS?
//  - Windows: totally different api
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]

use std::{fs, io};

/// Iterate over the start of Data and Holes within a `File`
#[derive(Debug)]
pub struct SparseIter<'a> {
    file: &'a fs::File,
}

impl<'a> From<&'a fs::File> for SparseIter<'a> {
    fn from(file: &'a fs::File) -> Self {
        // TODO: consider seeking to the start of the file. Not doing this allows writing
        // non-portable code.
        //
        // XXX: always need to allow non-portable escape hatches
        Self { file } 
    }
}

// MacOS man page:
//
//  - If whence is SEEK_HOLE, the offset is set to the start of the next hole greater than or equal
//    to the supplied offset.  The definition of a hole is provided below.
//  - If whence is SEEK_DATA, the offset is set to the start of the next non-hole file region
//    greater than or equal to the supplied offset.
//
// A "hole" is defined as a contiguous range of bytes in a file, all having the value of zero, but
// not all zeros in a file are guaranteed to be represented as holes returned with SEEK_HOLE.
// File systems are allowed to expose ranges of zeros with SEEK_HOLE, but not required to.
// Applications can use SEEK_HOLE to optimise their behavior for ranges of zeros, but must not
// depend on it to find all such ranges in a file.  Each file is presented as having a zero-size
// virtual hole at the very end of the file.  The existence of a hole at the end of every data
// region allows for easy programming and also provides compatibility to the original
// implementation in Solaris.  It also causes the current file size (i.e., end-of-file offset) to
// be returned to indicate that there are no more holes past the supplied offset.  Applications
// should use fpathconf(_PC_MIN_HOLE_SIZE) or pathconf(_PC_MIN_HOLE_SIZE) to determine if a file
// system supports SEEK_HOLE.  See pathconf(2).



#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::*;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

impl<'a> Iterator for SparseIter<'a> {
    type Item = io::Result<SparseItem>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // TODO: use lseek64 on 32-bit platforms that have it for larger seeks
            let off = unsafe { libc::lseek(self.file.as_raw_fd(), 0, SEEK_DATA) };
            if off < 0 {
                // error!
                return Some(Err(io::Error::last_os_error()));
            }


        }
    }
}

/// Is this Data or a Hole?
#[derive(Debug)]
pub enum ItemKind {
    /// Represents actual bytes (as far as the file system knows)
    Data,

    /// The absense of bytes and taken to equal a zeroed area
    Hole,

    /*
    /// We've reached the end of the file
    /// This is needed to communicate the total file length (and complete the range)
    ///
    /// For [`SparseIter`], this is returned immediately before the iterator completes (by returning
    /// `None`).
    /// [`SparseRangeIter`] will not return an Item with this kind.
    End,
    */
}

/// The item we've observed in the file we're iterating over
///
/// These correspond to distinct _points_ within the file rather than containing all information
/// for ranges.
///
/// To get ranges, use the `SparseRangeIter` adapter.
#[derive(Debug)]
pub struct SparseItem {
    /// The kind of this point
    pub kind: ItemKind,
    /// The byte offset in the file where this point is located
    pub offset: u64,
}

/// Iterate over a file returning the ranges of Data and Holes that compose it.
#[derive(Debug)]
pub struct SparseRangeIter<'a> {
    inner: SparseIter<'a>,
    prev: Option<SparseItem>,
}

impl<'a> From<SparseIter<'a>> for SparseRangeIter<'a> {
    fn from(inner: SparseIter<'a>) -> Self {
        Self { inner, prev: None } 
    }
}

impl<'a> Iterator for SparseRangeIter<'a> {
    type Item = io::Result<SparseRangeItem>;
    fn next(&mut self) -> Option<Self::Item> {
        /*
        let v = match self.inner.next() {
            Some(Err(e)) => {
                // TODO: consider fusing on error
                return Some(Err(e))
            },
            Some(Ok(v)) => Some(v),
            None => None,
        };

        match self.prev {
            None => {
                self.prev = v;
                None
            }
            Some(prev) => {
                let r = Some(Ok(SparseRangeItem { kind: prev.kind, start: prev.offset, end: v.offset }));
                self.prev = v;
                r
            }
        }
        */
        unimplemented!()
    }
}

/// A range from a [`SparseRangeIter`]
#[derive(Debug)]
pub struct SparseRangeItem {
    /// The kind of this range
    pub kind: ItemKind,
    /// The byte offset in the file where this range begins (including this offset)
    pub start: u64,
    /// The byte offset in the file 1 after this range ends (ie: excluding this offset)
    pub end: u64,
}
