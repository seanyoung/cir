/*
 * ir-encode.c - encodes IR scancodes in different protocols
 *
 * Copyright (C) 2016 Sean Young <sean@mess.org>
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 2 of the License.

 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 */

/*
 * TODO: XMP protocol and MCE keyboard
 */

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <ctype.h>
#include <stdio.h>

#include "lirc.h"

#include "ir-encode.h"

#define NS_TO_US(x) (((x)+500)/1000)

static const int nec_unit = 562500;

static void nec_add_byte(unsigned *buf, int *n, unsigned bits)
{
	int i;
	for (i=0; i<8; i++) {
		buf[(*n)++] = NS_TO_US(nec_unit);
		if (bits & (1 << i))
			buf[(*n)++] = NS_TO_US(nec_unit * 3);
		else
			buf[(*n)++] = NS_TO_US(nec_unit);
	}
}

static int nec_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	int n = 0;

	buf[n++] = NS_TO_US(nec_unit * 16);
	buf[n++] = NS_TO_US(nec_unit * 8);

	switch (proto) {
	default:
		return 0;
	case RC_PROTO_NEC:
		nec_add_byte(buf, &n, scancode >> 8);
		nec_add_byte(buf, &n, ~(scancode >> 8));
		nec_add_byte(buf, &n, scancode);
		nec_add_byte(buf, &n, ~scancode);
		break;
	case RC_PROTO_NECX:
		nec_add_byte(buf, &n, scancode >> 16);
		nec_add_byte(buf, &n, scancode >> 8);
		nec_add_byte(buf, &n, scancode);
		nec_add_byte(buf, &n, ~scancode);
		break;
	case RC_PROTO_NEC32:
		nec_add_byte(buf, &n, scancode >> 16);
		nec_add_byte(buf, &n, scancode >> 24);
		nec_add_byte(buf, &n, scancode);
		nec_add_byte(buf, &n, scancode >> 8);
		break;
	}

	buf[n++] = NS_TO_US(nec_unit);

	return n;
}

static int jvc_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	const int jvc_unit = 525000;
	int i;

	/* swap bytes so address comes first */
	scancode = ((scancode << 8) & 0xff00) | ((scancode >> 8) & 0x00ff);

	*buf++ = NS_TO_US(jvc_unit * 16);
	*buf++ = NS_TO_US(jvc_unit * 8);

	for (i=0; i<16; i++) {
		*buf++ = NS_TO_US(jvc_unit);

		if (scancode & 1)
			*buf++ = NS_TO_US(jvc_unit * 3);
		else
			*buf++ = NS_TO_US(jvc_unit);

		scancode >>= 1;
	}

	*buf = NS_TO_US(jvc_unit);

	return 35;
}

static const int sanyo_unit = 562500;

static void sanyo_add_bits(unsigned **buf, int bits, int count)
{
	int i;
	for (i=0; i<count; i++) {
		*(*buf)++ = NS_TO_US(sanyo_unit);

		if (bits & (1 << i))
			*(*buf)++ = NS_TO_US(sanyo_unit * 3);
		else
			*(*buf)++ = NS_TO_US(sanyo_unit);
	}
}

static int sanyo_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	*buf++ = NS_TO_US(sanyo_unit * 16);
	*buf++ = NS_TO_US(sanyo_unit * 8);

	sanyo_add_bits(&buf, scancode >> 8, 13);
	sanyo_add_bits(&buf, ~(scancode >> 8), 13);
	sanyo_add_bits(&buf, scancode, 8);
	sanyo_add_bits(&buf, ~scancode, 8);

	*buf = NS_TO_US(sanyo_unit);

	return 87;
}

static const int sharp_unit = 40000;

static void sharp_add_bits(unsigned **buf, int bits, int count)
{
	int i;
	for (i=0; i<count; i++) {
		*(*buf)++ = NS_TO_US(sharp_unit * 8);

		if (bits & (1 << i))
			*(*buf)++ = NS_TO_US(sharp_unit * 42);
		else
			*(*buf)++ = NS_TO_US(sharp_unit * 17);
	}
}

static int sharp_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	sharp_add_bits(&buf, scancode >> 8, 5);
	sharp_add_bits(&buf, scancode, 8);
	sharp_add_bits(&buf, 1, 2);

	*buf++ = NS_TO_US(sharp_unit * 8);
	*buf++ = NS_TO_US(sharp_unit * 1000);

	sharp_add_bits(&buf, scancode >> 8, 5);
	sharp_add_bits(&buf, ~scancode, 8);
	sharp_add_bits(&buf, ~1, 2);
	*buf++ = NS_TO_US(sharp_unit * 8);

	return (13 + 2) * 4 + 3;
}

static const int sony_unit = 600000;

static void sony_add_bits(unsigned *buf, int *n, int bits, int count)
{
	int i;
	for (i=0; i<count; i++) {
		if (bits & (1 << i))
			buf[(*n)++] = NS_TO_US(sony_unit * 2);
		else
			buf[(*n)++] = NS_TO_US(sony_unit);

		buf[(*n)++] = NS_TO_US(sony_unit);
	}
}

static int sony_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	int n = 0;

	buf[n++] = NS_TO_US(sony_unit * 4);
	buf[n++] = NS_TO_US(sony_unit);

	switch (proto) {
	case RC_PROTO_SONY12:
		sony_add_bits(buf, &n, scancode, 7);
		sony_add_bits(buf, &n, scancode >> 16, 5);
		break;
	case RC_PROTO_SONY15:
		sony_add_bits(buf, &n, scancode, 7);
		sony_add_bits(buf, &n, scancode >> 16, 8);
		break;
	case RC_PROTO_SONY20:
		sony_add_bits(buf, &n, scancode, 7);
		sony_add_bits(buf, &n, scancode >> 16, 5);
		sony_add_bits(buf, &n, scancode >> 8, 8);
		break;
	default:
		return 0;
	}

	/* ignore last space */
	return n - 1;
}

static const unsigned int rc5_unit = 888888;

static void rc5_advance_space(unsigned *buf, unsigned *n, unsigned length)
{
	if (*n % 2)
		buf[*n] += length;
	else
		buf[++(*n)] = length;
}

static void rc5_advance_pulse(unsigned *buf, unsigned *n, unsigned length)
{
	if (*n % 2)
		buf[++(*n)] = length;
	else
		buf[*n] += length;
}

static void rc5_add_bits(unsigned *buf, unsigned *n, int bits, int count)
{
	while (count--) {
		if (bits & (1 << count)) {
			rc5_advance_space(buf, n, NS_TO_US(rc5_unit));
			rc5_advance_pulse(buf, n, NS_TO_US(rc5_unit));
		} else {
			rc5_advance_pulse(buf, n, NS_TO_US(rc5_unit));
			rc5_advance_space(buf, n, NS_TO_US(rc5_unit));
		}
	}
}

static int rc5_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	unsigned n = 0;

	buf[n] = NS_TO_US(rc5_unit);

	switch (proto) {
	default:
		return 0;
	case RC_PROTO_RC5:
		rc5_add_bits(buf, &n, !(scancode & 0x40), 1);
		rc5_add_bits(buf, &n, 0, 1);
		rc5_add_bits(buf, &n, scancode >> 8, 5);
		rc5_add_bits(buf, &n, scancode, 6);
		break;
	case RC_PROTO_RC5_SZ:
		rc5_add_bits(buf, &n, !!(scancode & 0x2000), 1);
		rc5_add_bits(buf, &n, 0, 1);
		rc5_add_bits(buf, &n, scancode >> 6, 6);
		rc5_add_bits(buf, &n, scancode, 6);
		break;
	case RC_PROTO_RC5X_20:
		rc5_add_bits(buf, &n, !(scancode & 0x4000), 1);
		rc5_add_bits(buf, &n, 0, 1);
		rc5_add_bits(buf, &n, scancode >> 16, 5);
		rc5_advance_space(buf, &n, NS_TO_US(rc5_unit * 4));
		rc5_add_bits(buf, &n, scancode >> 8, 6);
		rc5_add_bits(buf, &n, scancode, 6);
		break;
	}

	/* drop any trailing pulse */
	return (n % 2) ? n : n + 1;
}

static const unsigned int rc6_unit = 444444;

static void rc6_advance_space(unsigned *buf, unsigned *n, unsigned length)
{
	if (*n % 2)
		buf[*n] += length;
	else
		buf[++(*n)] = length;
}

static void rc6_advance_pulse(unsigned *buf, unsigned *n, unsigned length)
{
	if (*n % 2)
		buf[++(*n)] = length;
	else
		buf[*n] += length;
}

static void rc6_add_bits(unsigned *buf, unsigned *n,
			 unsigned bits, unsigned count, unsigned length)
{
	while (count--) {
		if (bits & (1 << count)) {
			rc6_advance_pulse(buf, n, length);
			rc6_advance_space(buf, n, length);
		} else {
			rc6_advance_space(buf, n, length);
			rc6_advance_pulse(buf, n, length);
		}
	}
}

static int rc6_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	unsigned n = 0;
	buf[n++] = NS_TO_US(rc6_unit * 6);
	buf[n++] = NS_TO_US(rc6_unit * 2);
	buf[n] = 0;

	switch (proto) {
	default:
		return 0;
	case RC_PROTO_RC6_0:
		rc6_add_bits(buf, &n, 8, 4, NS_TO_US(rc6_unit));
		rc6_add_bits(buf, &n, 0, 1, NS_TO_US(rc6_unit * 2));
		rc6_add_bits(buf, &n, scancode, 16, NS_TO_US(rc6_unit));
		break;
	case RC_PROTO_RC6_6A_20:
		rc6_add_bits(buf, &n, 14, 4, NS_TO_US(rc6_unit));
		rc6_add_bits(buf, &n, 0, 1, NS_TO_US(rc6_unit * 2));
		rc6_add_bits(buf, &n, scancode, 20, NS_TO_US(rc6_unit));
		break;
	case RC_PROTO_RC6_6A_24:
		rc6_add_bits(buf, &n, 14, 4, NS_TO_US(rc6_unit));
		rc6_add_bits(buf, &n, 0, 1, NS_TO_US(rc6_unit * 2));
		rc6_add_bits(buf, &n, scancode, 24, NS_TO_US(rc6_unit));
		break;
	case RC_PROTO_RC6_6A_32:
	case RC_PROTO_RC6_MCE:
		rc6_add_bits(buf, &n, 14, 4, NS_TO_US(rc6_unit));
		rc6_add_bits(buf, &n, 0, 1, NS_TO_US(rc6_unit * 2));
		rc6_add_bits(buf, &n, scancode, 32, NS_TO_US(rc6_unit));
		break;
	}

	/* drop any trailing pulse */
	return (n % 2) ? n : n + 1;
}

static int xbox_dvd_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	int len = 0;

	buf[len++] = 4000;
	buf[len++] = 3900;

	scancode &= 0xfff;
	scancode |= (~scancode << 12) & 0xfff000;

	for (int i=23; i >=0; i--) {
		buf[len++] = 550;

		if (scancode & (1 << i))
			buf[len++] = 1900;
		else
			buf[len++] = 900;
	}

	buf[len++]= 550;

	return len;
}

static const struct {
	char name[10];
	unsigned scancode_mask;
	unsigned max_edges;
	unsigned carrier;
	int (*encode)(enum rc_proto proto, unsigned scancode, unsigned *buf);
} protocols[] = {
	[RC_PROTO_UNKNOWN] = { "unknown" },
	[RC_PROTO_OTHER] = { "other" },
	[RC_PROTO_RC5] = { "rc5", 0x1f7f, 25, 36000, rc5_encode },
	[RC_PROTO_RC5X_20] = { "rc5x_20", 0x1f7f3f, 40, 36000, rc5_encode },
	[RC_PROTO_RC5_SZ] = { "rc5_sz", 0x2fff, 27, 36000, rc5_encode },
	[RC_PROTO_SONY12] = { "sony12", 0x1f007f, 25, 40000, sony_encode },
	[RC_PROTO_SONY15] = { "sony15", 0xff007f, 31, 40000, sony_encode },
	[RC_PROTO_SONY20] = { "sony20", 0x1fff7f, 41, 40000, sony_encode },
	[RC_PROTO_JVC] = { "jvc", 0xffff, 35, 38000, jvc_encode },
	[RC_PROTO_NEC] = { "nec", 0xffff, 67, 38000, nec_encode },
	[RC_PROTO_NECX] = { "necx", 0xffffff, 67, 38000, nec_encode },
	[RC_PROTO_NEC32] = { "nec32", 0xffffffff, 67, 38000, nec_encode },
	[RC_PROTO_SANYO] = { "sanyo", 0x1fffff, 87, 38000, sanyo_encode },
	[RC_PROTO_RC6_0] = { "rc6_0", 0xffff, 43, 36000, rc6_encode },
	[RC_PROTO_RC6_6A_20] = { "rc6_6a_20", 0xfffff, 52, 36000, rc6_encode },
	[RC_PROTO_RC6_6A_24] = { "rc6_6a_24", 0xffffff, 60, 36000, rc6_encode },
	[RC_PROTO_RC6_6A_32] = { "rc6_6a_32", 0xffffffff, 76, 36000, rc6_encode },
	[RC_PROTO_RC6_MCE] = { "rc6_mce", 0xffff7fff, 76, 36000, rc6_encode },
	[RC_PROTO_SHARP] = { "sharp", 0x1fff, 63, 38000, sharp_encode },
	[RC_PROTO_MCIR2_KBD] = { "mcir2-kbd" },
	[RC_PROTO_MCIR2_MSE] = { "mcir2-mse" },
	[RC_PROTO_XMP] = { "xmp" },
	[RC_PROTO_CEC] = { "cec" },
	[RC_PROTO_IMON] = { "imon", 0x7fffffff },
	[RC_PROTO_RCMM12] = { "rc-mm-12", 0x0fff },
	[RC_PROTO_RCMM24] = { "rc-mm-24", 0xffffff },
	[RC_PROTO_RCMM32] = { "rc-mm-32", 0xffffffff },
	[RC_PROTO_XBOX_DVD] = { "xbox-dvd", 0xfff, 68, 38000, xbox_dvd_encode },
};

static bool str_like(const char *a, const char *b)
{
	while (*a && *b) {
		while (*a == ' ' || *a == '-' || *a == '_')
			a++;
		while (*b == ' ' || *b == '-' || *b == '_')
			b++;

		if (*a >= 0x7f || *b >= 0x7f)
			return false;

		if (tolower(*a) != tolower(*b))
			return false;

		a++; b++;
	}

	return !*a && !*b;
}

bool protocol_match(const char *name, enum rc_proto *proto)
{
	enum rc_proto p;

	for (p=0; p<ARRAY_SIZE(protocols); p++) {
		if (str_like(protocols[p].name, name)) {
			*proto = p;
			return true;
		}
	}

	return false;
}

unsigned protocol_carrier(enum rc_proto proto)
{
	return protocols[proto].carrier;
}

unsigned protocol_max_size(enum rc_proto proto)
{
	return protocols[proto].max_edges;
}

unsigned protocol_scancode_mask(enum rc_proto proto)
{
	return protocols[proto].scancode_mask;
}

void protocol_scancode_valid(enum rc_proto *p, unsigned *s)
{
	enum rc_proto p2 = *p;
	unsigned s2 = *s;

	// rc6_mce is rc6_6a_32 with vendor code 0x800f and
	if (*p == RC_PROTO_RC6_MCE && (*s & 0xffff0000) != 0x800f0000) {
		p2 = RC_PROTO_RC6_6A_32;
	} else if (*p == RC_PROTO_RC6_6A_32 && (*s & 0xffff0000) == 0x800f0000) {
		p2 = RC_PROTO_RC6_MCE;
	} else if (*p == RC_PROTO_NEC || *p == RC_PROTO_NECX || *p == RC_PROTO_NEC32) {
		// nec scancodes may repeat the address and command
		// in inverted form; the inverted values are not in the
		// scancode.

		// can 24 bit scancode be represented as 16 bit scancode
		if (*s > 0x0000ffff && *s <= 0x00ffffff) {
			if ((((*s >> 16) ^ ~(*s >> 8)) & 0xff) != 0) {
				// is it necx
				p2 = RC_PROTO_NECX;
			} else {
				// or regular nec
				s2 = ((*s >> 8) & 0xff00) | (*s & 0x00ff);
				p2 = RC_PROTO_NEC;
			}
		// can 32 bit scancode be represented as 24 or 16 bit scancode
		} else if (*s > 0x00ffffff) {
			if (((((*s >> 24) ^ ~(*s >> 16)) & 0xff) == 0) &&
			    ((((*s >> 8) ^ ~(*s >> 0)) & 0xff) == 0)) {
				// is it nec
				s2 = ((*s >> 16) & 0xff00) |
				     ((*s >> 8) & 0x00ff);
				p2 = RC_PROTO_NEC;
			} else if (((((*s >> 24) ^ ~(*s >> 16)) & 0xff) != 0) &&
			    ((((*s >> 8) ^ ~(*s >> 0)) & 0xff) == 0)) {
				// is it nec-x
				s2 = (*s >> 8) & 0xffffff;
				p2 = RC_PROTO_NECX;
			} else {
				// or it has to be nec32
				p2 = RC_PROTO_NEC32;
			}
		}
	}

	s2 &= protocols[p2].scancode_mask;

	if (*p != p2 || *s != s2) {
		fprintf(stderr,
			"warning: `%s:0x%x' will be decoded as `%s:0x%x'\n",
			protocol_name(*p), *s, protocol_name(p2), s2);

		*p = p2;
		*s = s2;
	}
}

bool protocol_encoder_available(enum rc_proto proto)
{
	return protocols[proto].encode != NULL;
}

unsigned protocol_encode(enum rc_proto proto, unsigned scancode, unsigned *buf)
{
	if (!protocols[proto].encode)
		return 0;

	return protocols[proto].encode(proto, scancode, buf);
}

const char* protocol_name(enum rc_proto proto)
{
	if (proto >= ARRAY_SIZE(protocols) || !protocols[proto].name[0])
		return NULL;

	return protocols[proto].name;
}
