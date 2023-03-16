# IRP Notation

This is an introduction the IRP Notation, a domain specific langauge that describes
infrared protocols. It is possible to both encode and decode an infrared protocol
using an IRP encoder or decoder, given its IRP notation.

You may want to refer to the
[Specification of IRP Notation by Graham Dixon](http://hifi-remote.com/wiki/index.php?title=IRP_Notation).

When IRP is encoded, the output is represented as raw IR. This is a list of lengths in
microseconds, alternating flash and gap. Flash means infrared light on, gap means
infrared light off. Sometimes this is known as pulse and space. In the notation used
here, flash is prefixed with `+` and a gap is prefixed with `-`.

```
+100 -200 +200 -100000
```
This means a pulse of 100 microseconds, a gap of 200 microseconds, a flash of
200 microseconds, and a gap of 100 milliseconds. The last gap is
useful for when multiple messages are sent consecutively.

You can experiment with IRP using the cir command line tool like so:

```
$ cir transmit irp --dry-run '{40k,600}<1,-1|2,-1>(4,-1,F:7,D:5,^45m)*[D:0..31,F:0..127]' -fD=12,F=64
info: carrier: 40000Hz
info: rawir: +2400 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -26400
```

Alternatively you could use IrpTransmogrifier:

```
$ irptransmogrifier --irp '{40k,600}<1,-1|2,-1>(4,-1,F:7,D:5,^45m)*[D:0..31,F:0..127]' render -r -n D=12,F=64
Freq=40000Hz[][+2400,-600,+600,-600,+600,-600,+600,-600,+600,-600,+600,-600,+600,-600,+1200,-600,+600,-600,+600,-600,+1200,-600,+1200,-600,+600,-26400][]
```

We will be describing a very simple IRP and then building from there.

## A simple IRP

```
{40k,30%,100}<>(1,-2,2u,-100m)
```

The IRP has three sections, the first part is known as the "general spec", which is
enclosed with curly braces `{` and `}`. In the general spec have three things:

* `40k`, which is the carrier for protocol, in kHz. If the carrier is omitted,
  then the carrier is `38k`. This value always has the `k` suffix.
* Next we have the duty cycle, in percentage. It has the `%` suffix, and if omitted,
  the duty cycle is not known.
* A single number `100`. This number is the unit. Every IR protocol uses signal lengths
  that are a multiple of some value, which we call the unit. The value is expression in
  microseconds.

After the general spec there is `<>`, the bitspec. We will talk about this later, so ignore
that for now.

The last part `(1,-2,2u,-100m)` is called the stream. The stream is a comma separated
list of things to encode, or decode as the case might be. The first is `1`, which
means a flash of one unit. So, we start with a flash of 100 microseconds. The
second is `-2`. A negative value means gap, so we have two units of gap, so 200
microseconds. The third part is a number followed by `u`, which means
microseconds. This is simply a flash of 2 microseconds. The last is `-200m`
which is a gap of 200 milliseconds. This encodes to:

```
+100 -200 +2 -100000
```

The stream must always end with a gap, else the IRP is invalid. The values do
not have to be integers. Once encoded, the values are always whole integers.

```
{38.5k,33%,100.1}<>(1.5,-2,10,-99.5m)

+150 -200 +1001 -99500
```

If the unit is omitted, it defaults to 1.

Both flash and gap can have the following suffix:

* `m`: milliseconds. The value is value is mulitplied by 1000.
* `u`: microseconds. The value is used as-is.
* `p`: pulses. The value is the number of carrier periods. The carrier period calculated by `1000000 / carrier_in_hz`.
* no suffix: units. The value is multiplied by the unit value from the general spec.

## The bit spec

In our simple IRP we left the bit spec `<>` empty. The bit spec specifies how bits
are encoded, i.e. how should a 0 bit be encoded and how should 1 bit be encoded. The
encoding for a 0 bit value and a 1 bit value are separated by `|`. So, for example
in the nec protocol, a 0 bit is encoded by a 560 microsecond pulse followed by 560
microsecond space, and a 1 bit is encoded by 560 microsecond pulse followed by a 1680
microsecond space. So, you would write this as:

```
{560}<1,-1|1,-3>(1,-2,2u,-100m)
```
We don't use the bit spec in this IRP yet. To encode bits, the stream needs bit
fields. Bit fields look like `value:length`, for example:

```
{560}<1,-1|1,-3>(10:4,-100m)
```
The value 10 is `1010` in binary, and it is encoded least significant bit first (lsb),
so `0` becomes `1,-1`, then `+560 -560`, the second `1` becomes `1,-3` then `+560 -1680`,
and then the same for the folling `0` and `1`. This will encode to:

```
+560 -560 +560 -1680 +560 -560 +560 -101680
```
The last bit ended with `-1068` and was followed by the `-100m` gap, so the two gaps
got merged into one larger gap. A flash between the two will prevent this from happening:
```
{560}<1,-1|1,-3>(10:4,2,-100m)

+560 -560 +560 -1680 +560 -560 +560 -1680 +1120 -100000
```
Some protocols like NEC encode most significant bit first (msb). There are two ways of
reversing the bit order. The first is to change all the bit ordering by specifying `msb`
in the general spec:
```
{560,msb}<1,-1|1,-3>(10:4,1,-100m)
```
The other is to add a `-` after `:` in the bit field, which reverses the bit order for that
specific bit field, but not others.
```
{560,lsb}<1,-1|1,-3>(10:-4,1,-100m)
```
Note that here we specified `lsb` in the general spec, which is already the default so
this is redundant. If both `msb` is specified and the bit field uses `:-` then the order
is back to least significant bit for that particular bit field.

FIXME: what happens with msb/lsb with multiple consecutive bit fields?

## Parameter Spec

Remote control protocols encode various values, like the button code, or which
device the remote wants to control. For air conditioning units there might be a
heating target. These parameters need a definition which tells the encoder and
decoder which values it may have. The parameter spec is optional.

```
{560}<1,-1|1,-3>(F:4,1,-100m) [F:0..15]
```

This is a parameterized IRP which allows us to encode and decode different values of `F`,
from 0 to 15 (inclusive). Parameters may have a default, this means that its value does not
need to be specified for encoding. The default value is not used for decoding. This is commonly
used for toggle values `T`:

```
{560}<1,-1|1,-3>(F:4,T:1,1,-100m) [F:0..15,T:0..1=0]
```

## Extents

When you hold down a button, with most protocols the infrared codes are repeated at a
constant interval irrespective of which button is being pressed. With the IRP given
above, the total length of the encoded infrared in microseconds depends on the value
of `F`. A 1 bit is encoded with gap of 1680 microsecond, and a 0 bit with a gap of 560.
After encoding, the total signal length with a `F` value of 0 will be 560 * 2 * 4
shorter than a `F` value of 15. However, Extents use the caret `^` rather
than a minus `-`.

```
{560}<1,-1|1,-3>(F:4,T:1,1,^100m) [F:0..15,T:0..1=0]
```
The `^100m` means: introduce gap so that the entire message will 100 milliseconds,
irrespective of the length of the previous data.

```
F=0 => +560 -560 +560 -560 +560 -560 +560 -560 +560 -560 +560 -93840
F=15 => +560 -1680 +560 -1680 +560 -1680 +560 -1680 +560 -560 +560 -89360
```

Multiple extents are permitted. Any following extent will introduce a gap calculated from
the previous extent rather than the beginning of the stream.

```
{560}<1,-1|1,-3>(F:4,T:1,1,^100m,F:1,1,^10m) [F:0..15,T:0..1=0]

F=15 => +560 -1680 +560 -1680 +560 -1680 +560 -1680 +560 -560 +560 -89360 +560 -1680 +560 -7200
```

## More on bit spec and bit fields

Some protocols encode 2 bits, 3 bits, or even 4 bits at time. The Human 4Phase
protocol is an example of this:

```
bit 0: -2,2
bit 1: -3,1
bit 2: 1,-3
bit 3: 2,-2
```

This can be written like so (this is not the actual Human 4Phase protocol):

```
{105}<-2,2|-3,1|1,-3|2,-2>(1,F:4,1,-100m) [F:0..15]
```

A bit spec of 4 encodes two bits at a time, a bit spec of 8 encodes 3 bits, and a bit spec
of 16 values encodes 4 bits a time. Here the value of `F:4` is encoded into two parts:
the lower two bits and the higher two bits. If you try:

```
{105}<-2,2|-3,1|1,-3|2,-2>(1,T:1,F:4,1,-100m) [F:0..15,T:0..1=0]
```
This will not work, because now we're encoding 5 bits in total, which is not a multiple of 2,
so there is no way to encode this. You will get an error saying this is an invalid IRP.

Now you may have the another bit of `F` encoded before `T:1`. This can be done with the
bit field syntax _expression:length:offset_. `F:1:4` means encode 1 bit of F, from offset
4.

```
{105}<-2,2|-3,1|1,-3|2,-2>(1,F:1:4,T:1,F:4,1,-100m) [F:0..31,T:0..1=0]
```

Now the total number of bits to encode is 6, and the IRP is valid again. The first two
bit fields are encoded together.

In the bit spec, it is permitted to omit trailing bit values, as long as they those bit values
do not occur. For example:

```
{105}<-2,2|-3,1|1,-3>(1,F:4,1,-100m) [F:0..15]
```
The bit spec has 3 values, which means two bits of the bit field will be encoded together,
just like when the bit spec has 4 values. However, no encoding is provided for the value 3.
So, this is valid a long as `F` does not begin or end with two set bits.

```
F=0 => +105 -210 +210 -210 +315 -100000
F=3 => error cannot encode 3
F=5 => +105 -315 +105 -315 +210 -100000
F=10 => +210 -315 +105 -315 +105 -100000
F=12 => error cannot encode 3
```

Some protocols use a different encoding for some bits. For example the rc6 protocol has a
different encoding for the toggle bits than the rest of the bits. We can override the bit spec
for some bit fields in the stream with the inner bit spec syntax.

```
{105}<1,-1|1,-3>(1,F:4,<2,-2|2,-6>(T:3),1,-100m) [F:0..15,T:0..7]
```

The inner bit spec may contain bit fields which then get encoded using the outer bit spec:

```
{105}<1,-1|1,-3>(1,F:4,<1:2|2:2>(T:3),1,-100m) [F:0..15,T:0..7]
```

Each bit of `T` gets encoded like this:

* `0`: => `1:2` => 1,-3,1,-1 => +105 -315 +105 -105
* `1`: => `2:2` => 1,-3,1,-1 => +105 -105 +105 -315

## Expressions and operators

Some protocols include a checksum. This can simply be some inverted bits like in the NEC protocol,
or more involved checksums. The value of a bit field can be an _expression_. Here is an example.

```
{105}<-2,2|-3,1|1,-3>(1,F:4,D:4,(F^D):4,1,-100m) [F:0..15,D:0..15]
```

following operators are allowed:

| Operator              |  Name                | Description                                         |
|-----------------------|----------------------|-----------------------------------------------------|
| `(expr)`              | parenthesis          |                                                     |
| `cond ? left : right` | conditional operator | if _cond__ is non-zero, return _left_, else _right_ |
| `left \|\| right`     | or                   | if _left_ is non-zero, return _left_, else _right_  |
| `left && right`       | and                  | if _left_ is non-zero, return _right_, else _left_  |
| `left \| right`       | bitwise or           | bitwise or of _left_ and _right_                    |
| `left & right`        | bitwise and          | bitwise and of _left_ and _right_                   |
| `left ^ right`        | bitwise xor          | bitwise xor of _left_ and _right_                   |
| `left == right`       | not equal            |                                                     |
| `left != right`       | equal                |                                                     |
| `left > right`        | more than            |                                                     |
| `left >= right`       | more than or equal   |                                                     |
| `left < right`        | less than            |                                                     |
| `left <= right`       | less than or equal   |                                                     |
| `left << right`       | bitwise shift left   |                                                     |
| `left >> right`       | bitwise shift right  |                                                     |
| `left + right`        | add                  |                                                     |
| `left - right`        | subtract             |                                                     |
| `left * right`        | mulitply             |                                                     |
| `left / right`        | divide               |                                                     |
| `left % right`        | modulo               |                                                     |
| `left ** right`       | power                |                                                     |
| `#expr`               | population count     | count number of set bits                            |
| `!expr`               | logical not          | if _expr_ is non-zero, return 1, else 0.            |
| `-expr`               | negate               |                                                     |
| `~expr`               | bitwise not          | one's complement                                    |
| `expr:length[:offset]`| bit field            | see bit field description  above                    |
| `expr::offset`        | infinite bit field   | equivalent to `expr >> offset`                      |

In a bit field, the expression should be enclosed parenthesis, with the exception of
`~` which is permitted without parenthesis, e.g `~F:4` is allowed but `!F:4` is not,
and should be written as `(!F):4`.

Expressions and variables use signed 64 bit values, and are limited to 63 bit. The 63 bit
limit is due to compatibility with IrpTransmogrifier which uses the java `long` type, which
is limited to 63 bits. This means for example that the `length` field of bit fields cannot
exceed 63.

Infinite bit fields cannot be used in the stream, they can only be used in expressions.

## Definitions and Assignments

The IRP has an optional definition section, which is useful when a checksum
has to be used more than once, for example. The definitions follow the
stream, and is a comma separated list of assignments.

```
{105}<-2,2|-3,1>(1,F:4,C:2:2,D:4,C:2:0,1,-100m){C=F^D} [F:0..15,D:0..15]
```
Rather than adding a definition, you can also add an assignment in the stream
or a bit spec.

```
{105}<-2,2|-3,1>(1,F:4,C=F^D,C:2:2,D:4,C:2:0,1,-100m) [F:0..15,D:0..15]
```

## Flash and gap using variables

So far, each flash and gap in the stream have been constant values. It is also possible
to have variables for these.

```
{105}<-2,2,X|-3,1,-X>(1,X=1,F:4,C=F^D,C:2:2,D:4,X=2,C:2:0,1,-100m) [F:0..15,D:0..15]
```

Just like constants, variables may have a `u`, `m`, or `p` suffix. You will need a space
to separate the name from the suffix. Without any suffix, the value is multiplied by
the unit from the general spec.

```
{105}<-2,2,X=1|-3,1,X=2>(1,F:4,1,-X,X m,-100m) [F:0..15,D:0..15]
```

## Simple constant repeats

Sometimes, a section of the stream has to be repeated a fixed number of number of times. This
can be achieved by putting the repeated items in the stream in parenthesis, and adding a constant
number.

```
{36k,msb,889}<1,-1|-1,1>(2,(T:1,D:5)2,F:6,^114m) [D:0..31,F:0..63,T@:0..1=0]
```

Note that this is unrelated to repeat markers `*` and `+` which are discussed below. The simple
constant repeats have no special handling other than repeating the section a constant number of times.

## Repeat markers

Some protocols have a distinct message for:

* down: this button is being pressed
* repeat: this button is being held down
* up: this button is being released

All the three parts are optional, and very few protocols have an up part.
The up section is useful because without it, the IR decoder needs to wait for
the absense of following IR to detect that a button is released, as there is
no message indicating it has been released. This delay may cause perceptible
sticky buttons. The down section is useful to distinquish between a button
being held and being released and then held again (also known
as toggled).

For a button press, there always one down section (if present), zero or more
repeat sections, and then followed by a up section if present. When encoding IRP,
you specify the number of repeats with a command line option, e.g. `--repeats=2`
with cir or `--number-repeats=2` with IrpTransmogrifier.

There are two ways of marking the down, repeat, and up section. One is with a
repeat marker `+` or `*` and the other is with variants, `[down][repeat][up]`.
Which one is more suitable depends on the protocol. First we'll discuss the
repeat markers, and then move on to variants.

In an IRP, the repeat marker `+` or `*` marks the item which is the repeating
part. Anything before the repeating part is the down section, and anything
following it the up section.

```
{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m,(16,-4,1,^108m)*) [D:0..255,S:0..255=255-D,F:0..255]
```

Here down section is `16,-8,D:8,S:8,F:8,~F:8,1,^108m` and the repeating part
`16,-4,1,^108m`. Since nothing follows that, there is no up section. The repeat
maker can have the following forms:

* `*`: any number of repeats
* `+`: 1 or more repeats
* `2+`: two or more repeats.

If a repeating section is marked with `+`, then even if you encode it with 0 repeats, the repeating
section will still be encoded once, and with `2+` it will be encoded twice + number of repeats.

It is an error to have more than one repeat marker with a `+` or `*` in an IRP.

## Variants

If the down, repeat, and up are very similar then variants might a shorter notation. A protocol may
simply encode a 1 for down, 2 for repeat and 3 for up. This can be easily represented with variants:

```
{560}<1,-1|1,-3>([V=1][V=2][V=3],F:4,V:2,1,-100m)+ [F:0..15]
```
This IRP has a down section with V=1, repeat with V=2, and up with V=3. The entire stream must be
marked with a `+` repeater, but the repeat variant will not encode anything with repeats set to 0.

The up variant is optional, and variants may be occur multiple times.

```
{560}<1,-1|1,-3>([V=1][V=2],F:4,V:2,[1][2],-100m)+ [F:0..15]
```

Empty variants `[]` have special significance. For repeat or up, it simply means "stop processing here",
for example:

```
{560}<1,-1|1,-3>([V=1][V=2][V=3],V:2,[1][2][],F:4,-100m)+ [F:0..15]
```
In this case `F:4` and everything following it is not encoded for the up part. An empty `[]` variant for down anywhere will make the entire down part empty.