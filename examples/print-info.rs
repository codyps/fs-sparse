#![warn(rust_2018_idioms)]

use libc::SEEK_END;

#[cfg(target_os = "macos")]
mod macos {
    pub const SEEK_HOLE: i32 = 3;
    pub const SEEK_DATA: i32 = 4;
}
#[cfg(target_os = "macos")]
use macos::*;

#[cfg(target_os = "linux")]
mod linux {
    pub use libc::{SEEK_DATA, SEEK_HOLE};
}
#[cfg(target_os = "linux")]
use linux::*;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use std::path::Path;
use std::io;
use std::fs::File;
use std::convert::TryInto;

// macos: has fcntl(dst_fd, F_PUNCHHOLE, &punchhole_args) to punch holes into existing files
//
// https://opensource.apple.com/source/copyfile/copyfile-166.40.1/copyfile.c.auto.html


// http://git.savannah.gnu.org/cgit/tar.git/tree/src/sparse.c#n273

// https://www.austingroupbugs.net/view.php?id=415


// zfsonlinux (zfs-0.8.4-1, linux 5.7.7-arch1-1)
// # zfs_dmu_offset_next_sync = 0
// args: ["./test_0.bin"]
// ENXIO
// END: 10737418240
// args: ["./test_1.bin"]
// ENXIO
// END: 10737418241
//
// iow: can't determine that sparse isn't working
//
// # zfs_dmu_offset_next_sync = 1
//
// appears to do the same thing. something is broken

// tmpfs (linux 5.7.7-arch1-1)
// [y@arnold tmp]$ ~/p/fs-sparse/target/debug/examples/print-info ./test_0.bin 
// args: ["./test_0.bin"]
// e: -1
// seek(f, 0, SEEK_DATA) -> ENXIO
// seek(f, 0, SEEK_END) -> 10737418240
// [y@arnold tmp]$ ~/p/fs-sparse/target/debug/examples/print-info ./test_1.bin 
// args: ["./test_1.bin"]
// seek(f, 0, SEEK_DATA) -> 10737418240
// seek(f, 10737418240, SEEK_HOLE) -> 10737418241
// e: -1
// seek(f, 10737418241, SEEK_DATA) -> ENXIO
// seek(f, 0, SEEK_END) -> 10737418241

fn usage(e: i32) -> ! {
    let ename = std::env::args().next().unwrap();

    eprintln!(
r"Dump file layout info

Usage: {0} <input_file>
",
    ename);

    std::process::exit(e)
}

fn seek(file: &File, offset: u64, loc: i32) -> io::Result<u64> {
    // TODO: use lseek64 on 32-bit platforms that have it for larger seeks
    let off = unsafe { libc::lseek(file.as_raw_fd(), offset.try_into().unwrap(), loc) };
    if off < 0 {
        // error!
        println!("e: {}", off);
        return Err(io::Error::last_os_error());
    }

    Ok(off.try_into().unwrap())
}

fn one_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;

    let mut offset = 0;
    loop {
        let r = seek(&file, offset, SEEK_DATA);
        print!("seek(f, {}, SEEK_DATA) -> ", offset);
        match r {
            Err(ref e) if e.raw_os_error() == Some(libc::ENXIO) => {
                /* No more data. */

                /* macos/apfs: If this is the first extent, this may be an all-hole file. We
                 * _probably_ need to examine */
                println!("ENXIO");
                let end = seek(&file, 0, SEEK_END)?;
                println!("seek(f, 0, SEEK_END) -> {}", end);

                let end_hole = seek(&file, offset, SEEK_HOLE)?;
                println!("seek(f, {}, SEEK_HOLE) -> {}", offset, end_hole);
                break;
            }
            Err(e) => {
                panic!(e);
            }
            Ok(off) => {
                println!("{}", off);
                offset = off;
                if off == 0 {
                    break;
                }
            }
        }

        let off = seek(&file, offset, SEEK_HOLE).unwrap();
        println!("seek(f, {}, SEEK_HOLE) -> {}", offset, off);
        offset = off;
        if off == 0 {
            break;
        }
    }

    Ok(())
}

fn cmd(args: pico_args::Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let input_files = args.free()?;

    println!("args: {:?}", input_files);
    for path in input_files {
        one_file((&path).as_ref())?
    }

    Ok(())
}

// NOTE: we don't use a `Result` return because it uses debug formatting and we want display
// formatting for our errors.
fn main() {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        usage(0)
    }

    match cmd(args) {
        Ok(()) => {},
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
