//! Handling of errors.

use std::borrow::Cow;
use std::error;
use std::fmt;
use std::io;

/// Error information.
#[derive(Debug)]
pub enum Error {
    MissingArgument,
    Io(io::Error),
    Custom(Cow<'static, str>),
}

impl From<&'static str> for Error {
    fn from(message: &'static str) -> Self {
        Error::Custom(Cow::from(message))
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Custom(Cow::from(message))
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match *self {
            MissingArgument => write!(fmt, "missing argument error"),
            Io(ref error) => write!(fmt, "I/O error: {}", error),
            Custom(ref message) => write!(fmt, "error: {}", message),
        }
    }
}
