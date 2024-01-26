#![feature(portable_simd)]
#![feature(core_intrinsics)]

mod error;

use std::intrinsics::unlikely;
use std::path::PathBuf;
use std::simd::num::SimdUint;
use std::{net::Ipv4Addr, str::FromStr, fs::File, ops::Range};
use std::io::{Cursor, Write};

use std::simd::Simd;
use std::simd::cmp::SimdPartialEq;

use argh::FromArgs;
use error::Error;
use memmap2::Mmap;

#[derive(FromArgs)]
/// Offline IP address lookup tool.
struct Args {
    #[argh(positional, arg_name = "[DATABASE]", greedy)]
    db: PathBuf,

    #[argh(positional, arg_name = "[IPv4]", greedy)]
    ip: String,

    /// amount of workers (default: 1)
    #[argh(option, default = "1", short = 'w', long = "workers")]
    workers: usize,
}

fn main() -> Result<(), Error> {
    let args: Args = argh::from_env();

    let ip = Ipv4Addr::from_str(&args.ip).map_err(Error::IP)?;
    let file = File::open(args.db).map_err(Error::FileOpen)?;

    let mmap: Mmap = unsafe { Mmap::map(&file).map_err(Error::Mmap)? };
    let b: &[u8] = &mmap;

    let data = if args.workers == 1 {
        lookup_ipv4(b, &ip)
    } else {
        parallel(b, &ip, args.workers)
    }?;

    match data {
        Some(s) => {
            let mut stdout = std::io::stdout().lock();
            unsafe { stdout.write_all(s.as_bytes()).unwrap_unchecked(); }

            Ok(())
        },
        None => Err(Error::NotFound),
    }
}

fn parallel<'a>(b: &'a [u8], ip: &Ipv4Addr, mut workers: usize) -> Result<Option<&'a str>, Error> {
    if workers == 0 { workers = std::thread::available_parallelism().map_err(Error::Workers)?.get(); }

    let ip_num = ipv4_num(ip);
    let ip_buf = unsafe {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},").unwrap_unchecked();

        c.into_inner()
    };

    let (tx, rx) = std::sync::mpsc::channel::<Option<&'a str>>();

    std::thread::scope(|s| {
        chunks(b, workers).for_each(|r| {
            s.spawn({
                let tx = tx.clone();
                move || tx.send(lookup_ipv4_num(&b[r], ip_num, &ip_buf))
            });
        });

        for _ in 0..workers {
            let data = unsafe { rx.recv().unwrap_unchecked() };
            if data.is_some() { return Ok(data); }
        }

        Ok(None)
    })
}

fn chunks(b: &[u8], workers: usize) -> impl Iterator<Item = Range<usize>> + '_ {
    let size = b.len() / workers;

    let mut start: usize = 0;

    (0..workers).map(move |_| {
        let end = start + size;

        if end + size < b.len() {
            let nl = find_nl(&b[end..]);

            if nl != 0 {
                start += size + nl + 1;
                return end - size..end + nl + 1
            }
        }

        start..b.len()
    })
}

fn lookup_ipv4<'a>(b: &'a [u8], ip: &Ipv4Addr) -> Result<Option<&'a str>, Error> {
    let ip_num = ipv4_num(ip);
    let ip_buf = unsafe {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},").unwrap_unchecked();

        c.into_inner()
    };

    Ok(lookup_ipv4_num(b, ip_num, &ip_buf))
}

fn lookup_ipv4_num<'a>(mut b: &'a [u8], ip_num: u32, ip_buf: &[u8]) -> Option<&'a str> {
    let mut best_mask: u32 = 0;

    while !b.is_empty() {
        unsafe {
            let num_mask: u32 = mask_128(b.get_unchecked(..16), ip_buf).reverse_bits();

            let nl = find_nl(b);

            if num_mask.leading_ones() >= best_mask.leading_ones() || num_mask > best_mask {
                best_mask = num_mask;

                let v = value(b.get_unchecked(..=nl), ip_num);
                if unlikely(v.is_some()) { return v; }
            }

            b = b.get_unchecked(nl + 1..);
        }
    }

    None
}

fn find_nl(b: &[u8]) -> usize {
    const NEWLINES: [u8; 32] = [b'\n'; 32];

    let mut nl: usize = 9;
    let mut nl_mask: u32 = mask_128(unsafe { b.get_unchecked(nl..nl + 16) }, &NEWLINES[..16]);

    nl += (nl_mask as u16).trailing_zeros() as usize;

    while nl_mask == 0 {
        if unlikely(nl + 32 > b.len()) { return b.len() - 1 }
        nl_mask = mask_256(unsafe { b.get_unchecked(nl..nl + 32) }, &NEWLINES);
        nl += nl_mask.trailing_zeros() as usize;
    }

    nl
}

fn ipv4_num(ip: &Ipv4Addr) -> u32 {
    let octets: Simd<u32, 4> = Simd::from_array(ip.octets().map(u32::from));
    const MUL:  Simd<u32, 4> = Simd::from_array([16777216, 65536, 256, 1]);

    (octets * MUL).reduce_sum()
}

fn mask_256(a: &[u8], b: &[u8]) -> u32 {
    let a: Simd<u8, 32> = Simd::from_slice(a);
    let b: Simd<u8, 32> = Simd::from_slice(b);

    a.simd_eq(b).to_bitmask() as u32
}

fn mask_128(a: &[u8], b: &[u8]) -> u32 {
    let a: Simd<u8, 16> = Simd::from_slice(a);
    let b: Simd<u8, 16> = Simd::from_slice(b);

    a.simd_eq(b).to_bitmask() as u32
}

fn value(b: &[u8], n: u32) -> Option<&str> {
    const COMMAS: [u8; 16] = [b','; 16];

    unsafe {
        let mask = mask_128(b.get_unchecked(6..6 + 16), &COMMAS);

        let first: usize = 6 + mask.trailing_zeros() as usize;
        let second: usize = first + 1 + (mask >> (first - 5)).trailing_zeros() as usize;

        let s = std::str::from_utf8_unchecked(b);

        let min = u32::from_str(s.get_unchecked(..first)).unwrap_unchecked();
        let max = u32::from_str(s.get_unchecked(first + 1..second)).unwrap_unchecked();

        // do not reorder
        if min > n || n > max {
            None
        } else {
            Some(s.get_unchecked(second + 1..b.len() - 1))
        }
    }
}

#[cfg(test)]
mod tests;
