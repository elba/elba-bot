# `elba-bot`

`elba-bot` is a bot that made to maintain the [elba package index](https://github.com/elba/index) on Github. It's still in alpha testing phase and may not ready to deploy.

It's lightweight that builds with one command and runs with a single binary. Requires nigthtly rustc.

Build:

```shell
cargo build --release
```

Run:

```shell
target/elba-bot
```

`elba-bot` reads the `.env` in workdir. Fill the file before starting it off.
