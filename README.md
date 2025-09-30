# ip-location-db-lookup
![Toolchain: Stable](https://img.shields.io/badge/Toolchain-Stable-%23D0F0C0) [![License: MIT](https://img.shields.io/badge/License-MIT-%23F0E68C)](https://opensource.org/licenses/MIT) 

Offline command line utility for IP address look-up through CSV databases from [ip-location-db](https://github.com/sapics/ip-location-db) project.

This is my personal text file parsing optimization sandbox, don't use for critical things.

## File Format
Only `-num` files are supported.  

`ip_range_start,ip_range_end,(VALUE)`

## Usage

```sh
Usage: ip-location-db-lookup [-w <workers>] [DATABASE] [IPv4]

Offline IP address lookup tool.

Options:
  -w, --workers     amount of workers (default: 1)
  --help            display usage information
```

```sh
alias country="ip-location-db-lookup ${HOME}/databases/dbip-country-ipv4-num.csv ${1}"
alias city="ip-location-db-lookup ${HOME}/databases/dbip-city-ipv4-num.csv ${1}"
```

## Building
To build this little thing, you'll need some [Rust](https://www.rust-lang.org/).

```sh
git clone --depth 1 https://github.com/Elvyria/ip-location-db-lookup
cd ip-location-db-lookup
cargo build --locked --release
```
or
```
RUSTFLAGS="-C target-cpu=native" cargo build --locked --release
```

## Tested with:
* [x] [`dbip-country-ipv4-num.csv`](https://github.com/sapics/ip-location-db/blob/main/dbip-country/dbip-country-ipv4-num.csv)
* [x] [`dbip-city-ipv4-num.csv`](https://github.com/sapics/ip-location-db/blob/main/dbip-city/dbip-city-ipv4-num.csv.gz)
* [x] [`dbip-asn-ipv4-num.csv`](https://github.com/sapics/ip-location-db/blob/main/dbip-asn/dbip-asn-ipv4-num.csv)
