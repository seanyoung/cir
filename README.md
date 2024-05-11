# cir - a new implementation of linux infrared tools

For Linux, there are tools for infrared: `ir-ctl` and `ir-keytable`. These
tools can load simple infrared keymaps and load decoders, and transmit simple
IR. The IR decoders and encoders are hardcoded.
There is also the lirc daemon and its tools, which supports many more IR
protocols but certainly not all.

This tool replaces all those tools, but with major new features:

 - Pronto hex codes
 - IRP support
 - lircd.conf remote definition support
 - daemon-less (using BPF)

Pronto hex codes are a fairly straightforward way of encoding raw IR,
NEC, RC-5 and a few others.

[IRP Notation](http://hifi-remote.com/wiki/index.php?title=IRP_Notation) is a
DSL language which can
express [any IR protocol](http://hifi-remote.com/wiki/index.php/DecodeIR).
We can parse IRP and compile a decoder to BPF using LLVM. So, any protocol can
be supported directly.

## List devices (cir list)

This gives you a list of infrared and CEC devices on your Linux system. It is the cir equivalent of both
`ir-keytable` with no arguments combined with the lirc features from `ir-ctl -f`.

```bash
cir list
```
This gives you a list of the all the rc devices on the system.
```
rc0:
        Device Name             : Media Center Ed. eHome Infrared Remote Transceiver (1784:0008)
        Driver                  : mceusb
        Default Keymap          : rc-rc6-mce
        Input Device            : /dev/input/event12
        Input properties        : Permission denied (os error 13)
        LIRC Device             : /dev/lirc0
        LIRC Features           : Permission denied (os error 13)
        Supported Protocols     : rc-5 nec rc-6 jvc sony rc-5-sz sanyo sharp mce_kbd xmp imon rc-mm
        Enabled Protocols       : rc-6
```
Not all information is available, unless you run it as root.
```
rc0:
        Device Name             : Media Center Ed. eHome Infrared Remote Transceiver (1784:0008)
        Driver                  : mceusb
        Default Keymap          : rc-rc6-mce
        Input Device            : /dev/input/event10
        Bus                     : USB
        Vendor/product          : 1784:0008 version 0x0101
        Repeat                  : delay 500 ms, period 125 ms
        LIRC Device             : /dev/lirc0
        LIRC Receiver           : raw receiver
        LIRC Resolution         : 50 microseconds
        LIRC Timeout            : 125000 microseconds
        LIRC Timeout Range      : 50 to 1250000 microseconds
        LIRC Wideband Receiver  : yes
        LIRC Measure Carrier    : yes
        LIRC Transmitter        : yes
        LIRC Set Tx Carrier     : yes
        LIRC Set Tx Duty Cycle  : no
        LIRC Transmitters       : 2
        BPF protocols           : 
        Supported Protocols     : rc-5 nec rc-6 jvc sony rc-5-sz sanyo sharp mce_kbd xmp imon rc-mm
        Enabled Protocols       : 
```

## Transmit/Send (cir transmit)

If you have a `.lircd.conf` file or `.toml` keymap, you can transmit with the following
command:

```bash
cir transmit keymap RM-Y173.lircd.conf KEY_CHANNELUP
```
Alternatively, you can send raw IR directly like so:
```bash
cir transmit rawir '+9000 -4500 +560'
```
You can also send files or linux kernel scancodes, using the same options like `ir-ctl`. This supports
mode2 files or raw IR files.
```bash
cir transmit rawir -s input-file -S nec:0xcafe
```
You can send pronto codes:
```bash
cir transmit pronto '5000 0073 0000 0001 0001 0001'
```
Lastly you use IRP notation and set the parameters. This is great for experimenting with IRP; use the `--dry-run` (`-n`)
to avoid sending.
```bash
cir transmit irp -n -fF=2 '{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]'
```
This will give the following output:
```
info: carrier: 40000Hz
info: rawir: +2400 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -32400
```

## Decoding (cir decode)

Use this if have a `.lircd.conf` file or `.toml` keymap, and want to decode the IR. This does not change
any configation.

```bash
cir decode keymap foo.lircd.conf
```
This will infrared from the first lirc device. You can also decode IR on the command line or a file.

```bash
cir decode keymap foo.lircd.conf -r '+9000 -4500 +560'
```
or
```bash
cir decode keymap foo.lircd.conf -f input-file
```
If you wish to decode using IRP Notation that is possible too:

```bash
cir decode irp '{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]'
```
Like above the input can be from a lirc device (optionally specify the device with
`-d /dev/lirc1` or `-s rc`), on the command line (`-r '+100 -200 +100'`) or a file (`-f filename`).

## Loading of keymaps

This is the cir equivalent of `ir-keytable -w`, however cir can not just load keymaps, it can
also load `.lircd.conf` files.

```bash
cir load -s rc0 foo.lircd.conf
```
This will generate a BPF decoder for `foo.lircd.conf` and load it.

On startup, `ir-keytable -a -s rc0` read the correct keymap from `/etc/rc_maps.cfg`.

```bash
cir auto -s rc0
```
## Configuration (cir config)

`cir config` is usually not needed, this for tweaking things like auto-repeat or lirc timeout. For example:
```bash
cir config -P 125 -D 500
```

## Test configuration (cir test)

This is the cir equivalent of `ir-keytable -t`

```bash
cir test
```

## Status

All the functionality is in place to load keymaps. More tests are needed,
and more polish. The aim is to have this done by the end of 2024.

## Building

On Linux, cir depends on llvm for BPF code generation. On Fedora you
need the `llvm-devel` package install and `llvm-dev` on Ubuntu.

```
cargo install --git https://github.com/seanyoung/cir cir
```

## Tests

- The IRP encoder and decoder is compared against IrpTransmogrifier with a large set of inputs.
- The parsing, encoding and decoding of lircd.conf files is compared against lirc (see liblircd)
- The encoding of linux protocols is compared against ir-ctl (see libirctl)
- There are many more tests
