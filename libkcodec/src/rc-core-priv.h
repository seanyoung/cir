/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Remote Controller core raw events header
 *
 * Copyright (C) 2010 by Mauro Carvalho Chehab
 */

#ifndef _RC_CORE_PRIV
#define _RC_CORE_PRIV

#include "lirc.h"
#include <stdbool.h>
#include <errno.h>

static void dev_dbg(int *dev, const char *fmt, ...)
{
	// printf(fmt, ...);
}

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long u64;

#define fallthrough

struct rc_dev {
	struct ir_raw_event_ctrl *raw;
	u64 enabled_protocols;
	int dev;
};

// defined in lib.rs
extern void rc_repeat(struct rc_dev*);
extern void rc_keydown(struct rc_dev*, u32 protocol, u64 scancode, u32 toggle);

static const u8 byte_rev_table[256] = {
        0x00, 0x80, 0x40, 0xc0, 0x20, 0xa0, 0x60, 0xe0,
        0x10, 0x90, 0x50, 0xd0, 0x30, 0xb0, 0x70, 0xf0,
        0x08, 0x88, 0x48, 0xc8, 0x28, 0xa8, 0x68, 0xe8,
        0x18, 0x98, 0x58, 0xd8, 0x38, 0xb8, 0x78, 0xf8,
        0x04, 0x84, 0x44, 0xc4, 0x24, 0xa4, 0x64, 0xe4,
        0x14, 0x94, 0x54, 0xd4, 0x34, 0xb4, 0x74, 0xf4,
        0x0c, 0x8c, 0x4c, 0xcc, 0x2c, 0xac, 0x6c, 0xec,
        0x1c, 0x9c, 0x5c, 0xdc, 0x3c, 0xbc, 0x7c, 0xfc,
        0x02, 0x82, 0x42, 0xc2, 0x22, 0xa2, 0x62, 0xe2,
        0x12, 0x92, 0x52, 0xd2, 0x32, 0xb2, 0x72, 0xf2,
        0x0a, 0x8a, 0x4a, 0xca, 0x2a, 0xaa, 0x6a, 0xea,
        0x1a, 0x9a, 0x5a, 0xda, 0x3a, 0xba, 0x7a, 0xfa,
        0x06, 0x86, 0x46, 0xc6, 0x26, 0xa6, 0x66, 0xe6,
        0x16, 0x96, 0x56, 0xd6, 0x36, 0xb6, 0x76, 0xf6,
        0x0e, 0x8e, 0x4e, 0xce, 0x2e, 0xae, 0x6e, 0xee,
        0x1e, 0x9e, 0x5e, 0xde, 0x3e, 0xbe, 0x7e, 0xfe,
        0x01, 0x81, 0x41, 0xc1, 0x21, 0xa1, 0x61, 0xe1,
        0x11, 0x91, 0x51, 0xd1, 0x31, 0xb1, 0x71, 0xf1,
        0x09, 0x89, 0x49, 0xc9, 0x29, 0xa9, 0x69, 0xe9,
        0x19, 0x99, 0x59, 0xd9, 0x39, 0xb9, 0x79, 0xf9,
        0x05, 0x85, 0x45, 0xc5, 0x25, 0xa5, 0x65, 0xe5,
        0x15, 0x95, 0x55, 0xd5, 0x35, 0xb5, 0x75, 0xf5,
        0x0d, 0x8d, 0x4d, 0xcd, 0x2d, 0xad, 0x6d, 0xed,
        0x1d, 0x9d, 0x5d, 0xdd, 0x3d, 0xbd, 0x7d, 0xfd,
        0x03, 0x83, 0x43, 0xc3, 0x23, 0xa3, 0x63, 0xe3,
        0x13, 0x93, 0x53, 0xd3, 0x33, 0xb3, 0x73, 0xf3,
        0x0b, 0x8b, 0x4b, 0xcb, 0x2b, 0xab, 0x6b, 0xeb,
        0x1b, 0x9b, 0x5b, 0xdb, 0x3b, 0xbb, 0x7b, 0xfb,
        0x07, 0x87, 0x47, 0xc7, 0x27, 0xa7, 0x67, 0xe7,
        0x17, 0x97, 0x57, 0xd7, 0x37, 0xb7, 0x77, 0xf7,
        0x0f, 0x8f, 0x4f, 0xcf, 0x2f, 0xaf, 0x6f, 0xef,
        0x1f, 0x9f, 0x5f, 0xdf, 0x3f, 0xbf, 0x7f, 0xff,
};

static inline u8 bitrev8(u8 byte)
{
        return byte_rev_table[byte];
}

static inline u16 bitrev16(u16 x)
{
        return (bitrev8(x & 0xff) << 8) | bitrev8(x >> 8);
}

/* Get NEC scancode and protocol type from address and command bytes */
static inline u32 ir_nec_bytes_to_scancode(u8 address, u8 not_address,
                                           u8 command, u8 not_command,
                                           enum rc_proto *protocol)
{
        u32 scancode;

        if ((command ^ not_command) != 0xff) {
                /* NEC transport, but modified protocol, used by at
                 * least Apple and TiVo remotes
                 */
                scancode = not_address << 24 |
                        address     << 16 |
                        not_command <<  8 |
                        command;
                *protocol = RC_PROTO_NEC32;
        } else if ((address ^ not_address) != 0xff) {
                /* Extended NEC */
                scancode = address     << 16 |
                           not_address <<  8 |
                           command;
                *protocol = RC_PROTO_NECX;
        } else {
                /* Normal NEC */
                scancode = address << 8 | command;
                *protocol = RC_PROTO_NEC;
        }

        return scancode;
}

#define BIT_ULL(nr)         (1ull << (nr))
#define BIT(nr)         (1u << (nr))

#define RC_PROTO_BIT_NONE               0ULL
#define RC_PROTO_BIT_UNKNOWN            BIT_ULL(RC_PROTO_UNKNOWN)
#define RC_PROTO_BIT_OTHER              BIT_ULL(RC_PROTO_OTHER)
#define RC_PROTO_BIT_RC5                BIT_ULL(RC_PROTO_RC5)
#define RC_PROTO_BIT_RC5X_20            BIT_ULL(RC_PROTO_RC5X_20)
#define RC_PROTO_BIT_RC5_SZ             BIT_ULL(RC_PROTO_RC5_SZ)
#define RC_PROTO_BIT_JVC                BIT_ULL(RC_PROTO_JVC)
#define RC_PROTO_BIT_SONY12             BIT_ULL(RC_PROTO_SONY12)
#define RC_PROTO_BIT_SONY15             BIT_ULL(RC_PROTO_SONY15)
#define RC_PROTO_BIT_SONY20             BIT_ULL(RC_PROTO_SONY20)
#define RC_PROTO_BIT_NEC                BIT_ULL(RC_PROTO_NEC)
#define RC_PROTO_BIT_NECX               BIT_ULL(RC_PROTO_NECX)
#define RC_PROTO_BIT_NEC32              BIT_ULL(RC_PROTO_NEC32)
#define RC_PROTO_BIT_SANYO              BIT_ULL(RC_PROTO_SANYO)
#define RC_PROTO_BIT_MCIR2_KBD          BIT_ULL(RC_PROTO_MCIR2_KBD)
#define RC_PROTO_BIT_MCIR2_MSE          BIT_ULL(RC_PROTO_MCIR2_MSE)
#define RC_PROTO_BIT_RC6_0              BIT_ULL(RC_PROTO_RC6_0)
#define RC_PROTO_BIT_RC6_6A_20          BIT_ULL(RC_PROTO_RC6_6A_20)
#define RC_PROTO_BIT_RC6_6A_24          BIT_ULL(RC_PROTO_RC6_6A_24)
#define RC_PROTO_BIT_RC6_6A_32          BIT_ULL(RC_PROTO_RC6_6A_32)
#define RC_PROTO_BIT_RC6_MCE            BIT_ULL(RC_PROTO_RC6_MCE)
#define RC_PROTO_BIT_SHARP              BIT_ULL(RC_PROTO_SHARP)
#define RC_PROTO_BIT_XMP                BIT_ULL(RC_PROTO_XMP)
#define RC_PROTO_BIT_CEC                BIT_ULL(RC_PROTO_CEC)
#define RC_PROTO_BIT_IMON               BIT_ULL(RC_PROTO_IMON)
#define RC_PROTO_BIT_RCMM12             BIT_ULL(RC_PROTO_RCMM12)
#define RC_PROTO_BIT_RCMM24             BIT_ULL(RC_PROTO_RCMM24)
#define RC_PROTO_BIT_RCMM32             BIT_ULL(RC_PROTO_RCMM32)
#define RC_PROTO_BIT_XBOX_DVD           BIT_ULL(RC_PROTO_XBOX_DVD)

/*
 * From rc-raw.c
 * The Raw interface is specific to InfraRed. It may be a good idea to
 * split it later into a separate header.
 */
struct ir_raw_event {
        union {
                u32             duration;
                u32             carrier;
        };
        u8                      duty_cycle;

        bool                pulse;
        bool                overflow;
        bool                timeout;
        bool                carrier_report;
};

#define US_TO_NS(usec)          ((usec) * 1000)
#define MS_TO_US(msec)          ((msec) * 1000)
#define IR_MAX_DURATION         MS_TO_US(500)
#define IR_DEFAULT_TIMEOUT      MS_TO_US(125)
#define IR_MAX_TIMEOUT          LIRC_VALUE_MASK

#define	RC_DEV_MAX		256
/* Define the max number of pulse/space transitions to buffer */
#define	MAX_IR_EVENT_SIZE	512

struct ir_raw_handler {
	u64 protocols; /* which are handled by this handler */
	int (*decode)(struct rc_dev *dev, struct ir_raw_event event);
	int (*encode)(enum rc_proto protocol, u32 scancode,
		      struct ir_raw_event *events, unsigned int max);
	u32 carrier;
	u32 min_timeout;

	/* These two should only be used by the mce kbd decoder */
	int (*raw_register)(struct rc_dev *dev);
	int (*raw_unregister)(struct rc_dev *dev);
};

struct ir_raw_event_ctrl {
	struct rc_dev			*dev;		/* pointer to the parent rc_dev */
	/* handle delayed ir_raw_event_store_edge processing */
	// spinlock_t			edge_spinlock;
	// struct timer_list		edge_handle;

	/* raw decoder state follows */
	struct ir_raw_event prev_ev;
	struct ir_raw_event this_ev;

	struct nec_dec {
		int state;
		unsigned count;
		u32 bits;
		bool is_nec_x;
		bool necx_repeat;
	} nec;

	struct rc5_dec {
		int state;
		u32 bits;
		unsigned count;
		bool is_rc5x;
	} rc5;

	struct rc6_dec {
		int state;
		u8 header;
		u32 body;
		bool toggle;
		unsigned count;
		unsigned wanted_bits;
	} rc6;

	struct sony_dec {
		int state;
		u32 bits;
		unsigned count;
	} sony;

	struct jvc_dec {
		int state;
		u16 bits;
		u16 old_bits;
		unsigned count;
		bool first;
		bool toggle;
	} jvc;

	struct sanyo_dec {
		int state;
		unsigned count;
		u64 bits;
	} sanyo;

	struct sharp_dec {
		int state;
		unsigned count;
		u32 bits;
		unsigned int pulse_len;
	} sharp;

	struct mce_kbd_dec {
		/* locks key up timer */
		// spinlock_t keylock;
		// struct timer_list rx_timeout;
		int state;
		u8 header;
		u32 body;
		unsigned count;
		unsigned wanted_bits;
	} mce_kbd;

	struct xmp_dec {
		int state;
		unsigned count;
		u32 durations[16];
	} xmp;

	struct imon_dec {
		int state;
		int count;
		int last_chk;
		unsigned int bits;
		bool stick_keyboard;
	} imon;

	struct rcmm_dec {
		int state;
		unsigned int count;
		u32 bits;
	} rcmm;
};

/* macros for IR decoders */
static inline bool geq_margin(unsigned d1, unsigned d2, unsigned margin)
{
	return d1 > (d2 - margin);
}

static inline bool eq_margin(unsigned d1, unsigned d2, unsigned margin)
{
	return ((d1 > (d2 - margin)) && (d1 < (d2 + margin)));
}

static inline bool is_transition(struct ir_raw_event *x, struct ir_raw_event *y)
{
	return x->pulse != y->pulse;
}

static inline void decrease_duration(struct ir_raw_event *ev, unsigned duration)
{
	if (duration > ev->duration)
		ev->duration = 0;
	else
		ev->duration -= duration;
}

/* Returns true if event is normal pulse/space event */
static inline bool is_timing_event(struct ir_raw_event ev)
{
	return !ev.carrier_report && !ev.overflow;
}

#define TO_STR(is_pulse)		((is_pulse) ? "pulse" : "space")

/* functions for IR encoders */
bool rc_validate_scancode(enum rc_proto proto, u32 scancode);

static inline void init_ir_raw_event_duration(struct ir_raw_event *ev,
					      unsigned int pulse,
					      u32 duration)
{
	*ev = (struct ir_raw_event) {
		.duration = duration,
		.pulse = pulse
	};
}

/**
 * struct ir_raw_timings_manchester - Manchester coding timings
 * @leader_pulse:	duration of leader pulse (if any) 0 if continuing
 *			existing signal
 * @leader_space:	duration of leader space (if any)
 * @clock:		duration of each pulse/space in ns
 * @invert:		if set clock logic is inverted
 *			(0 = space + pulse, 1 = pulse + space)
 * @trailer_space:	duration of trailer space in ns
 */
struct ir_raw_timings_manchester {
	unsigned int leader_pulse;
	unsigned int leader_space;
	unsigned int clock;
	unsigned int invert:1;
	unsigned int trailer_space;
};

int ir_raw_gen_manchester(struct ir_raw_event **ev, unsigned int max,
			  const struct ir_raw_timings_manchester *timings,
			  unsigned int n, u64 data);

/**
 * ir_raw_gen_pulse_space() - generate pulse and space raw events.
 * @ev:			Pointer to pointer to next free raw event.
 *			Will be incremented for each raw event written.
 * @max:		Pointer to number of raw events available in buffer.
 *			Will be decremented for each raw event written.
 * @pulse_width:	Width of pulse in ns.
 * @space_width:	Width of space in ns.
 *
 * Returns:	0 on success.
 *		-ENOBUFS if there isn't enough buffer space to write both raw
 *		events. In this case @max events will have been written.
 */
static inline int ir_raw_gen_pulse_space(struct ir_raw_event **ev,
					 unsigned int *max,
					 unsigned int pulse_width,
					 unsigned int space_width)
{
	if (!*max)
		return -ENOBUFS;
	init_ir_raw_event_duration((*ev)++, 1, pulse_width);
	if (!--*max)
		return -ENOBUFS;
	init_ir_raw_event_duration((*ev)++, 0, space_width);
	--*max;
	return 0;
}

/**
 * struct ir_raw_timings_pd - pulse-distance modulation timings
 * @header_pulse:	duration of header pulse in ns (0 for none)
 * @header_space:	duration of header space in ns
 * @bit_pulse:		duration of bit pulse in ns
 * @bit_space:		duration of bit space (for logic 0 and 1) in ns
 * @trailer_pulse:	duration of trailer pulse in ns
 * @trailer_space:	duration of trailer space in ns
 * @msb_first:		1 if most significant bit is sent first
 */
struct ir_raw_timings_pd {
	unsigned int header_pulse;
	unsigned int header_space;
	unsigned int bit_pulse;
	unsigned int bit_space[2];
	unsigned int trailer_pulse;
	unsigned int trailer_space;
	unsigned int msb_first:1;
};

int ir_raw_gen_pd(struct ir_raw_event **ev, unsigned int max,
		  const struct ir_raw_timings_pd *timings,
		  unsigned int n, u64 data);

/**
 * struct ir_raw_timings_pl - pulse-length modulation timings
 * @header_pulse:	duration of header pulse in ns (0 for none)
 * @bit_space:		duration of bit space in ns
 * @bit_pulse:		duration of bit pulse (for logic 0 and 1) in ns
 * @trailer_space:	duration of trailer space in ns
 * @msb_first:		1 if most significant bit is sent first
 */
struct ir_raw_timings_pl {
	unsigned int header_pulse;
	unsigned int bit_space;
	unsigned int bit_pulse[2];
	unsigned int trailer_space;
	unsigned int msb_first:1;
};

int ir_raw_gen_pl(struct ir_raw_event **ev, unsigned int max,
		  const struct ir_raw_timings_pl *timings,
		  unsigned int n, u64 data);

/*
 * Routines from rc-raw.c to be used internally and by decoders
 */
u64 ir_raw_get_allowed_protocols(void);
int ir_raw_event_prepare(struct rc_dev *dev);
int ir_raw_event_register(struct rc_dev *dev);
void ir_raw_event_free(struct rc_dev *dev);
void ir_raw_event_unregister(struct rc_dev *dev);
int ir_raw_handler_register(struct ir_raw_handler *ir_raw_handler);
void ir_raw_handler_unregister(struct ir_raw_handler *ir_raw_handler);
void ir_raw_load_modules(u64 *protocols);
void ir_raw_init(void);

#endif /* _RC_CORE_PRIV */
