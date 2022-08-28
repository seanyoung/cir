# IRP

This is a rust libary for encoding and decoding
[Infrared](https://en.wikipedia.org/wiki/Consumer_IR) protocols using
[IRP Notation](http://hifi-remote.com/wiki/index.php?title=IRP_Notation) or
[Pronto Hex](http://www.hifi-remote.com/wiki/index.php?title=Working_With_Pronto_Hex).
Remote controls for TVs, Hifi sets, air conditioning units, etc. send messages encoded
using many different protocols. Using this library you can decode the IR
received from a remote control, or encode IR into the same format as a remote
control.

This library only deals with the encoding and decoding, and does not know anything
about talking to infrared devices; you can use thee [cir crate](https://crates.io/crates/cir)
for that on Linux. You will also need an IRP definition or Pronto hex definition
of your remote protocol. There is a long list of IRP definitions maintained by
[IrpTransmogrifier](https://github.com/bengtmartensson/IrpTransmogrifier/blob/master/src/main/resources/IrpProtocols.xml) and on
[hifi-remote](http://hifi-remote.com/wiki/index.php/DecodeIR).

There are also some utility functions for parsing raw ir and mode2 output.

See the [docs](https://docs.rs/irp/) to see the complete interface, or use the
examples below.

## Encoding IRP

This example encodes an button press using NEC encoding, encodes and then simply prints the encoded result.

```rust
use irp::{Irp, Vartable};

fn main() {
    let mut vars = Vartable::new();
    // set D to 255, bit width 8
    vars.set(String::from("D"), 255, 8);
    vars.set(String::from("S"), 52, 8);
    vars.set(String::from("F"), 1, 8);
    // nec protocol
    let irp = Irp::parse(r#"
        {38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m,(16,-4,1,^108m)*)
        [D:0..255,S:0..255=255-D,F:0..255]"#)
        .expect("parse should succeed");
    // encode message with 0 repeats
    let message = irp.encode(vars, 0).expect("encode should succeed");
    if let Some(carrier) = &message.carrier {
        println!("carrier: {}Hz", carrier);
    }
    if let Some(duty_cycle) = &message.duty_cycle {
        println!("duty cycle: {}%", duty_cycle);
    }
    println!("{}", message.print_rawir());
}
```

The output is in raw ir format, which looks like so:

```ignore
carrier: 38400Hz
+9024 -4512 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -36372
```

Each entry is a duration in microseconds, and prefixed with `+` for *flash*
(infrared light on) and `-` for *gap* for infrared light off. This is also known
as *pulse* and *space*.

## Encoding Pronto Hex

The [Pronto Hex](http://www.hifi-remote.com/wiki/index.php?title=Working_With_Pronto_Hex) is made popular by the
Philips Pronto universal remote. The format is a series of 4 digits hex numbers. This library can parse the long
codes, there is no support for the short format yet.

```rust
use irp::Pronto;

fn main() {
    let pronto = Pronto::parse(r#"
        0000 006C 0000 0022 00AD 00AD 0016 0041 0016 0041 0016 0041 0016 0016 0016
        0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016
        0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016
        0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0041
        0016 0041 0016 0041 0016 0041 0016 0041 0016 0041 0016 06FB
        "#).expect("parse should succeed");
    // encode using 1 repeats
    let message = pronto.encode(1);
    if let Some(carrier) = &message.carrier {
        println!("carrier: {}Hz", carrier);
    }
    println!("{}", message.print_rawir());
}
```

Output:
```ignore
+4507 -4507 +573 -1693 +573 -1693 +573 -1693 +573 -573 +573 -573 +573 -573 +573 -573 +573 -573 +573 -1693 +573 -1693 +573 -1693 +573 -573 +573 -573 +573 -573 +573 -573 +573 -573 +573 -573 +573 -1693 +573 -573 +573 -573 +573 -573 +573 -573 +573 -573 +573 -573 +573 -1693 +573 -573 +573 -1693 +573 -1693 +573 -1693 +573 -1693 +573 -1693 +573 -1693 +573 -46559
```

## Encoding IRP to Pronto Hex

The IRP can also be encoded to pronto hex codes. Pronto hex codes have a repeating part, so no repeat argument is needed.

```rust
use irp::{Irp, Vartable};

fn main() {
    let mut vars = Vartable::new();
    vars.set(String::from("F"), 1, 8);
    // sony8 protocol
    let irp = Irp::parse("{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]")
        .expect("parse should succeed");
    let pronto = irp.encode_pronto(vars).expect("encode should succeed");
    println!("{}", pronto);
}
```

The output:

```ignore
0000 0068 0009 0000 0060 0018 0030 0018 0018 0018 0018 0018 0018 0018 0018 0018 0018 0018 0018 0018 0018 0510
```

## Decoding using IRP

This example decodes some IR using rc5 protocol. First the IRP notation is parsed, and then
we compile the NFA state machine for decoding. Then we create a decoder, which
needs some matching parameters, and then we can feed it input. The results can be retrieved
with the get() function on the decoder.

```rust
use irp::{Irp,InfraredData};

fn main() {
    let irp = Irp::parse(r#"
        {36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)
        [D:0..31,F:0..127,T@:0..1=0]"#)
        .expect("parse should succeed");
    let nfa = irp.compile().expect("build nfa should succeed");
    // Create a decoder with 100 microsecond tolerance, 30% relative tolerance,
    // and 20000 microseconds maximum gap.
    let mut decoder = nfa.decoder(100, 30, 20000);
    for ir in InfraredData::from_rawir(
        "+940 -860 +1790 -1750 +880 -880 +900 -890 +870 -900 +1750
        -900 +890 -910 +840 -920 +870 -920 +840 -920 +870 -1810 +840 -125000").unwrap() {
        decoder.input(ir);
    }
    let res = decoder.get().unwrap();

    println!("decoded: F={} D={} T={}", res["F"], res["D"], res["T"]);
}
```

This should print:

```ignore
decoded: F=1 D=30 T=0
```

## Parsing lirc mode2 pulse space files

This format was made popular by the [`mode2` tool](https://www.lirc.org/html/mode2.html), which prints a single line
for each flash and gap, but then calls them `pulse` and `space`. It looks like so:

```skip
carrier 38400
pulse 9024
space 4512
pulse 4512
```

This is an example of how to parse this. The result is printed in the more concise raw ir format.

```rust
fn main() {
    let message = irp::mode2::parse(r#"
        carrier 38400
        pulse 9024
        space 4512
        pulse 4512
    "#).expect("parse should succeed");
    if let Some(carrier) = &message.carrier {
        println!("carrier: {}Hz", carrier);
    }
    if let Some(duty_cycle) = &message.duty_cycle {
        println!("duty cycle: {}%", duty_cycle);
    }
    println!("{}", message.print_rawir());
}
```

## Parsing raw ir format

The raw ir format looks like "+100 -100 +100". The leading `+` and `-` may be omitted, but if present they are
checked for consistency. The parse function returns a `Vec<u32>`.

```rust
fn main() {
    let rawir: Vec<u32> = irp::rawir::parse("+100 -100 +100").expect("parse should succeed");
    println!("{}", irp::rawir::print_to_string(&rawir));
}
```

## Sending IR using cir crate

This example opens the first lirc device `/dev/lirc0` and transmits the `1`
button from a Hauppauge remote.

```rust,no_run
extern crate cir;
use cir::lirc;
use irp::{Irp, Vartable};

const RC5_IRP: &str =
    "{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]";

fn main() {
    let mut dev = lirc::open("/dev/lirc0").unwrap();

    let mut vars = Vartable::new();
    vars.set("F".to_string(), 30, 8);
    vars.set("D".to_string(), 0, 8);
    let irp = Irp::parse(RC5_IRP).unwrap();

    let message = irp.encode(vars, 1).unwrap();

    if let Some(carrier) = &message.carrier {
        // set the carrier frequency (see the 36k in the IRP definition)
        dev.set_send_carrier(*carrier as u32).unwrap();
    }

    // send the message
    dev.send(&message.raw).unwrap();

    println!("done");
}
```
