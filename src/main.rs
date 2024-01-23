mod cheat;

use std::{net::Ipv4Addr, str::FromStr, path::Path, fs::File, ops::Range};
use std::sync::{Mutex, Arc};
use std::io::{Write, Cursor};

use anyhow::Error;
use memmap2::Mmap;

use crate::cheat::*;

fn usage() {
    println!("Usage: {} [.../dbip-country-ipv4-num.csv] [IPv4]", env!("CARGO_BIN_NAME"));
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.len() != 2 { usage() }

    let path = Path::new(&args[0]);
    let ip = Ipv4Addr::from_str(&args[1])?;

    let file = File::open(path)?;

    let mmap: Mmap = unsafe { Mmap::map(&file).unwrap() };
    let b: &[u8] = &mmap;

    let country = parallel(b, &ip, 0)?;
    println!("{}", country.unwrap());

    Ok(())
}

fn parallel<'a>(b: &'a [u8], ip: &Ipv4Addr, mut workers: usize) -> Result<Option<&'a str>, Error> {
    if workers == 0 { workers = std::thread::available_parallelism()?.get(); }

    let result = Arc::new(Mutex::new(None));

    std::thread::scope(|s| {
        chunks(b, workers).for_each(|r| {
            let result = result.clone();
            s.spawn(move || {
                if let Some(country) = lookup_ipv4(&b[r], ip).ok().flatten() {
                    let mut lock = result.lock().unwrap();
                    *lock = Some(country);
                }
            });
        })
    });

    let m = Arc::try_unwrap(result).unwrap();
    let inner = m.into_inner().unwrap();

    Ok(inner)
}

fn chunks(b: &[u8], workers: usize) -> impl Iterator<Item = Range<usize>> + '_ {
    let size = b.len() / workers;

    const NEWLINES: [u8; 16] = [b'\n'; 16];

    let mut start: usize = 0;

    (0..workers).map(move |_| {
        let end = start + size;

        if end > b.len() || size == b.len() { return start..b.len() }

        let mut nl_mask = mask(&b[end..(end + 16).min(b.len())], &NEWLINES) as u16;
        let mut nl: usize = 0;
        if nl_mask == 0 {
            nl_mask = mask(&b[end + 16..end + 32], &NEWLINES) as u16;
            nl += 16;
        }

        nl += nl_mask.trailing_zeros() as usize;

        if nl != 0 {
            start += size + nl + 1;
            return end - size..end + nl + 1
        }

        start..b.len()
    })
}

fn lookup_ipv4<'a>(mut b: &'a [u8], ip: &Ipv4Addr) -> Result<Option<&'a str>, Error> {
    let ip_num = ipv4_num(ip);
    let ip_buf = {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},")?;

        c.into_inner()
    };

    let mut best_mask: u32 = 0;

    const NEWLINES: [u8; 16] = [b'\n'; 16];

    while !b.is_empty() {
        let nl_mask = mask(unsafe { b.get_unchecked(9..MAX_LINE_LEN.min(b.len())) }, &NEWLINES);
        let nl: usize = MIN_LINE_LEN - 1 + (nl_mask >> 11).trailing_zeros() as usize;

        let num_mask: u32 = mask(unsafe { b.get_unchecked(..16) }, &ip_buf).reverse_bits();

        if num_mask.leading_ones() >= best_mask.leading_ones() || num_mask > best_mask {
            best_mask = num_mask;

            let country = confirm(&b[..nl], ip_num);
            if country.is_some() {
                return Ok(country);
            }
        }

        b = &b[nl + 1..];
    }

    Ok(None)
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

// a: [u8; 16] = [1, 2, 3, 4, ...]
// b: [u8; 16] = [1, 4, 3, 2, ...]
// >>>>>>>>>>>   [1, 0, 1, 0, ...]
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn mask(a: &[u8], b: &[u8]) -> u32 {

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

fn confirm(b: &[u8], n: u32) -> Option<&str> {
    const COMMAS: [u8; 4] = [0, 0, b',', b','];

    let s = std::str::from_utf8(b).ok()?;

    let mask = mask(&b[7..11], &COMMAS) as u8;
    let offset: usize = 8  + (mask >> 2) as usize;

    let min = u32::from_str(&s[..offset]).ok()?;
    let max = u32::from_str(&s[offset + 1..b.len() - 3]).ok()?;

    if (min..=max).contains(&n) { Some(&s[s.len() - 2..]) } else { None }
}

#[cfg(test)]
mod tests;
