use super::argparse::Arguments;
use super::types::{Abort, Fail};
use super::udiff::DiffRange;
use async_channel::{bounded, Receiver};
use regex::Regex;
use std::{
  collections::{HashMap, HashSet},
  ffi::OsString,
  io::ErrorKind,
  os::unix::ffi::OsStringExt,
  path::{Path, PathBuf},
};
use tokio::{
  fs::{canonicalize, File},
  io::{self, AsyncBufReadExt, BufReader},
  select,
  task::{spawn, JoinHandle},
};

#[derive(Debug)]
pub enum Payload {
  Entire(PathBuf),
  Piecewise(PathBuf, HashSet<DiffRange>),
}

struct DiffLine(PathBuf, DiffRange);

fn p_line(line: String) -> Result<DiffLine, Fail> {
  let f = Fail::ArgumentError(String::new());
  let preg = "\n\n\n\n@@ -(\\d+),(\\d+) \\+(\\d+),(\\d+) @@$";
  let re = Regex::new(preg).map_err(Fail::RegexError)?;
  let captures = re.captures(&line).ok_or_else(|| f.clone())?;

  let before_start = captures
    .get(1)
    .ok_or_else(|| f.clone())?
    .as_str()
    .parse::<usize>()
    .map_err(|_| f.clone())?;
  let before_inc = captures
    .get(2)
    .ok_or_else(|| f.clone())?
    .as_str()
    .parse::<usize>()
    .map_err(|_| f.clone())?;
  let after_start = captures
    .get(3)
    .ok_or_else(|| f.clone())?
    .as_str()
    .parse::<usize>()
    .map_err(|_| f.clone())?;
  let after_inc = captures
    .get(4)
    .ok_or_else(|| f.clone())?
    .as_str()
    .parse::<usize>()
    .map_err(|_| f.clone())?;

  let range = DiffRange {
    before: (before_start - 1, before_inc),
    after: (after_start - 1, after_inc),
  };
  let path = PathBuf::from(String::from(re.replace(&line, "")));
  Ok(DiffLine(path, range))
}

async fn read_patches(path: &Path) -> Result<HashMap<PathBuf, HashSet<DiffRange>>, Fail> {
  let fd = File::open(path)
    .await
    .map_err(|e| Fail::IO(path.to_owned(), e.kind()))?;
  let mut reader = BufReader::new(fd);
  let mut acc = HashMap::<PathBuf, HashSet<DiffRange>>::new();

  loop {
    let mut buf = Vec::new();
    let n = reader
      .read_until(b'\0', &mut buf)
      .await
      .map_err(|e| Fail::IO(path.to_owned(), e.kind()))?;

    match n {
      0 => break,
      _ => {
        buf.pop();
        let line =
          String::from_utf8(buf).map_err(|_| Fail::IO(path.to_owned(), ErrorKind::InvalidData))?;
        let patch = p_line(line)?;
        match acc.get_mut(&patch.0) {
          Some(ranges) => {
            ranges.insert(patch.1);
          }
          None => {
            let mut ranges = HashSet::new();
            ranges.insert(patch.1);
            acc.insert(patch.0, ranges);
          }
        }
      }
    }
  }

  Ok(acc)
}

fn stream_patch(abort: &Abort, patch: &Path) -> (JoinHandle<()>, Receiver<Payload>) {
  let abort = abort.clone();
  let patch = patch.to_owned();
  let (tx, rx) = bounded::<Payload>(1);

  let handle = spawn(async move {
    match read_patches(&patch).await {
      Ok(patches) => {
        for patch in patches {
          if tx.send(Payload::Piecewise(patch.0, patch.1)).await.is_err() {
            let _ = abort.send(Fail::Join);
            break;
          }
        }
      }
      Err(err) => {
        let _ = abort.send(err);
      }
    }
  });
  (handle, rx)
}

fn stream_stdin(abort: &Abort, use_nul: bool) -> (JoinHandle<()>, Receiver<Payload>) {
  let (tx, rx) = bounded::<Payload>(1);

  let abort = abort.clone();
  let handle = spawn(async move {
    if atty::is(atty::Stream::Stdin) {
      let _ = abort.send(Fail::ArgumentError(
        "/dev/stdin connected to tty".to_owned(),
      ));
    } else {
      let delim = if use_nul { b'\0' } else { b'\n' };
      let mut on_abort = abort.subscribe();
      let mut reader = BufReader::new(io::stdin());
      let mut seen = HashSet::new();

      loop {
        let mut buf = Vec::new();
        select! {
          _ = on_abort.recv() => break,
          n = reader.read_until(delim, &mut buf) => {
            match n {
              Ok(0) => break,
              Ok(_) => {
                buf.pop();
                let path = PathBuf::from(OsString::from_vec(buf));
                match canonicalize(&path).await {
                  Ok(canonical) => {
                    if seen.insert(canonical.clone()) &&
                       tx.send(Payload::Entire(canonical)).await.is_err() {
                        let _ = abort.send(Fail::Join);
                        break
                    }
                  },
                  Err(err) => {
                    let _ = abort.send(Fail::IO(path, err.kind()));
                    break;
                  }
                }
              }
              Err(err) => {
                let _ = abort.send(Fail::IO(PathBuf::from("/dev/stdin"), err.kind()));
                break;
              }
            }
          }
        }
      }
    }
  });
  (handle, rx)
}

pub fn stream_input(abort: &Abort, args: &Arguments) -> (JoinHandle<()>, Receiver<Payload>) {
  if let Some(preview) = &args.internal_preview {
    stream_patch(abort, preview)
  } else if let Some(patch) = &args.internal_patch {
    stream_patch(abort, patch)
  } else {
    stream_stdin(abort, args.nul_delim)
  }
}
