# cir - a new implementation of ir-keytable/ir-ctl for linux

For Linux, there are two tools to interact with any infrared hardware:
ir-ctl and ir-keytable. These tools can load simple infrared keymaps
and load decoders, and transmit simple IR. The IR decoders are hardcoded
and a small set is included.

This project plans to completely replace both those tools, but with
three major new features:

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

This is a while away and there is much work to be done.
