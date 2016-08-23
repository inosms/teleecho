extern crate telegram_bot;
extern crate serde_json;
use std::fmt;


/// teleecho error type
pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(::std::io::Error),
    Json(self::serde_json::error::Error),
    TelegramBot(self::telegram_bot::Error),
    ConfigEntryExists,
    ConfigConnectionNotExist,
    ConfigNotUniqueConnection,
    HomePathNotFound,
}



impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref e) => e.description(),
            Error::Json(ref e) => e.description(),
            Error::TelegramBot(ref e) => e.description(),
            Error::ConfigEntryExists => "entry already exists",
            Error::ConfigConnectionNotExist => "specified connection does not exist",
            Error::ConfigNotUniqueConnection => {
                "empty connection name can only be specified, when exactly one connecition is \
                 registered"
            }
            Error::HomePathNotFound => "unable to retreive home path",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref e) => e.fmt(f),
            Error::Json(ref e) => e.fmt(f),
            Error::TelegramBot(ref e) => e.fmt(f),
            Error::ConfigEntryExists => "entry already exists".fmt(f),
            Error::ConfigConnectionNotExist => "specified connection does not exist".fmt(f),
            Error::ConfigNotUniqueConnection => {
                "empty connection name can only be specified, when exactly one connecition is \
                 registered"
                    .fmt(f)
            }
            Error::HomePathNotFound => "unable to retreive home path".fmt(f),
        }
    }
}


macro_rules! from_impl {
    ($ty:path, $variant:ident) => (
        impl From<$ty> for Error {
            fn from(e: $ty) -> Self {
                Error::$variant(e)
            }
        }
    )
}

from_impl!(::std::io::Error, Io);
from_impl!(self::serde_json::error::Error, Json);
from_impl!(self::telegram_bot::Error, TelegramBot);
