use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("couldn't open file for reading: {0:?}")]
    FileOpen(std::io::Error),

    #[error("couldn't map file to memory: {0:?}")]
    Mmap(std::io::Error),

    #[error(transparent)]
    IP(std::net::AddrParseError),

    #[error("couldn't automatically detect the amount of available workers: {0:?}")]
    Workers(std::io::Error),

    #[error("")]
    NotFound,
}
