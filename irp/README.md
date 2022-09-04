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
about talking to infrared devices for receiving or transmitting; you can use
the [cir crate](https://crates.io/crates/cir)
for that on Linux. You will also need an IRP definition or Pronto hex definition
of your remote protocol. There is a long list of IRP definitions maintained by
[IrpTransmogrifier](https://github.com/bengtmartensson/IrpTransmogrifier/blob/master/src/main/resources/IrpProtocols.xml) and on
[hifi-remote](http://hifi-remote.com/wiki/index.php/DecodeIR).

There are also some utility functions for parsing raw ir and mode2 output.

See the [docs](https://docs.rs/irp/) to see the complete interface, or use the
examples below.

## What does raw IR mean?

This library encodes to *raw IR*. *raw IR* is alternating on-off durations of
infrared light, expressed in microseconds. For example,

```ignore
+500 -100 +500
```

This means 500 microseconds of infrared light on, 100 microseconds off, and
then 500 microseconds on again. This is also known as *flash* and *gap*, and lirc
uses the terms *pulse* and *space*.

It is common for raw IR to end with a gap. This ensures the gap period is correct
between one message and the next.

## What is IRP?

This is a simple example of IRP:

```ignore
{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]
```

IRP is a notation for infrared protocols, which this library uses for both
encoding and decoding infrared. This usually involve some parameters like:

- `F` for function, like *play* or *volume up*.
- `D` for device; a hi-fi set can include multiple units,
  so do you want the tape deck to *play* or the cd player?
- `S` for Subdevice
- `T` for toggle. Has a button been pressed down or was it released and
  pressed again, i.e. *toggled*. The value of `T` does not matter, just whether
  it changes from one packet to the next.
- Other protocol specific values like heating or cooling for air conditioning
  units.

Decoding means recovering the parameters from raw IR, and encoding means
creating the raw IR from some parameters values.

## What is pronto hex?

This is a notation used by the Philips Pronto universal remote, which is a series
of hex numbers, for example:

```ignore
0000 0070 0003 0002 0006 0002 0004 0002 0004 0006 0006 0003 0003 000С
```

There is one pronto hex code per button; it is not parameterized like IRP.

## Repeats

When a button is held down on a remote, then the IR message is repeated
until the button is released. Even if a button is pressed briefly, the IR
message may be repeated a few times.

Some IR receivers require repeats before IR is decoded. For example, the Sony
LBT-V702 requires at least one repeat, else the IR will be ignored.

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
Philips Pronto universal remote. The format is a series of 4 digits hex numbers.

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

This format was made popular by the [mode2 tool](https://www.lirc.org/html/mode2.html), which prints a single line
for each flash and gap, but then calls them `pulse` and `space`. It looks like so:

```skip
carrier 38400
pulse 9024
space 4512
pulse 4512
```

This is an example of how to parse this. The result is printed in the more concise raw ir format.

```rust
use irp::Message;

fn main() {
    let message = Message::parse_mode2(r#"
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
