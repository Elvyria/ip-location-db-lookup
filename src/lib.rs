use std::num::NonZero;
use std::{net::Ipv4Addr, ops::Range};
use std::io::{Cursor, Write};

pub fn guess<'a>(b: &'a [u8], ip: &Ipv4Addr) -> Option<&'a str> {
    const MARGIN: usize = 1024 * 1024;

    let size = b.len() / 223;
    let offset = ip.octets()[0] as usize * size;

    let mut head: usize = offset.saturating_sub(MARGIN);
    head += if head != 0 {
        find_nl(unsafe { b.get_unchecked(offset - MARGIN..)} ) + 1
    }
    else { 0 };

    let mut tail = (offset + MARGIN).min(b.len());
    tail += find_nl(unsafe { b.get_unchecked(tail..) });

    let result = lookup_ipv4(unsafe { b.get_unchecked(head..tail + 1) }, ip);
    if result.is_some() { result } else { lookup_ipv4(b, ip) }
}

pub fn parallel<'a>(b: &'a [u8], ip: &Ipv4Addr, workers: NonZero<usize>) -> Option<&'a str> {
    let ip_num = u32::from_be_bytes(ip.octets());
    let ip_buf = unsafe {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},").unwrap_unchecked();

        c.into_inner()
    };

    let (tx, rx) = std::sync::mpsc::channel::<Option<&'a str>>();

    std::thread::scope(|s| {
        chunks(b, workers.get()).for_each(|r| {
            s.spawn({
                let tx = tx.clone();
                move || tx.send(lookup_ipv4_num(&b[r], ip_num, &ip_buf))
            });
        });

        for _ in 0..workers.get() {
            let data = unsafe { rx.recv().unwrap_unchecked() };
            if data.is_some() { return data; }
        }

        None
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

pub fn lookup_ipv4<'a>(b: &'a [u8], ip: &Ipv4Addr) -> Option<&'a str> {
    let ip_num = u32::from_be_bytes(ip.octets());
    let ip_buf = unsafe {
        let mut c = Cursor::new([0u8; 16]);

        write!(&mut c, "{ip_num},").unwrap_unchecked();

        c.into_inner()
    };

    lookup_ipv4_num(b, ip_num, &ip_buf)
}

fn lookup_ipv4_num<'a>(mut b: &'a [u8], ip_num: u32, ip_buf: &[u8]) -> Option<&'a str> {
    let mut best_mask: u32 = 0;

    while !b.is_empty() {
        unsafe {
            let num_mask: u32 = mask_128(b.get_unchecked(..16), ip_buf);

            let nl = find_nl(b);

            if num_mask.trailing_zeros() <= best_mask.trailing_zeros()  {
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

fn into_num(b: &[u8]) -> u32 {
    let mut num = [0u8; 10];
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), num.get_unchecked_mut(10 - b.len()..).as_mut_ptr(), b.len()) }

    let mut head: u16 = unsafe { *(num.as_ptr() as *mut u16) };
    let mut tail: u64 = unsafe { *(num[2..].as_ptr() as *mut u64) };

    head = (head & 0x0F0F).wrapping_mul((10 << 8) + 1) >> 8;

    tail = (tail & 0x0F0F0F0F0F0F0F0F).wrapping_mul((10 << 8) + 1) >> 8;
    tail = (tail & 0x00FF00FF00FF00FF).wrapping_mul((100   << 16) + 1) >> 16;
    tail = (tail & 0x0000FFFF0000FFFF).wrapping_mul((10000 << 32) + 1) >> 32;

    (head as u32 * 100000000) + tail as u32
}

fn value(b: &[u8], n: u32) -> Option<&str> {
    const COMMAS: [u8; 16] = [b','; 16];

    unsafe {
        let mask = mask_128(b.get_unchecked(6..6 + 16), &COMMAS);

        let first: usize = 6 + mask.trailing_zeros() as usize;
        let second: usize = first + 1 + (mask >> (first - 5)).trailing_zeros() as usize;

        let min = into_num(b.get_unchecked(..first));
        let max = into_num(b.get_unchecked(first + 1..second));

        // do not reorder
        if n > max || min > n {
            None
        } else {
            Some(std::str::from_utf8_unchecked(b.get_unchecked(second + 1..b.len() - 1)))
        }
    }
}

#[test]
fn test_find_nl() {
    assert_eq!(find_nl(b"16777472,16778239,CN\n"), 20);
    assert_eq!(find_nl(b"223.255.252.0,223.255.253.255,CN\n"), 32);
    assert_eq!(find_nl(b"2.17.192.0,2.17.192.255,US\n"), 26);
    assert_eq!(find_nl(b"17301760,17302015,38345,Internet Domain Name System Beijing Engineering Resrarch Center Ltd.\n"), 92);
    assert_eq!(find_nl(b"17367040,17432575,4788,TM TECHNOLOGY SERVICES SDN BHD\n17435136,17435391,148000,National Knowledge Network\n"), 53);
}

#[test]
fn test_str_to_num() {
    assert_eq!(into_num(b"16777216"),   16777216);
    assert_eq!(into_num(b"971448832"),  971448832);
    assert_eq!(into_num(b"3758096128"), 3758096128);
}
