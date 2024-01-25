use std::path::PathBuf;
use std::{net::Ipv4Addr, str::FromStr, fs::File, ops::Range};
use std::io::{Cursor, Write};

use anyhow::Error;
use argh::FromArgs;
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

    let ip = Ipv4Addr::from_str(&args.ip)?;
    let file = File::open(args.db)?;

    let mmap: Mmap = unsafe { Mmap::map(&file)? };
    let b: &[u8] = &mmap;

    let data = if args.workers == 1 {
        lookup_ipv4(b, &ip)
    } else {
        parallel(b, &ip, args.workers)
    }?;

    match data {
        Some(s) => println!("{s}"),
        None => todo!(),
    }

    Ok(())
}

fn parallel<'a>(b: &'a [u8], ip: &Ipv4Addr, mut workers: usize) -> Result<Option<&'a str>, Error> {
    if workers == 0 { workers = std::thread::available_parallelism()?.get(); }

    let ip_num = ipv4_num(ip);
    let ip_buf = {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},")?;

        c.into_inner()
    };

    let (tx, rx) = std::sync::mpsc::channel::<Option<&'a str>>();

    std::thread::scope(|s| {
        chunks(b, workers).for_each(|r| {
            s.spawn({
                let tx = tx.clone();
                move || tx.send(lookup_ipv4_num(&b[r], ip_num, &ip_buf).ok().flatten())
            });
        });

        for _ in 0..workers {
            let data = rx.recv().unwrap();
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
    let ip_buf = {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},")?;

        c.into_inner()
    };

    lookup_ipv4_num(b, ip_num, &ip_buf)
}

fn lookup_ipv4_num<'a>(mut b: &'a [u8], ip_num: u32, ip_buf: &[u8]) -> Result<Option<&'a str>, Error> {
    let mut best_mask: u32 = 0;

    while !b.is_empty() {
        unsafe {
            let nl = find_nl(b);

            let num_mask: u32 = mask_128(b.get_unchecked(..16), ip_buf).reverse_bits();

            if num_mask.leading_ones() >= best_mask.leading_ones() || num_mask > best_mask {
                best_mask = num_mask;

                let v = value(b.get_unchecked(..=nl), ip_num);
                if v.is_some() { return Ok(v); }
            }

            b = b.get_unchecked(nl + 1..);
        }
    }

    Ok(None)
}

fn find_nl(b: &[u8]) -> usize {
    const NEWLINES: [u8; 32] = [b'\n'; 32];

    let mut nl: usize = 9;
    let mut nl_mask: u32 = mask_128(unsafe { b.get_unchecked(nl..nl + 16) }, &NEWLINES[..16]);

    nl += (nl_mask as u16).trailing_zeros() as usize;

    while nl_mask == 0 {
        if nl + 32 > b.len() { return b.len() - 1 }
        nl_mask = mask_256(unsafe { b.get_unchecked(nl..nl + 32) }, &NEWLINES);
        nl += nl_mask.trailing_zeros() as usize;
    }

    nl
}

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn ipv4_num(ip: &Ipv4Addr) -> u32 {

    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let octets = ip.octets().map(u32::from);
    const MUL: [u32; 4] = [16777216, 65536, 256, 1];

    unsafe {
        let a = _mm_loadu_si128(&octets as *const _ as *const _);
        let b = _mm_load_si128(&MUL as *const _ as *const _);

        let mul = _mm_mullo_epi32(a, b);
        let mul = std::mem::transmute::<_, [u32; 4]>(mul);

        mul.iter().sum()
    }
}

// a: [u8; 32] = [1, 2, 3, 4, ...]
// b: [u8; 32] = [1, 4, 3, 2, ...]
// >>>>>>>>>>>   [1, 0, 1, 0, ...]
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn mask_256(a: &[u8], b: &[u8]) -> u32 {

    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    unsafe {
        let a = _mm256_loadu_si256(a as *const _ as *const _);
        let b = _mm256_loadu_si256(b as *const _ as *const _);

        let cmp = _mm256_cmpeq_epi8(a, b);
        _mm256_movemask_epi8(cmp) as u32
    }
}

// a: [u8; 16] = [1, 2, 3, 4, ...]
// b: [u8; 16] = [1, 4, 3, 2, ...]
// >>>>>>>>>>>   [1, 0, 1, 0, ...]
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn mask_128(a: &[u8], b: &[u8]) -> u32 {

    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    unsafe {
        let a = _mm_loadu_si128(a as *const _ as *const _);
        let b = _mm_loadu_si128(b as *const _ as *const _);

        let cmp = _mm_cmpeq_epi8(a, b);
        _mm_movemask_epi8(cmp) as u32
    }
}

fn value(b: &[u8], n: u32) -> Option<&str> {
    const COMMAS: [u8; 16] = [b','; 16];

    unsafe {
        let mask = mask_128(b.get_unchecked(6..21), &COMMAS);

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
