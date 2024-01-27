mod error;

use std::path::PathBuf;
use std::{net::Ipv4Addr, str::FromStr, fs::File, ops::Range};
use std::io::{Cursor, Write};

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
                if v.is_some() { return v; }
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

// a: [u8; 32] = [31, 32, 33, 34, ...]
// b: [u8; 32] = [0F, 0F, 0F, 0F, ...]
fn mask_num(b: &[u8]) -> [u8; 32] {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    const MASK: &[u8] = &[0x0F; 32];

    unsafe {
        let a = _mm256_loadu_si256(b    as *const _ as *const _);
        let b = _mm256_loadu_si256(MASK as *const _ as *const _);

        std::mem::transmute(_mm256_and_si256(a, b))
    }
}

fn into_num64(b: &[u8]) -> u32 {
    let mut num = [0u8; 10];
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), num.get_unchecked_mut(10 - b.len()..).as_mut_ptr(), b.len()) }

    let mut head: u16 = unsafe { *(num.as_ptr() as *mut u16) };
    let mut tail: u64 = unsafe { *(num[2..].as_ptr() as *mut u64) };

    // masked input
    // head = head.wrapping_mul((10 << 8) + 1) >> 8;
    // tail = tail.wrapping_mul((10 << 8) + 1) >> 8;

    head = (head & 0x0F0F).wrapping_mul((10 << 8) + 1) >> 8;

    tail = (tail & 0x0F0F0F0F0F0F0F0F).wrapping_mul((10 << 8) + 1) >> 8;
    tail = (tail & 0x00FF00FF00FF00FF).wrapping_mul((100   << 16) + 1) >> 16;
    tail = (tail & 0x0000FFFF0000FFFF).wrapping_mul((10000 << 32) + 1) >> 32;

    (head as u32 * 100000000) + tail as u32
}

// worse than 64 bit tail + head version
#[cfg(target_pointer_width = "128")]
fn into_num128(b: &[u8]) -> u32 {
    let mut num = [0u8; 16];

    let mut num: u128 = unsafe {
        std::ptr::copy_nonoverlapping(b.as_ptr(), num.get_unchecked_mut(16 - b.len()..).as_mut_ptr(), b.len());
        *(num.as_ptr() as *mut _)
    };

    // masked input
    // num = num.wrapping_mul((10 << 8) + 1) >> 8;

    // unmasked input
    num = (num & 0x0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F).wrapping_mul((10        <<  8) + 1) >>  8;
    num = (num & 0x00FF00FF00FF00FF00FF00FF00FF00FF).wrapping_mul((100       << 16) + 1) >> 16;
    num = (num & 0x0000FFFF0000FFFF0000FFFF0000FFFF).wrapping_mul((10000     << 32) + 1) >> 32;
    num = (num & 0x00000000FFFFFFFF00000000FFFFFFFF).wrapping_mul((100000000 << 64) + 1) >> 64;

    num as u32
}

fn value(b: &[u8], n: u32) -> Option<&str> {
    const COMMAS: [u8; 16] = [b','; 16];

    unsafe {
        let mask = mask_128(b.get_unchecked(6..6 + 16), &COMMAS);

        let first: usize = 6 + mask.trailing_zeros() as usize;
        let second: usize = first + 1 + (mask >> (first - 5)).trailing_zeros() as usize;

        let min = into_num64(b.get_unchecked(..first));
        let max = into_num64(b.get_unchecked(first + 1..second));

        // do not reorder
        if n > max || min > n {
            None
        } else {
            Some(std::str::from_utf8_unchecked(b.get_unchecked(second + 1..b.len() - 1)))
        }
    }
}

#[cfg(test)]
mod tests;
