use ansi_term::Colour;
use std::{io, process, string};
use tokio::{sync::mpsc::error::SendError, task::JoinError};

/*
 * Consolidate Error Handling
 * ==========================
 */

#[derive(Debug)]
pub enum Failure {
  Simple(String),
  IO(io::Error),
  Str(string::FromUtf8Error),
  Regex(regex::Error),
  SendError,
  JoinError,
}

pub type SadResult<T> = Result<T, Failure>;

pub trait SadnessFrom<T> {
  fn into_sadness(self) -> SadResult<T>;
}

impl<T, E: Into<Failure>> SadnessFrom<T> for Result<T, E> {
  fn into_sadness(self) -> SadResult<T> {
    match self {
      Ok(val) => Ok(val),
      Err(e) => Err(e.into()),
    }
  }
}

/* ==========================
 * Consolidate Error Handling
 */

impl From<io::Error> for Failure {
  fn from(err: io::Error) -> Self {
    Failure::IO(err)
  }
}

impl From<string::FromUtf8Error> for Failure {
  fn from(err: string::FromUtf8Error) -> Self {
    Failure::Str(err)
  }
}

impl From<regex::Error> for Failure {
  fn from(err: regex::Error) -> Self {
    Failure::Regex(err)
  }
}

impl<T> From<SendError<T>> for Failure {
  fn from(_: SendError<T>) -> Self {
    Failure::SendError
  }
}

impl From<JoinError> for Failure {
  fn from(_: JoinError) -> Self {
    Failure::JoinError
  }
}

/* Exit */

pub fn err_exit(err: Failure) -> ! {
  eprintln!("{}", Colour::Red.paint(format!("{:#?}", err)));
  process::exit(1)
}
