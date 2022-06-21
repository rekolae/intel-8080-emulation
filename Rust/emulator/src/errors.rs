use std::fmt::{Display, Formatter, Result, Debug};
use std::io;

pub enum EmulatorError {
    FilePathNotGiven,
    FilePathNotFound(String),
    FileCantOpen(String),
}

fn get_err_msg(err: &EmulatorError) -> String {
    match err {
        EmulatorError::FilePathNotGiven => format!("File path was not given!"),
        EmulatorError::FilePathNotFound(s) => format!("File path '{s}' was not valid!"),
        EmulatorError::FileCantOpen(s) => format!("Couldn't open file '{s}'!"),
    }
}

impl Display for EmulatorError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", get_err_msg(self))
    }
}

impl Debug for EmulatorError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", get_err_msg(self))
    }
}

impl From<io::Error> for EmulatorError {
    fn from(error: io::Error) -> Self {
        EmulatorError::FileCantOpen(error.to_string()) 
    }
}