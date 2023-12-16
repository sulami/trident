# Trident

_Like a fork, but cooler._

A way to bolt on effective parallelism to other tools. `trident`
behaves similarly to `xargs`, running a command N times in parallel.
Contrary to `xargs` though, `trident` stripes the input lines into N
buckets, running each child process with its portion of the inputs at
once.

In some cases this can be much faster than both extremes, running the
entire input in one single process, or running one process per input.

## Usage

Use `{}` as the placeholder for arguments. All arguments will be
dropped in (as separate arguments) in this position.

```bash
cat files | trident -- cat {}
```

Get a full reference of options available from the help function:

```
trident -h
```
