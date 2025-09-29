use std::{fs::File, hint::black_box, net::Ipv4Addr, num::NonZero};

use gungraun::{library_benchmark, library_benchmark_group, main};
use memmap2::Mmap;

use ip_location_db_lookup::{guess, lookup_ipv4, parallel};

const GB: Ipv4Addr = Ipv4Addr::new(217, 195, 14, 20);

fn city_ipv4() -> Mmap {
    let file = File::open("dbip-city-ipv4-num.csv").unwrap();
    unsafe { Mmap::map(&file).unwrap() }
}

#[library_benchmark]
#[bench::city_gb(city_ipv4(), GB)]
fn bench_lookup_ipv4(b: Mmap, ip: Ipv4Addr) {
    black_box(lookup_ipv4(&b, &ip)).unwrap();
}

#[library_benchmark]
#[bench::city_gb(city_ipv4(), GB)]
fn bench_guess(b: Mmap, ip: Ipv4Addr) {
    black_box(guess(&b, &ip)).unwrap();
}

#[library_benchmark]
#[bench::city_two(city_ipv4(), GB, 2)]
#[bench::city_three(city_ipv4(), GB, 3)]
#[bench::city_all(city_ipv4(), GB, 0)]
fn bench_parallel(b: Mmap, ip: Ipv4Addr, workers: usize) {
    let workers = NonZero::try_from(workers).or_else(|_| std::thread::available_parallelism()).unwrap();
    black_box(parallel(&b, &ip, workers)).unwrap();
}

library_benchmark_group!(
    name = group;
    benchmarks = bench_lookup_ipv4, bench_guess, bench_parallel
);

main!(library_benchmark_groups = group);
