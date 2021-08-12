use super::argparse::{Action, Options, Printer};
use super::fzf::run_fzf;
use super::types::{Abort, Task};
use ansi_term::Colour;
use async_channel::Receiver;
use futures::future::try_join;
use std::process;
use tokio::{
  io::{self, AsyncWriteExt, BufWriter},
  select, task,
};

fn stream_stdout(abort: Abort, stream: Receiver<String>) -> Task {
  let mut stdout = BufWriter::new(io::stdout());
  task::spawn(async move {
    loop {
      select! {
        _ = abort.rx.changed() => {
          break
        },
        print = stream.recv() => {
        match print {
          Ok(val) => match stdout.write(val.as_bytes()).await {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
              abort.tx.send(Failure::Interrupt).await.expect("<CHANNEL>");
              break;
            }
            Err(e) => {
              abort.tx.send(e).await.expect("<CHANNEL>");
              break;
            }
            _ => {}
          },
          Err(e) => {
            abort.tx.send(e).await.expect("<CHANNEL>");
            break;
          }
        }
        }
      }
    }
  })
}

pub fn stream_output(abort: Abort, opts: Options, stream: Receiver<String>) -> Task {
  match (&opts.action, &opts.printer) {
    (Action::Fzf(fzf_p, fzf_a), _) => run_fzf(fzf_p.to_owned(), fzf_a.to_owned(), stream),
    (_, Printer::Pager(cmd)) => cmd.stream(stream),
    (_, Printer::Stdout) => stream_stdout(stream),
  }
}
