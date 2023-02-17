/****************************************************************************
** ir_remote_types.h *******************************************************
****************************************************************************
*
* ir_remote_types.h - describes and decodes the signals from IR remotes
*
* Copyright (C) 1996,97 Ralph Metzler <rjkm@thp.uni-koeln.de>
* Copyright (C) 1998 Christoph Bartelmus <lirc@bartelmus.de>
*
*/

/**
 * @file ir_remote_types.h
 * @ingroup private_api
 * @ingroup private_api
 * @brief Describes and decodes the signals from IR remotes.
 */

#ifndef IR_REMOTE_TYPES_H
#define IR_REMOTE_TYPES_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

#include <sys/time.h>
#include <unistd.h>
#include <string.h>
#include <math.h>
#include <stdlib.h>
#if defined(__linux__)
#include <linux/types.h>
#endif

#include "lirc.h"

/**
 * Denotes an internal coded representation for an IR transmission.
 *  Note that there may be more than one ir_code in one ir_ncode.
 */
typedef uint64_t ir_code;

/**
 * An ir_code for entering into (singly) linked lists,
 *  i.e. with a pointer to other ir_code_node.
 */
struct ir_code_node {
	ir_code			code;
	struct ir_code_node*	next;
};

/**
 * IR Command, corresponding to one (command defining) line of the
 * configuration file.
 * Normally, there is only one ir_code herein (field code), however, there
 *  may be more, namely in the linked list in the field next.
 */
struct ir_ncode {
	/** Name of command */
	char*			name;

	/** The first code of the command*/
	ir_code			code;

	/** (private) */
	int			length;

	/** (private) */
	lirc_t*			signals;

	/** Linked list of the subsequent ir_code's, after the first one.
	 *  NULL if there is only one ir_code (normal case). */
	struct ir_code_node*	next;

	/** Should point at the ir_code currently being transmitted,
	 * or NULL if none. */
	struct ir_code_node*	current;

	/** (private) */
	struct ir_code_node*	transmit_state;

	/** Next code in recorded buttons list. */
	struct ir_ncode*	next_ncode;
};

/*
 * struct ir_remote
 * defines the encoding of a remote control
 */

/* definitions for flags */

#define IR_PROTOCOL_MASK 0x07ff

/* protocols: must not be combined */
/* Don't forget to take a look at config_file.h when adding new flags */

#define RAW_CODES       0x0001  /**< for internal use only */
#define RC5             0x0002  /**< IR data follows RC5 protocol */
#define SHIFT_ENC          RC5  /**< IR data is shift encoded (name obsolete) */
/* Hm, RC6 protocols seem to have changed the biphase semantics so
 * that lircd will calculate the bit-wise complement of the codes. But
 * this is only a guess as I did not have a datasheet... */

#define RC6             0x0004  /**< IR data follows RC6 protocol */
#define RCMM            0x0008  /**< IR data follows RC-MM protocol */
#define SPACE_ENC       0x0010  /**< IR data is space encoded */
#define SPACE_FIRST     0x0020  /**< bits are encoded as space+pulse */
#define GRUNDIG         0x0080  /**< encoding found on Grundig remote */
#define BO              0x0100  /**< encoding found on Bang & Olufsen remote */
#define SERIAL          0x0200  /**< serial protocol */
#define XMP             0x0400  /**< XMP protocol */

/* additinal flags: can be orred together with protocol flag */
#define REVERSE         0x0800
#define NO_HEAD_REP     0x1000          /**< no header for key repeats */
#define NO_FOOT_REP     0x2000          /**< no foot for key repeats */
#define CONST_LENGTH    0x4000          /**< signal length+gap is always constant */
#define REPEAT_HEADER   0x8000          /**< header is also sent before repeat code */

#define COMPAT_REVERSE  0x00010000      /**< compatibility mode for REVERSE flag */

/** stop repeating after 600 signals (approx. 1 minute)
 * update technical.html when changing this value
 */
#define REPEAT_MAX_DEFAULT 600

#define DEFAULT_FREQ 38000

#define IR_PARITY_NONE 0
#define IR_PARITY_EVEN 1
#define IR_PARITY_ODD  2

/** State describing code, pre, post + gap and repeat state. */
struct decode_ctx_t {
	ir_code code;                   /**< Code part, matched to code defintion. */
	ir_code pre;                    /**< pre data, before code. */
	ir_code post;                   /**< post data, sent after code. */
	int	repeat_flag;            /**< True if code is a repeated one. */
	lirc_t	max_remaining_gap;      /**< Estimated max time of trailing gap.*/
	lirc_t	min_remaining_gap;      /**< Estimated min time of trailing gap.*/
};


/**
 * One remote as represented in the configuration file.
 */
struct ir_remote {
	const char*		name;   /**< name of remote control */
	const char*		driver; /**< Name of driver for LIRCCODE cases. */
	struct ir_ncode*	codes;
	int			bits;   /**< bits (length of code) */
	int			flags;  /**< flags */
	int			eps;    /**< eps (_relative_ tolerance) */
	unsigned int            aeps;   /**< detecting _very short_ pulses is
					 * difficult with relative tolerance
					 * for some remotes,
					 * this is an _absolute_ tolerance
					 * to solve this problem
					 * usually you can say 0 here. */
	char*		dyncodes_name;  /**< name for unknown buttons */
	int		dyncode;        /**< last received code */
	struct ir_ncode dyncodes[2];    /**< helper structs for unknown buttons */

	/* pulse and space lengths of: */

	lirc_t		phead, shead;           /**< header */
	lirc_t		pthree, sthree;         /**< 3 (only used for RC-MM) */
	lirc_t		ptwo, stwo;             /**< 2 (only used for RC-MM) */
	lirc_t		pone, sone;             /**< 1 */
	lirc_t		pzero, szero;           /**< 0 */
	lirc_t		plead;                  /**< leading pulse */
	lirc_t		ptrail;                 /**< trailing pulse */
	lirc_t		pfoot, sfoot;           /**< foot */
	lirc_t		prepeat, srepeat;       /**< indicate repeating */

	int		pre_data_bits;          /**< length of pre_data */
	ir_code		pre_data;               /**< data which the remote sends before actual keycode */
	int		post_data_bits;         /**< length of post_data */
	ir_code		post_data;              /**< data which the remote sends after actual keycode */
	lirc_t		pre_p, pre_s;           /**< signal between pre_data and keycode */
	lirc_t		post_p, post_s;         /**< signal between keycode and post_code */

	uint32_t	gap;                    /**< time between signals in usecs */
	uint32_t	gap2;                   /**< time between signals in usecs */
	uint32_t	repeat_gap;             /**< time between two repeat codes if different from gap */
	int		toggle_bit;             /**< obsolete */
	ir_code		toggle_bit_mask;        /**< previously only one bit called toggle_bit */
	int		suppress_repeat;        /**< suppress unwanted repeats */
	/** code is repeated at least x times
	 * code sent once -> min_repeat=0 */
	int		min_repeat;
	/**
	 * meaningful only if remote sends
	 *				   a repeat code: in this case
	 *				   this value indicates how often
	 *				   the real code is repeated
	 *				   before the repeat code is being
	 *				   sent */
	unsigned int		min_code_repeat;
	unsigned int		freq;           /**< modulation frequency */
	unsigned int		duty_cycle;     /**< 0<duty cycle<=100 default: 50 */
	ir_code			toggle_mask;    /**< Sharp (?) error detection scheme */
	ir_code			rc6_mask;       /**< RC-6 doubles signal length of some bits */

	/* serial protocols */
	unsigned int		baud;           /**< can be overridden by [p|s]zero, [p|s]one */
	unsigned int		bits_in_byte;   /**< default: 8 */
	unsigned int		parity;         /**< currently unsupported */
	unsigned int		stop_bits;      /**< mapping: 1->2 1.5->3 2->4 */

	/** mask defines which bits can be
	 * ignored when matching a code */
	ir_code			ignore_mask;
	ir_code			repeat_mask; /**< mask defines which bits are inverted for repeats */
	/* end of user editable values */

	ir_code			toggle_bit_mask_state;
	int			toggle_mask_state;
	int			repeat_countdown;
	struct ir_ncode*	last_code;                      /**< code received or sent last */
	struct ir_ncode*	toggle_code;                    /**< toggle code received or sent last */
	int			reps;
	struct timeval		last_send;                      /**< time last_code was received or sent */
	lirc_t			min_remaining_gap;              /**< remember gap for CONST_LENGTH remotes */
	lirc_t			max_remaining_gap;              /**< gap range */

	lirc_t			min_total_signal_length;        /**< how long is the shortest signal including gap */
	lirc_t			max_total_signal_length;        /**< how long is the longest signal including gap */
	lirc_t			min_gap_length;                 /**< how long is the shortest gap */
	lirc_t			max_gap_length;                 /**< how long is the longest gap */
	lirc_t			min_pulse_length, max_pulse_length;
	lirc_t			min_space_length, max_space_length;
	int			release_detected;       /**< set by release generator */
	int			manual_sort;            /**< If set in any remote, disables automatic sorting. */
	struct ir_remote*	next;
};

#ifdef __cplusplus
}
#endif

#endif
