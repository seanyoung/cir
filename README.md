# cir - a new implementation of ir-keytable/ir-ctl for linux

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

## Status

All the functionality is in place to load keymaps. More tests are needed,
and more polish. The aim is to have this done by the end of 2024.

## Building

On Linux, cir depends on llvm for BPF code generation. On Fedora you
need the `llvm-devel` package install and `llvm-dev` on Ubuntu.