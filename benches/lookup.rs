use std::{fs::File, hint::black_box, net::Ipv4Addr, num::NonZero};

use gungraun::{library_benchmark, library_benchmark_group, main};
use memmap2::Mmap;

use ip_location_db_lookup::{guess, lookup_ipv4, parallel};

const DE: Ipv4Addr = Ipv4Addr::new(217, 195,  14,   20);
const ZW: Ipv4Addr = Ipv4Addr::new( 41, 220,  29,   40);
const US: Ipv4Addr = Ipv4Addr::new( 65, 212,  65,  188);
const FR: Ipv4Addr = Ipv4Addr::new( 88, 138,  12,  121);
const MG: Ipv4Addr = Ipv4Addr::new(102,  17, 119,  231);
const AU: Ipv4Addr = Ipv4Addr::new(223, 255, 255,  255);

fn city_ipv4() -> Mmap {
    let file = File::open("dbip-city-ipv4-num.csv").unwrap();
    unsafe { Mmap::map(&file).unwrap() }
}

fn country_ipv4() -> Mmap {
    let file = File::open("dbip-country-ipv4-num.csv").unwrap();
    unsafe { Mmap::map(&file).unwrap() }
}

#[library_benchmark]
#[bench::city_de(city_ipv4(), DE)]
fn bench_lookup_ipv4(b: Mmap, ip: Ipv4Addr) {
    black_box(lookup_ipv4(&b, &ip)).unwrap();
}

#[library_benchmark]
#[benches::all(iter = [DE, ZW, US, FR, MG, AU])]
fn bench_city_guess(ip: Ipv4Addr) {
    black_box(guess(&city_ipv4(), &ip)).unwrap();
}

#[library_benchmark]
#[benches::all(iter = [DE, ZW, US, FR, MG, AU])]
fn bench_country_guess(ip: Ipv4Addr) {
    black_box(guess(&country_ipv4(), &ip)).unwrap();
}

#[library_benchmark]
#[benches::de(iter = [0, 2, 3])]
fn bench_city_parallel(workers: usize) {
    let workers = NonZero::try_from(workers).or_else(|_| std::thread::available_parallelism()).unwrap();
    black_box(parallel(&city_ipv4(), &DE, workers)).unwrap();
}

#[library_benchmark]
#[benches::de(iter = [0, 2, 3])]
fn bench_country_parallel(workers: usize) {
    let workers = NonZero::try_from(workers).or_else(|_| std::thread::available_parallelism()).unwrap();
    black_box(parallel(&country_ipv4(), &DE, workers)).unwrap();
}

library_benchmark_group!(
    name = group;
    benchmarks = bench_lookup_ipv4, bench_city_guess, bench_country_guess, bench_city_parallel
);

main!(library_benchmark_groups = group);
