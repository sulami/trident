//! Like a fork, but cooler.

use std::{
    io::stdin,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Result};
use clap::Parser;
use rayon::prelude::*;

#[derive(Parser)]
#[command(author, version, about = "Like a fork, but cooler")]
struct Cli {
    #[arg(short, long, default_value_t = 8)]
    threads: usize,

    #[arg(short, long, help = "Suppress stdout and stderr")]
    silent: bool,

    #[arg(last = true)]
    command: Vec<String>,

    #[arg(short, long, help = "Distribution mode", default_value_t = Mode::Stripe)]
    mode: Mode,
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Stripe,
    Chunk,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Stripe => write!(f, "stripe"),
            Mode::Chunk => write!(f, "chunk"),
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stripe" => Ok(Mode::Stripe),
            "chunk" => Ok(Mode::Chunk),
            _ => Err(anyhow!("valid modes are: stripe, chunk")),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.threads < 1 {
        return Err(anyhow!("threads must be > 0"));
    }

    if cli.command.is_empty() {
        return Ok(());
    }

    // Collect stdin and distribute it into buckets.
    let input_buckets = match cli.mode {
        Mode::Stripe => stripe(cli.threads, &mut stdin().lines())?,
        Mode::Chunk => chunk(cli.threads, &mut stdin().lines())?,
    };

    // Start up child threads.
    let children = input_buckets
        .iter()
        .map(|bucket| {
            let cmd = {
                let mut cmd = cli.command.clone();
                replace_inputs(&mut cmd, bucket);
                cmd
            };

            let mut command = Command::new(&cmd[0]);
            command.args(&cmd[1..]);
            if cli.silent {
                command.stdout(Stdio::null());
                command.stderr(Stdio::null());
            } else {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
            }
            command.spawn()
        })
        .collect::<Result<Vec<_>, _>>()?;

    children.into_par_iter().enumerate().for_each(|(i, child)| {
        let output = child.wait_with_output().expect("failed to wait on child");
        if !cli.silent {
            println!(
                "[Thread {i}] exited with code {}:\n{}",
                output.status.code().unwrap_or_default(),
                String::from_utf8(output.stdout).expect("failed to parse output")
            );
        }
    });

    Ok(())
}

/// Group `input` into `num` buckets, striping the elements across so
/// that six elements result in:
/// 1 bucket: [1, 2, 3, 4, 5, 6]
/// 2 buckets: [1, 3, 5] [2, 4, 6]
/// 3 buckets: [1, 4] [2, 5] [3, 6]
fn stripe(
    num: usize,
    input: &mut impl Iterator<Item = Result<String, std::io::Error>>,
) -> Result<Vec<Vec<String>>> {
    let mut buckets: Vec<Vec<String>> = Vec::with_capacity(num);
    let mut idx = 0;
    for line in input {
        if buckets.len() < idx + 1 {
            buckets.push(Vec::new());
        }
        buckets[idx].push(line?);
        idx = (idx + 1) % num;
    }
    Ok(buckets)
}

/// Group `input` into `num` buckets, chunking the elements across so
/// that six elements result in:
/// 1 bucket: [1, 2, 3, 4, 5, 6]
/// 2 buckets: [1, 2, 3] [4, 5, 6]
/// 3 buckets: [1, 2] [3, 4] [5, 6]
fn chunk(
    num: usize,
    input: &mut impl Iterator<Item = Result<String, std::io::Error>>,
) -> Result<Vec<Vec<String>>> {
    let lines = input
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(anyhow::Error::new)?;
    Ok(lines
        .chunks(lines.len().div_ceil(num))
        .map(Vec::from)
        .collect())
}

/// Replace the first instance of "{}" in `cmd` with all the items in
/// `substitutes`.
fn replace_inputs(cmd: &mut Vec<String>, substitutes: &[String]) {
    if let Some(idx) = cmd.iter().position(|part| *part == "{}") {
        cmd.remove(idx);
        for item in substitutes.iter().rev() {
            cmd.insert(idx, item.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripe_stripes_for_one_bucket() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            stripe(1, &mut input).unwrap(),
            vec![vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ],]
        )
    }

    #[test]
    fn stripe_stripes_for_two_buckets() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            stripe(2, &mut input).unwrap(),
            vec![
                vec!["foo".to_string(), "baz".to_string()],
                vec!["bar".to_string()]
            ]
        )
    }

    #[test]
    fn stripe_stripes_for_three_buckets() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            stripe(3, &mut input).unwrap(),
            vec![
                vec!["foo".to_string()],
                vec!["bar".to_string()],
                vec!["baz".to_string()],
            ]
        )
    }

    #[test]
    fn stripe_does_not_create_empty_buckets() {
        let mut input = "foo\nbar".lines().map(str::to_string).map(Ok);
        assert_eq!(
            stripe(3, &mut input).unwrap(),
            vec![vec!["foo".to_string()], vec!["bar".to_string()]]
        )
    }

    #[test]
    fn chunks_chunks_for_one_bucket() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            chunk(1, &mut input).unwrap(),
            vec![vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ],]
        )
    }

    #[test]
    fn chunks_chunks_foor_two_buckets() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            chunk(2, &mut input).unwrap(),
            vec![
                vec!["foo".to_string(), "bar".to_string()],
                vec!["baz".to_string()]
            ]
        )
    }

    #[test]
    fn chunks_does_not_create_empty_buckets() {
        let mut input = "foo\nbar".lines().map(str::to_string).map(Ok);
        assert_eq!(
            chunk(3, &mut input).unwrap(),
            vec![vec!["foo".to_string()], vec!["bar".to_string()]]
        )
    }

    #[test]
    fn replace_inputs_without_match_is_noop() {
        let mut cmd = vec!["foo".to_string(), "bar".to_string()];
        replace_inputs(&mut cmd, &["baz".to_string()]);
        assert_eq!(cmd, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn replace_inputs_inserts_substitutes() {
        let mut cmd = vec!["foo".to_string(), "{}".to_string(), "bar".to_string()];
        replace_inputs(&mut cmd, &["baz".to_string(), "qux".to_string()]);
        assert_eq!(
            cmd,
            vec![
                "foo".to_string(),
                "baz".to_string(),
                "qux".to_string(),
                "bar".to_string()
            ]
        );
    }
}
