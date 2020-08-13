use pcp::types::ResultCode;
use std::sync::mpsc::RecvError;
use std::{io, net};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        use crate::win;

        #[derive(Debug)]
        pub enum OsSpecific {
            WinAPIError(win::ResultCode),
        }

        impl From<OsSpecific> for Error {
            fn from(value: OsSpecific) -> Self {
                Self::OsError("Windows", value)
            }
        }

        impl From<win::ResultCode> for Error {
            fn from(value: win::ResultCode) -> Self {
                OsSpecific::WinAPIError(value).into()
            }
        }

    } else if #[cfg(target_os = "linux")] {
        #[derive(Debug)]
        pub enum OsSpecific {
            LinuxAPIError(nix::Error),
        }

        impl From<OsSpecific> for Error {
            fn from(value: OsSpecific) -> Self {
                Self::OsError("Linux", value)
            }
        }

        impl From<nix::Error> for Error {
            fn from(value: nix::Error) -> Self {
                OsSpecific::LinuxAPIError(value).into()
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O operation failed: `{0}`")]
    IoError(io::Error),

    #[error("PCP operation failed: `{0}`")]
    PCPError(ResultCode),

    #[error("Internal PCP error: `{0}`")]
    RecvError(RecvError),

    // TODO: Is it really needed?
    #[error("[{0} specific error] {1:?}")]
    OsError(&'static str, OsSpecific),

    #[error("An error occurred: `{0}`")]
    Other(&'static str),

    #[error("Address resolution failed")]
    AddressNotFound,

    #[error("Address parsing failed: `{0}`")]
    ParseError(net::AddrParseError),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}
impl From<ResultCode> for Error {
    fn from(value: ResultCode) -> Self {
        Self::PCPError(value)
    }
}
impl From<RecvError> for Error {
    fn from(value: RecvError) -> Self {
        Self::RecvError(value)
    }
}
impl From<net::AddrParseError> for Error {
    fn from(value: net::AddrParseError) -> Self {
        Self::ParseError(value)
    }
}
impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::Other(value)
    }
}
