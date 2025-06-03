# ocularity

This is an experiment. It measures how humans see colour. More specifically it measures what colours humans can distinguish.

The software crowd-sources data from visitors to a web site. After an introduction and a questionnaire, the server repeatedly shows two randomly-generated images and asks the user to choose one.

## Deficiencies

Because the data is crowd-sourced from people on the internet, the experiment is not very well controlled. Aspects out of my control include:

 - The display equipment and configuration
 - The lighting
 - The time of day
 - The background (outside the browser window)
 - The user's mental state (e.g. are they drunk?)
 - distractions, surroundings

## Building

The web server is written in Rust, and can be built using `cargo`:

```sh
cargo build --release
```

## Running

For testing, you can run the server using `cargo`:

```sh
cargo run
```

By default the server listens on `localhost:8081` and appends experimental data to `/tmp/ocularity.log`. These values can be configured as described below.

For production, you can run the server using `systemd` as described below, so that it continues running when you log out, and restarts itself automatically if there is an error.

## Configuration

The server can be configured using three environment variables:

- OCULARITY_RESULTS - the filename of the results file (default: `/tmp/ocularity.log`)
- OCULARITY_ADDRESS - the host and port to listen on (default: `localhost:8081`
- OCULARITY_BASE_URL - the globally visible URL of the web server (default: `http://SERVER_ADDRESS`)

One way that the `BASE_URL` can differ from the `ADDRESS` is if the server is exposed to the internet via a proxy.

## Systemd configuration

The file `ocularity.service` belongs in `$HOME/.config/systemd/user/`. Edit it as needed.

Run the following commands:

```sh
loginctl enable-linger
systemctl --user --enable ocularity.service
systemctl --user start ocularity.service
```

Replace `start` with `stop`, `restart` or `status` as appropriate. Systemd will log `stdout` and `stderr` by default, which makes for a usable server log. View it with

```sh
journalctl --user-unit ocularity
```