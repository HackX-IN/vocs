/*!
 * SOURCE OF TRUTH KEYWORDS: free_bytes, space_needed, DiskSpaceCheck,
 *   check_space, DOWNLOAD_HEADROOM_BYTES, NSFileSystemFreeSize
 * WHAT:  Asks the OS how much room is left on the volume holding a path, and
 *        decides whether a download of a given size fits.
 * WHY:   The model download is roughly 600MB and the first thing a new user
 *        does. Running out of disk partway through ends in a write error the
 *        user reads as "the app is broken", after they have already waited. One
 *        question asked up front turns that into a sentence that names the
 *        number and the fix.
 *
 *        The check is deliberately forgiving in one direction: if the OS will
 *        not say (an unreadable volume, a failed call), the download proceeds
 *        and a genuine disk-full error surfaces from the write instead. A
 *        preflight that refuses on its own failure would block users whose
 *        disks are fine. It is also sized to the bytes STILL to fetch, not the
 *        whole file, so a nearly-finished resume is not refused for room it has
 *        already used.
 * WHERE: Called by adapters/http_models/store.rs::ensure before any download.
 */

use std::path::Path;

/// Room beyond the file itself: the SQLite WAL, logs, and the hash pass all
/// write while the part file is on disk, and a volume filled to the last byte
/// makes macOS itself misbehave.
pub const DOWNLOAD_HEADROOM_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum DiskSpaceCheck {
    Enough,
    /// Not enough: how many bytes are needed, and how many are free.
    Short { needed: u64, available: u64 },
    /// The OS would not say. Proceed; see the module WHY.
    Unknown,
}

/// Whether `remaining` more bytes (plus headroom) fit in `available`.
pub fn check_space(remaining: u64, available: Option<u64>) -> DiskSpaceCheck {
    let Some(available) = available else {
        return DiskSpaceCheck::Unknown;
    };
    let needed = remaining.saturating_add(DOWNLOAD_HEADROOM_BYTES);
    if available >= needed {
        DiskSpaceCheck::Enough
    } else {
        DiskSpaceCheck::Short { needed, available }
    }
}

/// Free bytes on the volume containing `path`. The path must exist.
#[cfg(target_os = "macos")]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use objc2_foundation::{NSFileManager, NSNumber, NSString};

    let manager = NSFileManager::defaultManager();
    let attributes = manager
        .attributesOfFileSystemForPath_error(&NSString::from_str(&path.to_string_lossy()))
        .ok()?;
    // The documented value of the NSFileSystemFreeSize constant. Looked up by
    // its string so no `unsafe` extern static is needed to read it.
    let free = attributes.objectForKey(&NSString::from_str("NSFileSystemFreeSize"))?;
    let number = free.downcast_ref::<NSNumber>()?;
    Some(number.unsignedLongLongValue())
}

#[cfg(not(target_os = "macos"))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

/// Megabytes, rounded up, for a sentence a person reads.
pub fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    #[test]
    fn a_download_that_fits_with_headroom_is_enough() {
        assert_eq!(check_space(600 * MB, Some(2_000 * MB)), DiskSpaceCheck::Enough);
    }

    #[test]
    fn the_file_fitting_exactly_is_not_enough_because_headroom_is_part_of_the_cost() {
        let result = check_space(600 * MB, Some(600 * MB));
        assert_eq!(
            result,
            DiskSpaceCheck::Short { needed: 600 * MB + DOWNLOAD_HEADROOM_BYTES, available: 600 * MB }
        );
    }

    #[test]
    fn an_unreadable_volume_never_blocks_a_download() {
        assert_eq!(check_space(600 * MB, None), DiskSpaceCheck::Unknown);
    }

    #[test]
    fn a_nearly_finished_resume_needs_almost_nothing() {
        assert_eq!(check_space(10 * MB, Some(400 * MB)), DiskSpaceCheck::Enough);
    }

    #[test]
    fn a_huge_remaining_size_cannot_overflow_the_sum() {
        assert!(matches!(check_space(u64::MAX, Some(1)), DiskSpaceCheck::Short { .. }));
    }

    #[test]
    fn megabytes_round_up() {
        assert_eq!(megabytes(1), 1);
        assert_eq!(megabytes(MB), 1);
        assert_eq!(megabytes(MB + 1), 2);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_volume_reports_a_plausible_amount_of_free_space() {
        let free = free_bytes(&std::env::temp_dir()).expect("the temp volume answers");
        assert!(free > 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_path_that_does_not_exist_is_unknown_not_a_panic() {
        assert_eq!(free_bytes(Path::new("/definitely/not/a/real/path")), None);
    }
}
