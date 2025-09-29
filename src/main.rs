mod error;

use std::fs::File;
use std::io::Write;
use std::net::Ipv4Addr;
use std::num::NonZero;
use std::path::PathBuf;
use std::str::FromStr;

use argh::FromArgs;
use error::Error;
use memmap2::Mmap;

use ip_location_db_lookup::{guess, parallel};

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
        Ok(guess(b, &ip))
    } else {
        let workers = NonZero::try_from(args.workers).or_else(|_| std::thread::available_parallelism().map_err(Error::Workers))?;
        Ok(parallel(b, &ip, workers))
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

#[cfg(test)]
mod tests;
