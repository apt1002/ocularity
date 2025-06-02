# ocularity
Colour perception experiment

## Systemd configuration

The file `ocularity.service` belongs in `$HOME/.config/systemd/user/`. Run the following commands:

```sh
loginctl enable-linger
systemctl --user --enable ocularity.service
systemctl --user start ocularity.service
```

Replace `start` with `stop`, `restart` or `status` as appropriate. Systemd will log `stdout` and `stderr` by default, which makes for a usable server log. View it with

```sh
journalctl --user-unit ocularity
```