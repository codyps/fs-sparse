#![warn(rust_2018_idioms)]

#[cfg(target_os = "macos")]
mod macos {
    pub const SEEK_HOLE: i32 = 3;
    pub const SEEK_DATA: i32 = 4;
}
#[cfg(target_os = "macos")]
use macos::*;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use std::path::Path;
use std::io;
use std::fs::File;
use std::convert::TryInto;

fn usage(e: i32) -> ! {
    let ename = std::env::args().next().unwrap();

    eprintln!(
r"Dump file layout info

Usage: {0} <input_file>
",
    ename);

    std::process::exit(e)
}

fn seek(file: &File, loc: i32) -> io::Result<u64> {
    // TODO: use lseek64 on 32-bit platforms that have it for larger seeks
    let off = unsafe { libc::lseek(file.as_raw_fd(), 0, loc) };
    if off < 0 {
        // error!
        return Err(io::Error::last_os_error());
    }

    Ok(off.try_into().unwrap())
}

fn one_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;

    loop {
        let off = match seek(&file, SEEK_DATA) {
            Err(ref e)
        }

        println!("DATA: {}", off);
        if off == 0 {
            break;
        }

        let off = seek(&file, SEEK_HOLE).unwrap();

        println!("HOLE: {}", off);
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
