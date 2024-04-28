# cir - a new implementation of linux infrared tools

aka as "daemon-less lircd, ir-keytable, ir-ctl combined and much more".

For Linux, there are two tools to interact with any infrared hardware:
ir-ctl and ir-keytable. These tools can load simple infrared keymaps
and load decoders, and transmit simple IR. The IR decoders are hardcoded
and a small set is included.

This tool replaced both those tools, but with three major new features:

 - Pronto hex codes
 - IRP support
 - lircd.conf remote definition support

Pronto hex codes are a fairly straightforward way of encoding raw IR,
NEC, RC-5 and a few others.

[IRP](http://hifi-remote.com/wiki/index.php?title=IRP_Notation) is a
DSL language which can
express [any IR protocol](http://hifi-remote.com/wiki/index.php/DecodeIR).
The aim is parse IRP and compile a decoder to BPF. So, any protocol can
be supported directly.

## Listing IR devices

This is the cir equivalent of both `ir-keytable` with no arguments and `ir-ctl -f`.

```
cir config
```

## Transmit/Send

If you have a `.lircd.conf` file or `.toml` keymap, you can send with the following
command:

```
cir transmit keymap foo.lircd.conf KEY_CHANNELUP
```
Alternatively, you can send raw IR directly like so:
```
cir transmit rawir '+9000 -4500 +560'
```
You can also files or linux kernel scancodes, exactly like the `ir-ctl` tool. This supports
mode2 files or raw IR files.
```
cir transmit rawir -s input-file -S nec:0xcafe
```
You can send pronto codes:
```
cir transmit pronto '5000 0073 0000 0001 0001 0001'
```
Lastly you use IRP notation and set the parameters. This is great for experimenting with IRP; use the `--dry-run` (`-n`)
to avoid sending.
```
cir transmit irp -n -fF=2 '{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]'
```

## Decoding

Use this if have a `.lircd.conf` file or `.toml` keymap, and want to decode the IR, without changing
any configation.

```
cir decode keymap foo.lircd.conf
```
This will infrared from the first lirc device. You can also decode IR on the command line or a file.

```
cir decode keymap foo.lircd.conf -r '+9000 -4500 +560'
```
or
```
cir decode keymap foo.lircd.conf -f input-file
```

## Configuration

This is the cir equivalent of `ir-keytable -w`.

```
cir config -s rc0 -w foo.lircd.conf
```
This will generate a BPF decoder for `foo.lircd.conf` and load it.

On startup, `ir-keytable -a -s rc0` read the correct keymap from `/etc/rc_maps.cfg`. 

```
cir auto -s rc0
```

## Test configuration

This is the cir equivalent of `ir-keytable -t`

```
cir test
```

## Status

All the functionality is in place to load keymaps. More tests are needed,
and more polish. The aim is to have this done by the end of 2024.

## Building

On Linux, cir depends on llvm for BPF code generation. On Fedora you
need the `llvm-devel` package install and `llvm-dev` on Ubuntu.

```
cargo install --git https://github.com/seanyoung/cir
```