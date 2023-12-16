//! Like a fork, but cooler.

use std::{
    io::{stdin, IsTerminal},
    process::Stdio,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use tokio::process::Command;

#[derive(Parser)]
#[command(author, version, about = "Like a fork, but cooler")]
struct Cli {
    #[arg(short, long, default_value_t = 8)]
    threads: usize,

    #[arg(short, long, help = "Suppress stdout and stderr")]
    silent: bool,

    #[arg(last = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.threads < 1 {
        return Err(anyhow!("threads must be > 0"));
    }

    if cli.command.is_empty() {
        return Ok(());
    }

    let has_input = !stdin().is_terminal();

    let input_buckets = if has_input {
        // Collect stdin and distribute it into buckets.
        bucket(cli.threads, &mut stdin().lines())?
    } else {
        vec![]
    };

    // Start up child threads.
    let children = (0..cli.threads)
        .map(|thread| {
            let cmd = if has_input {
                let mut cmd = cli.command.clone();
                replace_inputs(&mut cmd, &input_buckets[thread]);
                cmd
            } else {
                cli.command.clone()
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

    // Wait for child threads to complete, maybe print their output.
    for (i, child) in children.into_iter().enumerate() {
        let output = child.wait_with_output().await?;
        if !cli.silent {
            println!(
                "[Thread {i}] exited with code {}:\n{}",
                output.status.code().unwrap_or_default(),
                String::from_utf8(output.stdout)?
            );
        }
    }

    Ok(())
}

/// Group `input` into `num` buckets, striping the elements across so
/// that six elements result in:
/// 1 bucket: [1, 2, 3, 4, 5, 6]
/// 2 buckets: [1, 3, 5] [2, 4, 6]
/// 3 buckets: [1, 4] [2, 5] [3, 6]
fn bucket(
    num: usize,
    input: &mut impl Iterator<Item = Result<String, std::io::Error>>,
) -> Result<Vec<Vec<String>>> {
    let mut buckets: Vec<Vec<String>> = Vec::with_capacity(num);
    for _ in 0..num {
        buckets.push(Vec::new());
    }
    let mut idx = 0;
    for line in input {
        buckets[idx].push(line?);
        idx = (idx + 1) % num;
    }
    Ok(buckets)
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
    fn bucket_stripes_for_one_bucket() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            bucket(1, &mut input).unwrap(),
            vec![vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ],]
        )
    }

    #[test]
    fn bucket_stripes_for_two_buckets() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            bucket(2, &mut input).unwrap(),
            vec![
                vec!["foo".to_string(), "baz".to_string()],
                vec!["bar".to_string()]
            ]
        )
    }

    #[test]
    fn bucket_stripes_for_three_buckets() {
        let mut input = "foo\nbar\nbaz".lines().map(str::to_string).map(Ok);
        assert_eq!(
            bucket(3, &mut input).unwrap(),
            vec![
                vec!["foo".to_string()],
                vec!["bar".to_string()],
                vec!["baz".to_string()],
            ]
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
