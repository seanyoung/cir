/* SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note */
/*
 * lirc.h - linux infrared remote control header file
 */

#ifndef _LINUX_LIRC_H
#define _LINUX_LIRC_H

#define PULSE_BIT       0x01000000
#define PULSE_MASK      0x00FFFFFF

#define LIRC_MODE2_SPACE     0x00000000
#define LIRC_MODE2_PULSE     0x01000000
#define LIRC_MODE2_FREQUENCY 0x02000000
#define LIRC_MODE2_TIMEOUT   0x03000000
#define LIRC_MODE2_OVERFLOW  0x04000000

#define LIRC_VALUE_MASK      0x00FFFFFF
#define LIRC_MODE2_MASK      0xFF000000

#define LIRC_SPACE(val) (((val) & LIRC_VALUE_MASK) | LIRC_MODE2_SPACE)
#define LIRC_PULSE(val) (((val) & LIRC_VALUE_MASK) | LIRC_MODE2_PULSE)
#define LIRC_FREQUENCY(val) (((val) & LIRC_VALUE_MASK) | LIRC_MODE2_FREQUENCY)
#define LIRC_TIMEOUT(val) (((val) & LIRC_VALUE_MASK) | LIRC_MODE2_TIMEOUT)
#define LIRC_OVERFLOW(val) (((val) & LIRC_VALUE_MASK) | LIRC_MODE2_OVERFLOW)

#define LIRC_VALUE(val) ((val)&LIRC_VALUE_MASK)
#define LIRC_MODE2(val) ((val)&LIRC_MODE2_MASK)

#define LIRC_IS_SPACE(val) (LIRC_MODE2(val) == LIRC_MODE2_SPACE)
#define LIRC_IS_PULSE(val) (LIRC_MODE2(val) == LIRC_MODE2_PULSE)
#define LIRC_IS_FREQUENCY(val) (LIRC_MODE2(val) == LIRC_MODE2_FREQUENCY)
#define LIRC_IS_TIMEOUT(val) (LIRC_MODE2(val) == LIRC_MODE2_TIMEOUT)
#define LIRC_IS_OVERFLOW(val) (LIRC_MODE2(val) == LIRC_MODE2_OVERFLOW)

/* used heavily by lirc userspace */
#define lirc_t int

/*** lirc compatible hardware features ***/

#define LIRC_MODE2SEND(x) (x)
#define LIRC_SEND2MODE(x) (x)
#define LIRC_MODE2REC(x) ((x) << 16)
#define LIRC_REC2MODE(x) ((x) >> 16)

#define LIRC_MODE_RAW                  0x00000001
#define LIRC_MODE_PULSE                0x00000002
#define LIRC_MODE_MODE2                0x00000004
#define LIRC_MODE_SCANCODE             0x00000008
#define LIRC_MODE_LIRCCODE             0x00000010


#define LIRC_CAN_SEND_RAW              LIRC_MODE2SEND(LIRC_MODE_RAW)
#define LIRC_CAN_SEND_PULSE            LIRC_MODE2SEND(LIRC_MODE_PULSE)
#define LIRC_CAN_SEND_MODE2            LIRC_MODE2SEND(LIRC_MODE_MODE2)
#define LIRC_CAN_SEND_LIRCCODE         LIRC_MODE2SEND(LIRC_MODE_LIRCCODE)

#define LIRC_CAN_SEND_MASK             0x0000003f

#define LIRC_CAN_SET_SEND_CARRIER      0x00000100
#define LIRC_CAN_SET_SEND_DUTY_CYCLE   0x00000200
#define LIRC_CAN_SET_TRANSMITTER_MASK  0x00000400

#define LIRC_CAN_REC_RAW               LIRC_MODE2REC(LIRC_MODE_RAW)
#define LIRC_CAN_REC_PULSE             LIRC_MODE2REC(LIRC_MODE_PULSE)
#define LIRC_CAN_REC_MODE2             LIRC_MODE2REC(LIRC_MODE_MODE2)
#define LIRC_CAN_REC_SCANCODE          LIRC_MODE2REC(LIRC_MODE_SCANCODE)
#define LIRC_CAN_REC_LIRCCODE          LIRC_MODE2REC(LIRC_MODE_LIRCCODE)

#define LIRC_CAN_REC_MASK              LIRC_MODE2REC(LIRC_CAN_SEND_MASK)

#define LIRC_CAN_SET_REC_CARRIER       (LIRC_CAN_SET_SEND_CARRIER << 16)

#define LIRC_CAN_SET_REC_CARRIER_RANGE    0x80000000
#define LIRC_CAN_GET_REC_RESOLUTION       0x20000000
#define LIRC_CAN_SET_REC_TIMEOUT          0x10000000

#define LIRC_CAN_MEASURE_CARRIER          0x02000000
#define LIRC_CAN_USE_WIDEBAND_RECEIVER    0x04000000

#define LIRC_CAN_SEND(x) ((x)&LIRC_CAN_SEND_MASK)
#define LIRC_CAN_REC(x) ((x)&LIRC_CAN_REC_MASK)

/*
 * Unused features. These features were never implemented, in tree or
 * out of tree. These definitions are here so not to break the lircd build.
 */
#define LIRC_CAN_SET_REC_FILTER		0
#define LIRC_CAN_NOTIFY_DECODE		0

/**
 * enum rc_proto - the Remote Controller protocol
 *
 * @RC_PROTO_UNKNOWN: Protocol not known
 * @RC_PROTO_OTHER: Protocol known but proprietary
 * @RC_PROTO_RC5: Philips RC5 protocol
 * @RC_PROTO_RC5X_20: Philips RC5x 20 bit protocol
 * @RC_PROTO_RC5_SZ: StreamZap variant of RC5
 * @RC_PROTO_JVC: JVC protocol
 * @RC_PROTO_SONY12: Sony 12 bit protocol
 * @RC_PROTO_SONY15: Sony 15 bit protocol
 * @RC_PROTO_SONY20: Sony 20 bit protocol
 * @RC_PROTO_NEC: NEC protocol
 * @RC_PROTO_NECX: Extended NEC protocol
 * @RC_PROTO_NEC32: NEC 32 bit protocol
 * @RC_PROTO_SANYO: Sanyo protocol
 * @RC_PROTO_MCIR2_KBD: RC6-ish MCE keyboard
 * @RC_PROTO_MCIR2_MSE: RC6-ish MCE mouse
 * @RC_PROTO_RC6_0: Philips RC6-0-16 protocol
 * @RC_PROTO_RC6_6A_20: Philips RC6-6A-20 protocol
 * @RC_PROTO_RC6_6A_24: Philips RC6-6A-24 protocol
 * @RC_PROTO_RC6_6A_32: Philips RC6-6A-32 protocol
 * @RC_PROTO_RC6_MCE: MCE (Philips RC6-6A-32 subtype) protocol
 * @RC_PROTO_SHARP: Sharp protocol
 * @RC_PROTO_XMP: XMP protocol
 * @RC_PROTO_CEC: CEC protocol
 * @RC_PROTO_IMON: iMon Pad protocol
 * @RC_PROTO_RCMM12: RC-MM protocol 12 bits
 * @RC_PROTO_RCMM24: RC-MM protocol 24 bits
 * @RC_PROTO_RCMM32: RC-MM protocol 32 bits
 * @RC_PROTO_XBOX_DVD: Xbox DVD Movie Playback Kit protocol
 * @RC_PROTO_MAX: Maximum value of enum rc_proto
 */
enum rc_proto {
	RC_PROTO_UNKNOWN	= 0,
	RC_PROTO_OTHER		= 1,
	RC_PROTO_RC5		= 2,
	RC_PROTO_RC5X_20	= 3,
	RC_PROTO_RC5_SZ		= 4,
	RC_PROTO_JVC		= 5,
	RC_PROTO_SONY12		= 6,
	RC_PROTO_SONY15		= 7,
	RC_PROTO_SONY20		= 8,
	RC_PROTO_NEC		= 9,
	RC_PROTO_NECX		= 10,
	RC_PROTO_NEC32		= 11,
	RC_PROTO_SANYO		= 12,
	RC_PROTO_MCIR2_KBD	= 13,
	RC_PROTO_MCIR2_MSE	= 14,
	RC_PROTO_RC6_0		= 15,
	RC_PROTO_RC6_6A_20	= 16,
	RC_PROTO_RC6_6A_24	= 17,
	RC_PROTO_RC6_6A_32	= 18,
	RC_PROTO_RC6_MCE	= 19,
	RC_PROTO_SHARP		= 20,
	RC_PROTO_XMP		= 21,
	RC_PROTO_CEC		= 22,
	RC_PROTO_IMON		= 23,
	RC_PROTO_RCMM12		= 24,
	RC_PROTO_RCMM24		= 25,
	RC_PROTO_RCMM32		= 26,
	RC_PROTO_XBOX_DVD	= 27,
	RC_PROTO_MAX		= RC_PROTO_XBOX_DVD,
};

#endif
