use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub enum FsError {
    InvalidValue(String),
    InvalidType(String),
}

impl Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            FsError::InvalidValue(err_msg) => write!(f, "InvalidValue: {}", err_msg),
            FsError::InvalidType(err_msg) => write!(f, "InvalidType: {}", err_msg),
        }
    }
}
