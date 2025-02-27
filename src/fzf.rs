use {
  super::{
    argparse::Mode,
    subprocess::{stream_subproc, SubprocCommand},
    types::Die,
  },
  futures::stream::{Stream, StreamExt},
  std::{
    collections::HashMap,
    env::{self, current_exe},
    ffi::OsString,
    io::ErrorKind,
    path::PathBuf,
    process::Stdio,
  },
  tokio::process::Command,
  which::which,
};

async fn reset_term() -> Result<(), Die> {
  if let Ok(path) = which("tput") {
    let status = Command::new(&path)
      .kill_on_drop(true)
      .stdin(Stdio::null())
      .arg("reset")
      .status()
      .await
      .map_err(|e| Die::IO(path, e.kind()))?;

    if status.success() {
      return Ok(());
    }
  }
  if let Ok(path) = which("reset") {
    let status = Command::new(&path)
      .kill_on_drop(true)
      .stdin(Stdio::null())
      .status()
      .await
      .map_err(|e| Die::IO(path, e.kind()))?;
    if status.success() {
      return Ok(());
    }
  }
  Err(Die::IO(PathBuf::from("reset"), ErrorKind::NotFound))
}

pub fn stream_fzf_proc(
  bin: PathBuf,
  args: Vec<String>,
  stream: impl Stream<Item = Result<OsString, Die>> + Unpin,
) -> impl Stream<Item = Result<(), Die>> {
  let execute = format!("become:{}\x04{{+f}}", Mode::PATCH);
  let mut arguments = vec![
    "--read0".to_owned(),
    "--print0".to_owned(),
    "-m".to_owned(),
    "--ansi".to_owned(),
    "--preview-window=70%:wrap".to_owned(),
    format!("--bind=enter:{execute}"),
    format!("--bind=double-click:{execute}"),
    format!("--preview={}\x04{{f}}", Mode::PREVIEW),
  ];
  arguments.extend(args);

  let mut fzf_env = HashMap::new();
  fzf_env.insert(
    Mode::ARGV.to_owned(),
    env::args().collect::<Vec<_>>().join("\x04"),
  );
  fzf_env.insert(
    "SHELL".to_owned(),
    current_exe()
      .or_else(|_| which(env!("CARGO_PKG_NAME")))
      .map_or_else(
        |_| env!("CARGO_PKG_NAME").to_owned(),
        |path| format!("{}", path.display()),
      ),
  );
  fzf_env.insert("LC_ALL".to_owned(), "C.UTF-8".to_owned());

  let cmd = SubprocCommand {
    prog: bin,
    args: arguments,
    env: fzf_env,
  };
  stream_subproc(cmd, stream).then(|row| async {
    match row {
      Ok(o) => Ok(o),
      Err(Die::BadExit(_, 130)) => Err(Die::Interrupt),
      e => {
        let _ = reset_term().await;
        e
      }
    }
  })
}
