# IRP

A Rust library for handling [Consumer IR](https://en.wikipedia.org/wiki/Consumer_IR), e.g. the infrared protocol a TV remote uses. This library supports
 [IRP Notation](http://hifi-remote.com/wiki/index.php?title=IRP_Notation),
[Pronto Hex](http://www.hifi-remote.com/wiki/index.php?title=Working_With_Pronto_Hex), and common IR encodings like raw IR and lirc's mode2 pulse/space format.

See the [docs](https://docs.rs/irp/) for the usage and some examples.

Currently IRP can be encoded to either raw IR or pronto hex. Decoding is done
via NFA state machine.
