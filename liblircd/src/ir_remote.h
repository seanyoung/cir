/****************************************************************************
** ir_remote.h *************************************************************
****************************************************************************
*
* ir_remote.h - describes and decodes the signals from IR remotes
*
* Copyright (C) 1996,97 Ralph Metzler <rjkm@thp.uni-koeln.de>
* Copyright (C) 1998 Christoph Bartelmus <lirc@bartelmus.de>
*
*/
/**
 *  @file ir_remote.h
 *  @author Ralph Metzler, Christoph Bartelmus
 *  @brief Describes and decodes the signals from IR remotes.
 *  @ingroup private_api
 *  @ingroup driver_api
 *  @addtogroup driver_api
 *  @{
 */

#ifndef IR_REMOTE_H
#define IR_REMOTE_H

#include <sys/types.h>
#include <sys/time.h>
#include <unistd.h>
#include <string.h>
#include <math.h>
#include <stdlib.h>

#include "driver.h"

#include "ir_remote_types.h"

#ifdef __cplusplus
extern "C" {
#endif


/** Create a malloc'd, deep copy of ncode. Use ncode_free() to dispose(). */
struct ir_ncode* ncode_dup(struct ir_ncode* ncode);

/** Dispose an ir_ncode instance obtained from ncode_dup(). */
void ncode_free(struct ir_ncode* ncode);


/**
 * TODO
 */
extern struct ir_remote* last_remote;


/**
 * Global pointer to the remote that contains the code currently
 * repeating. Defined in ir_remote.c.
 */
extern struct ir_remote* repeat_remote;

/**
 * Global pointer to the code currently repeating. Defined in ir_remote.c.
 */
extern struct ir_ncode* repeat_code;


static inline ir_code get_ir_code(const struct ir_ncode*	ncode,
				  const struct ir_code_node*	node)
{
	if (ncode->next && node != NULL)
		return node->code;
	return ncode->code;
}

static inline struct ir_code_node*
get_next_ir_code_node(const struct ir_ncode*		ncode,
		      const struct ir_code_node*	node)
{
	if (node == NULL)
		return ncode->next;
	return node->next;
}

static inline int bit_count(const struct ir_remote* remote)
{
	return remote->pre_data_bits + remote->bits + remote->post_data_bits;
}

static inline int bits_set(ir_code data)
{
	int ret = 0;

	while (data) {
		if (data & 1)
			ret++;
		data >>= 1;
	}
	return ret;
}

static inline ir_code reverse(ir_code data, int bits)
{
	int i;
	ir_code c;

	c = 0;
	for (i = 0; i < bits; i++)
		c |= (ir_code)(((data & (((ir_code)1) << i)) ? 1 : 0))
			<< (bits - 1 - i);
	return c;
}

static inline int is_pulse(lirc_t data)
{
	return ((data & LIRC_MODE2_MASK)==LIRC_MODE2_PULSE) ? 1 : 0;
}

static inline int is_space(lirc_t data)
{
	return ((data & LIRC_MODE2_MASK)==LIRC_MODE2_SPACE) ? 1 : 0;
}

static inline int is_timeout(lirc_t data)
{
	return ((data & LIRC_MODE2_MASK)==LIRC_MODE2_TIMEOUT) ? 1 : 0;
}

static inline int is_overflow(lirc_t data)
{
	return ((data & LIRC_MODE2_MASK)==LIRC_MODE2_OVERFLOW) ? 1 : 0;
}

static inline int has_repeat(const struct ir_remote* remote)
{
	if (remote->prepeat > 0 && remote->srepeat > 0)
		return 1;
	else
		return 0;
}

static inline void set_protocol(struct ir_remote* remote, int protocol)
{
	remote->flags &= ~(IR_PROTOCOL_MASK);
	remote->flags |= protocol;
}

static inline int is_raw(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == RAW_CODES)
		return 1;
	else
		return 0;
}

static inline int is_space_enc(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == SPACE_ENC)
		return 1;
	else
		return 0;
}

static inline int is_space_first(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == SPACE_FIRST)
		return 1;
	else
		return 0;
}

static inline int is_rc5(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == RC5)
		return 1;
	else
		return 0;
}

static inline int is_rc6(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == RC6 || remote->rc6_mask)
		return 1;
	else
		return 0;
}

static inline int is_biphase(const struct ir_remote* remote)
{
	if (is_rc5(remote) || is_rc6(remote))
		return 1;
	else
		return 0;
}

static inline int is_rcmm(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == RCMM)
		return 1;
	else
		return 0;
}

static inline int is_grundig(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == GRUNDIG)
		return 1;
	else
		return 0;
}

static inline int is_bo(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == BO)
		return 1;
	else
		return 0;
}

static inline int is_serial(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == SERIAL)
		return 1;
	else
		return 0;
}

static inline int is_xmp(const struct ir_remote* remote)
{
	if ((remote->flags & IR_PROTOCOL_MASK) == XMP)
		return 1;
	else
		return 0;
}

static inline int is_const(const struct ir_remote* remote)
{
	if (remote->flags & CONST_LENGTH)
		return 1;
	else
		return 0;
}

static inline int has_repeat_gap(const struct ir_remote* remote)
{
	if (remote->repeat_gap > 0)
		return 1;
	else
		return 0;
}

static inline int has_pre(const struct ir_remote* remote)
{
	if (remote->pre_data_bits > 0)
		return 1;
	else
		return 0;
}

static inline int has_post(const struct ir_remote* remote)
{
	if (remote->post_data_bits > 0)
		return 1;
	else
		return 0;
}

static inline int has_header(const struct ir_remote* remote)
{
	if (remote->phead > 0 && remote->shead > 0)
		return 1;
	else
		return 0;
}

static inline int has_foot(const struct ir_remote* remote)
{
	if (remote->pfoot > 0 && remote->sfoot > 0)
		return 1;
	else
		return 0;
}

static inline int has_toggle_bit_mask(const struct ir_remote* remote)
{
	if (remote->toggle_bit_mask > 0)
		return 1;
	else
		return 0;
}

static inline int has_ignore_mask(const struct ir_remote* remote)
{
	if (remote->ignore_mask > 0)
		return 1;
	else
		return 0;
}

static inline int has_repeat_mask(struct ir_remote* remote)
{
	if (remote->repeat_mask > 0)
		return 1;
	else
		return 0;
}

static inline int has_toggle_mask(const struct ir_remote* remote)
{
	if (remote->toggle_mask > 0)
		return 1;
	else
		return 0;
}

static inline lirc_t min_gap(const struct ir_remote* remote)
{
	if (remote->gap2 != 0 && remote->gap2 < remote->gap)
		return remote->gap2;
	else
		return remote->gap;
}

static inline lirc_t max_gap(const struct ir_remote* remote)
{
	if (remote->gap2 > remote->gap)
		return remote->gap2;
	else
		return remote->gap;
}

static inline unsigned int get_duty_cycle(const struct ir_remote* remote)
{
	if (remote->duty_cycle == 0)
		return 50;
	else if (remote->duty_cycle < 0)
		return 1;
	else if (remote->duty_cycle > 100)
		return 100;
	else
		return remote->duty_cycle;
}

/* check if delta is inside exdelta +/- exdelta*eps/100 */

static inline int expect(const struct ir_remote*	remote,
			 lirc_t				delta,
			 lirc_t				exdelta)
{
	int aeps = curr_driver->resolution > remote->aeps ?
		   curr_driver->resolution : remote->aeps;

	if (abs(exdelta - delta) <= exdelta * remote->eps / 100
	    || abs(exdelta - delta) <= aeps)
		return 1;
	return 0;
}

static inline int expect_at_least(const struct ir_remote*	remote,
				  lirc_t			delta,
				  lirc_t			exdelta)
{
	int aeps = curr_driver->resolution > remote->aeps ?
		   curr_driver->resolution : remote->aeps;

	if (delta + exdelta * remote->eps / 100 >= exdelta
	    || delta + aeps >= exdelta)
		return 1;
	return 0;
}

static inline int expect_at_most(const struct ir_remote*	remote,
				 lirc_t				delta,
				 lirc_t				exdelta)
{
	int aeps = curr_driver->resolution > remote->aeps ?
		   curr_driver->resolution : remote->aeps;

	if (delta <= exdelta + exdelta * remote->eps / 100
	    || delta <= exdelta + aeps)
		return 1;
	return 0;
}

static inline lirc_t upper_limit(const struct ir_remote* remote, lirc_t val)
{
	int aeps = curr_driver->resolution > remote->aeps ?
		   curr_driver->resolution : remote->aeps;
	lirc_t eps_val = val * (100 + remote->eps) / 100;
	lirc_t aeps_val = val + aeps;

	return eps_val > aeps_val ? eps_val : aeps_val;
}

static inline lirc_t lower_limit(const struct ir_remote* remote, lirc_t val)
{
	int aeps = curr_driver->resolution > remote->aeps ?
		   curr_driver->resolution : remote->aeps;
	lirc_t eps_val = val * (100 - remote->eps) / 100;
	lirc_t aeps_val = val - aeps;

	if (eps_val <= 0)
		eps_val = 1;
	if (aeps_val <= 0)
		aeps_val = 1;

	return eps_val < aeps_val ? eps_val : aeps_val;
}

/* only works if last <= current */
static inline unsigned long time_elapsed(const struct timeval*	last,
					 const struct timeval*	current)
{
	unsigned long secs, diff;

	secs = current->tv_sec - last->tv_sec;

	diff = 1000000 * secs + current->tv_usec - last->tv_usec;

	return diff;
}

static inline ir_code gen_mask(int bits)
{
	int i;
	ir_code mask;

	mask = 0;
	for (i = 0; i < bits; i++) {
		mask <<= 1;
		mask |= 1;
	}
	return mask;
}

static inline ir_code gen_ir_code(const struct ir_remote*	remote,
				  ir_code			pre,
				  ir_code			code,
				  ir_code			post)
{
	ir_code all;

	all = (pre & gen_mask(remote->pre_data_bits));
	all <<= remote->bits;
	all |= is_raw(remote) ? code : (code & gen_mask(remote->bits));
	all <<= remote->post_data_bits;
	all |= post & gen_mask(remote->post_data_bits);

	return all;
}

/**
 * Test if a given remote is in a list of remotes.
 *
 * @param remotes Head of linked list of remotes (using remote.next).
 * @param remote Pointer to remote to check
 * @return 1 if remote exists in remotes list, else 0
 */
const struct ir_remote* is_in_remotes(const struct ir_remote*	remotes,
				      const struct ir_remote*	remote);

/** Return ir_remote with given name in remotes list, or NULL if not found. */
struct ir_remote* get_ir_remote(const struct ir_remote* remotes,
				const char*		name);

void get_frequency_range(const struct ir_remote*	remotes,
			 unsigned int*			min_freq,
			 unsigned int*			max_freq);

void get_filter_parameters(const struct ir_remote*	remotes,
			   lirc_t*			max_gap_lengthp,
			   lirc_t*			min_pulse_lengthp,
			   lirc_t*			min_space_lengthp,
			   lirc_t*			max_pulse_lengthp,
			   lirc_t*			max_space_lengthp);

int map_code(const struct ir_remote*	remote,
	     struct decode_ctx_t*	ctx,
	     int			pre_bits,
	     ir_code			pre,
	     int			bits,
	     ir_code			code,
	     int			post_bits,
	     ir_code			post);

void map_gap(const struct ir_remote*	remote,
	     struct decode_ctx_t*	ctx,
	     const struct timeval*	start,
	     const struct timeval*	last,
	     lirc_t			signal_length);

/** Return code with given name in remote's list of codes or NULL. */
struct ir_ncode* get_code_by_name(const struct ir_remote*	remote,
				  const char*			name);

int write_message(char*		buffer,
		  size_t	size,
		  const char*	remote_name,
		  const char*	button_name,
		  const char*	button_suffix,
		  ir_code	code,
		  int		reps);

/**
 * Tries to decode current signal trying all known remotes. This is
 * non-blocking, failures could be retried later when more data is
 * available.
 *
 * @param remotes Parsed lircd.conf file as returned by read_config()
 * @return NULL on errors or no data available. Else a dynamically
 *     allocated string like "000000000000fad3 00 KEY_POWER apple".
 *     Caller owns string and eventually de-allocates it.
 */
char* decode_all(struct ir_remote* remotes);

/**
 * Transmits the actual code in the second  argument by calling the
 * current hardware driver.  The processing depends on global
 * repeat-remote. If this is not-NULL, the codes are sent using repeat
 * formatting if the remote supports it.
 * @param remote Currently active remote, used as database for timing,
 *     and as keeper of an internal state.
 * @param code IR code to be transmitted
 * @param delay If true (normal case), generate a delay corresponding
 *     to the time it takes to send the code. If not (test case), don't.
 * @return Non-zero if success.
 */
int send_ir_ncode(struct ir_remote* remote, struct ir_ncode* code, int delay);

#ifdef __cplusplus
}
#endif

/**
 * Initiate: define if dynamic codes should be used.
 *
 * @param use_dyncodes Should normally reflect "lircd:dynamic-codes" option.
 *
 */
void ir_remote_init(int use_dyncodes);

/** Return pointer to currently decoded remote. */
const struct ir_remote* get_decoding(void);

/** @} */

#endif
