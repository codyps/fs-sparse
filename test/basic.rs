use std::process::Command;

// [ENXIO] The whence argument is SEEK_HOLE or SEEK_DATA, and offset is
// greater or equal to the file size; or the whence argument is SEEK_DATA
// and the offset falls within the final hole of the file.

// https://www.systutorials.com/how-to-efficiently-archive-a-very-large-sparse-file/
// notes a potential difference in behavior between a "all hole" and "hole with 1 data" file.

fn dd_ct(path: &Path, ct: u64, seek: u64) {
    let c = Command::new("dd")
        .args(["if=/dev/zero", &format!("of={}", path), "bs=1", &format!("count={}", ct), &format!("seek={}", seek)])
        .status()
        .expect("dd failed to execute");
}

#![test]
fn dd_ct_0() {
    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    dd_ct(tmpfile.path(), 0, 10 * 1024 * 1024 * 1024);

    // macos/apfs: using SEEK_DATA returns ENXIO
}

#[test]
fn dd_ct_1() {
    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    dd_ct(tmpfile.path(), 1, 10 * 1024 * 1024 * 1024);

    // macos/apfs: using SEEK_DATA returns valid offset (of 10G), but SEEK_HOLE then returns 0
    // (instead of 10G + 1).
}
