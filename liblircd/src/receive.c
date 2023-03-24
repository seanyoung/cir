/****************************************************************************
 * receive.c ***************************************************************
 ****************************************************************************
 *
 * functions that decode IR codes
 *
 * Copyright (C) 1999 Christoph Bartelmus <lirc@bartelmus.de>
 *
 */

/**
 * @file receive.c
 * @author Christoph Bartelmus
 * @brief Implements receive.h
 */

#ifdef HAVE_CONFIG_H
# include <config.h>
#endif
#define LIRC_EOF                0x08000000
#include <errno.h>
#include <limits.h>
#include <poll.h>
#include <stdint.h>

#include "lirc.h"

#include "driver.h"
#include "lirc_log.h"
#include "receive.h"
#include "ir_remote.h"

#define RBUF_SIZE 2560

#define REC_SYNC 8

static const logchannel_t logchannel = LOG_LIB;

/**
 * Structure for the receiving buffer.
 */
struct rbuf {
	lirc_t		data[RBUF_SIZE];
	ir_code		decoded;
	int		rptr;
	int		wptr;
	int		too_long;
	int		is_biphase;
	lirc_t		pendingp;
	lirc_t		pendings;
	lirc_t		sum;
	struct timeval	last_signal_time;
	int		at_eof;
	FILE*		input_log;
};


/**
 * Global receiver buffer.
 */
static struct rbuf rec_buffer;
static int update_mode = 0;


void rec_set_update_mode(int mode)
{
	update_mode = mode;
}

int (*lircd_waitfordata)(uint32_t timeout) = NULL;


static lirc_t readdata(lirc_t timeout)
{
	lirc_t data;

	data = curr_driver->readdata(timeout);
	rec_buffer.at_eof = data & LIRC_EOF ? 1 : 0;
	if (rec_buffer.at_eof)
		log_debug("receive: Got EOF");
	return data;
}


static lirc_t lirc_t_max(lirc_t a, lirc_t b)
{
	return a > b ? a : b;
}

static void set_pending_pulse(lirc_t deltap)
{
	log_trace2("pending pulse: %lu", deltap);
	rec_buffer.pendingp = deltap;
}

static void set_pending_space(lirc_t deltas)
{
	log_trace2("pending space: %lu", deltas);
	rec_buffer.pendings = deltas;
}


static void log_input(lirc_t data)
{
	fprintf(rec_buffer.input_log, "%s %u\n",
		data & PULSE_BIT ? "pulse" : "space", data & PULSE_MASK);
	fflush(rec_buffer.input_log);
}


static lirc_t get_next_rec_buffer_internal(lirc_t maxusec)
{
	if (rec_buffer.rptr < rec_buffer.wptr) {
		log_trace2("<%c%lu", rec_buffer.data[rec_buffer.rptr] & PULSE_BIT ? 'p' : 's', (uint32_t)
			  rec_buffer.data[rec_buffer.rptr] & (PULSE_MASK));
		rec_buffer.sum += rec_buffer.data[rec_buffer.rptr] & (PULSE_MASK);
		return rec_buffer.data[rec_buffer.rptr++];
	}
	if (rec_buffer.wptr < RBUF_SIZE) {
		lirc_t data = 0;
		unsigned long elapsed = 0;

		if (timerisset(&rec_buffer.last_signal_time)) {
			struct timeval current;

			gettimeofday(&current, NULL);
			elapsed = time_elapsed(&rec_buffer.last_signal_time, &current);
		}
		if (elapsed < maxusec)
			data = readdata(maxusec - elapsed);
		if (!data) {
			log_trace2("timeout: %u", maxusec);
			return 0;
		}
		if (data & LIRC_EOF) {
			log_debug("Receive: returning EOF");
			return data;
		}
		if (LIRC_IS_TIMEOUT(data)) {
			log_trace("timeout received: %lu", (uint32_t)LIRC_VALUE(data));
			if (LIRC_VALUE(data) < maxusec)
				return get_next_rec_buffer_internal(maxusec - LIRC_VALUE(data));
			return 0;
		}

		rec_buffer.data[rec_buffer.wptr] = data;
		if (rec_buffer.input_log != NULL)
			log_input(data);
		if (rec_buffer.data[rec_buffer.wptr] == 0)
			return 0;
		rec_buffer.sum += rec_buffer.data[rec_buffer.rptr]
				  & (PULSE_MASK);
		rec_buffer.wptr++;
		rec_buffer.rptr++;
		log_trace2("+%c%lu", rec_buffer.data[rec_buffer.rptr - 1] & PULSE_BIT ? 'p' : 's', (uint32_t)
			  rec_buffer.data[rec_buffer.rptr - 1]
			  & (PULSE_MASK));
		return rec_buffer.data[rec_buffer.rptr - 1];
	}
	rec_buffer.too_long = 1;
	return 0;
}


void set_waitfordata_func(int(*func)(uint32_t maxusec))
{
	lircd_waitfordata = func;
}

void rec_buffer_set_logfile(FILE* f)
{
	if (rec_buffer.input_log != NULL)
		fclose(rec_buffer.input_log);
	rec_buffer.input_log = f;
}


static lirc_t get_next_rec_buffer(lirc_t maxusec)
{
	return get_next_rec_buffer_internal(receive_timeout(maxusec));
}

void rec_buffer_init(void)
{
	memset(&rec_buffer, 0, sizeof(rec_buffer));
}

void rec_buffer_rewind(void)
{
	rec_buffer.rptr = 0;
	rec_buffer.too_long = 0;
	set_pending_pulse(0);
	set_pending_space(0);
	rec_buffer.sum = 0;
	rec_buffer.at_eof = 0;
}

void rec_buffer_reset_wptr(void)
{
	rec_buffer.wptr = 0;
}

int rec_buffer_clear(void)
{
	int move, i;

	timerclear(&rec_buffer.last_signal_time);
	if (curr_driver->rec_mode == LIRC_MODE_LIRCCODE) {
		unsigned char buffer[curr_driver->code_length/CHAR_BIT + 1];
		size_t count;

		count = curr_driver->code_length / CHAR_BIT;
		if (curr_driver->code_length % CHAR_BIT)
			count++;

		if (read(curr_driver->fd, buffer, count) != count) {
			log_error("reading in mode LIRC_MODE_LIRCCODE failed");
			return 0;
		}
		for (i = 0, rec_buffer.decoded = 0; i < count; i++)
			rec_buffer.decoded = (rec_buffer.decoded << CHAR_BIT) + ((ir_code)buffer[i]);
	} else {
		lirc_t data;

		move = rec_buffer.wptr - rec_buffer.rptr;
		if (move > 0 && rec_buffer.rptr > 0) {
			memmove(&rec_buffer.data[0], &rec_buffer.data[rec_buffer.rptr],
				sizeof(rec_buffer.data[0]) * move);
			rec_buffer.wptr -= rec_buffer.rptr;
		} else {
			rec_buffer.wptr = 0;
			data = readdata(0);

			log_trace2("c%lu", (uint32_t)data & (PULSE_MASK));

			rec_buffer.data[rec_buffer.wptr] = data;
			rec_buffer.wptr++;
		}
	}

	rec_buffer_rewind();
	rec_buffer.is_biphase = 0;

	return 1;
}

static void unget_rec_buffer(int count)
{
	log_trace2("unget: %d", count);
	if (count == 1 || count == 2) {
		rec_buffer.rptr -= count;
		rec_buffer.sum -= rec_buffer.data[rec_buffer.rptr] & (PULSE_MASK);
		if (count == 2)
			rec_buffer.sum -= rec_buffer.data[rec_buffer.rptr + 1]
					  & (PULSE_MASK);
	}
}

static void unget_rec_buffer_delta(lirc_t delta)
{
	rec_buffer.rptr--;
	rec_buffer.sum -= delta & (PULSE_MASK);
	rec_buffer.data[rec_buffer.rptr] = delta;
}

static lirc_t get_next_pulse(lirc_t maxusec)
{
	lirc_t data;

	data = get_next_rec_buffer(maxusec);
	if (data == 0)
		return 0;
	if (!is_pulse(data)) {
		log_trace1("pulse expected");
		return 0;
	}
	return data & (PULSE_MASK);
}

static lirc_t get_next_space(lirc_t maxusec)
{
	lirc_t data;

	data = get_next_rec_buffer(maxusec);
	if (data == 0)
		return 0;
	if (!is_space(data)) {
		log_trace1("space expected");
		return 0;
	}
	return data;
}

static int sync_pending_pulse(struct ir_remote* remote)
{
	if (rec_buffer.pendingp > 0) {
		lirc_t deltap;

		deltap = get_next_pulse(rec_buffer.pendingp);
		if (deltap == 0)
			return 0;
		if (!expect(remote, deltap, rec_buffer.pendingp))
			return 0;
		set_pending_pulse(0);
	}
	return 1;
}

static int sync_pending_space(struct ir_remote* remote)
{
	if (rec_buffer.pendings > 0) {
		lirc_t deltas;

		deltas = get_next_space(rec_buffer.pendings);
		if (deltas == 0)
			return 0;
		if (!expect(remote, deltas, rec_buffer.pendings))
			return 0;
		set_pending_space(0);
	}
	return 1;
}

static int expectpulse(struct ir_remote* remote, int exdelta)
{
	lirc_t deltap;
	int retval;

	log_trace2("expecting pulse: %lu", exdelta);
	if (!sync_pending_space(remote))
		return 0;

	deltap = get_next_pulse(rec_buffer.pendingp + exdelta);
	if (deltap == 0)
		return 0;
	if (rec_buffer.pendingp > 0) {
		if (rec_buffer.pendingp > deltap)
			return 0;
		retval = expect(remote, deltap - rec_buffer.pendingp, exdelta);
		if (!retval)
			return 0;
		set_pending_pulse(0);
	} else {
		retval = expect(remote, deltap, exdelta);
	}
	return retval;
}

static int expectspace(struct ir_remote* remote, int exdelta)
{
	lirc_t deltas;
	int retval;

	log_trace2("expecting space: %lu", exdelta);
	if (!sync_pending_pulse(remote))
		return 0;

	deltas = get_next_space(rec_buffer.pendings + exdelta);
	if (deltas == 0)
		return 0;
	if (rec_buffer.pendings > 0) {
		if (rec_buffer.pendings > deltas)
			return 0;
		retval = expect(remote, deltas - rec_buffer.pendings, exdelta);
		if (!retval)
			return 0;
		set_pending_space(0);
	} else {
		retval = expect(remote, deltas, exdelta);
	}
	return retval;
}

static int expectone(struct ir_remote* remote, int bit)
{
	if (is_biphase(remote)) {
		int all_bits = bit_count(remote);
		ir_code mask;

		mask = ((ir_code)1) << (all_bits - 1 - bit);
		if (mask & remote->rc6_mask) {
			if (remote->sone > 0 && !expectspace(remote, 2 * remote->sone)) {
				unget_rec_buffer(1);
				return 0;
			}
			set_pending_pulse(2 * remote->pone);
		} else {
			if (remote->sone > 0 && !expectspace(remote, remote->sone)) {
				unget_rec_buffer(1);
				return 0;
			}
			set_pending_pulse(remote->pone);
		}
	} else if (is_space_first(remote)) {
		if (remote->sone > 0 && !expectspace(remote, remote->sone)) {
			unget_rec_buffer(1);
			return 0;
		}
		if (remote->pone > 0 && !expectpulse(remote, remote->pone)) {
			unget_rec_buffer(2);
			return 0;
		}
	} else {
		if (remote->pone > 0 && !expectpulse(remote, remote->pone)) {
			unget_rec_buffer(1);
			return 0;
		}
		if (remote->ptrail > 0) {
			if (remote->sone > 0 && !expectspace(remote, remote->sone)) {
				unget_rec_buffer(2);
				return 0;
			}
		} else {
			set_pending_space(remote->sone);
		}
	}
	return 1;
}

static int expectzero(struct ir_remote* remote, int bit)
{
	if (is_biphase(remote)) {
		int all_bits = bit_count(remote);
		ir_code mask;

		mask = ((ir_code)1) << (all_bits - 1 - bit);
		if (mask & remote->rc6_mask) {
			if (!expectpulse(remote, 2 * remote->pzero)) {
				unget_rec_buffer(1);
				return 0;
			}
			set_pending_space(2 * remote->szero);
		} else {
			if (!expectpulse(remote, remote->pzero)) {
				unget_rec_buffer(1);
				return 0;
			}
			set_pending_space(remote->szero);
		}
	} else if (is_space_first(remote)) {
		if (remote->szero > 0 && !expectspace(remote, remote->szero)) {
			unget_rec_buffer(1);
			return 0;
		}
		if (remote->pzero > 0 && !expectpulse(remote, remote->pzero)) {
			unget_rec_buffer(2);
			return 0;
		}
	} else {
		if (!expectpulse(remote, remote->pzero)) {
			unget_rec_buffer(1);
			return 0;
		}
		if (remote->ptrail > 0) {
			if (!expectspace(remote, remote->szero)) {
				unget_rec_buffer(2);
				return 0;
			}
		} else {
			set_pending_space(remote->szero);
		}
	}
	return 1;
}

static lirc_t sync_rec_buffer(struct ir_remote* remote)
{
	int count;
	lirc_t deltas, deltap;

	count = 0;
	deltas = get_next_space(1000000);
	if (deltas == 0)
		return 0;

	if (last_remote != NULL && !is_rcmm(remote)) {
		while (!expect_at_least(last_remote, deltas, last_remote->min_remaining_gap)) {
			deltap = get_next_pulse(1000000);
			if (deltap == 0)
				return 0;
			deltas = get_next_space(1000000);
			if (deltas == 0)
				return 0;
			count++;
			if (count > REC_SYNC)   /* no sync found,
						 * let's try a diffrent remote */
				return 0;
		}
		if (has_toggle_mask(remote)) {
			if (!expect_at_most(last_remote, deltas, last_remote->max_remaining_gap)) {
				remote->toggle_mask_state = 0;
				remote->toggle_code = NULL;
			}
		}
	}
	rec_buffer.sum = 0;
	return deltas;
}

static int get_header(struct ir_remote* remote)
{
	if (is_rcmm(remote)) {
		lirc_t deltap, deltas, sum;

		deltap = get_next_pulse(remote->phead);
		if (deltap == 0) {
			unget_rec_buffer(1);
			return 0;
		}
		deltas = get_next_space(remote->shead);
		if (deltas == 0) {
			unget_rec_buffer(2);
			return 0;
		}
		sum = deltap + deltas;
		if (expect(remote, sum, remote->phead + remote->shead))
			return 1;
		unget_rec_buffer(2);
		return 0;
	} else if (is_bo(remote)) {
		if (expectpulse(remote, remote->pone) && expectspace(remote, remote->sone)
		    && expectpulse(remote, remote->pone) && expectspace(remote, remote->sone)
		    && expectpulse(remote, remote->phead) && expectspace(remote, remote->shead))
			return 1;
		return 0;
	}
	if (remote->shead == 0) {
		if (!sync_pending_space(remote))
			return 0;
		set_pending_pulse(remote->phead);
		return 1;
	}
	if (!expectpulse(remote, remote->phead)) {
		unget_rec_buffer(1);
		return 0;
	}
	/* if this flag is set I need a decision now if this is really
	 * a header */
	if (remote->flags & NO_HEAD_REP) {
		lirc_t deltas;

		deltas = get_next_space(remote->shead);
		if (deltas != 0) {
			if (expect(remote, remote->shead, deltas))
				return 1;
			unget_rec_buffer(2);
			return 0;
		}
	}

	set_pending_space(remote->shead);
	return 1;
}

static int get_foot(struct ir_remote* remote)
{
	if (!expectspace(remote, remote->sfoot))
		return 0;
	if (!expectpulse(remote, remote->pfoot))
		return 0;
	return 1;
}

static int get_lead(struct ir_remote* remote)
{
	if (remote->plead == 0)
		return 1;
	if (!sync_pending_space(remote))
		return 0;
	set_pending_pulse(remote->plead);
	return 1;
}

static int get_trail(struct ir_remote* remote)
{
	if (remote->ptrail != 0)
		if (!expectpulse(remote, remote->ptrail))
			return 0;
	if (rec_buffer.pendingp > 0)
		if (!sync_pending_pulse(remote))
			return 0;
	return 1;
}

static int get_gap(struct ir_remote* remote, lirc_t gap)
{
	lirc_t data;

	log_trace1("sum: %d", rec_buffer.sum);
	data = get_next_rec_buffer(gap - gap * remote->eps / 100);
	if (data == 0)
		return 1;
	if (!is_space(data)) {
		log_trace1("space expected");
		return 0;
	}
	unget_rec_buffer(1);
	if (!expect_at_least(remote, data, gap)) {
		log_trace("end of signal not found");
		return 0;
	}
	return 1;
}

static int get_repeat(struct ir_remote* remote)
{
	if (!get_lead(remote))
		return 0;
	if (is_biphase(remote)) {
		if (!expectspace(remote, remote->srepeat))
			return 0;
		if (!expectpulse(remote, remote->prepeat))
			return 0;
	} else {
		if (!expectpulse(remote, remote->prepeat))
			return 0;
		set_pending_space(remote->srepeat);
	}
	if (!get_trail(remote))
		return 0;
	if (!get_gap
		    (remote,
		    is_const(remote) ? (min_gap(remote) >
					rec_buffer.sum ?
					min_gap(remote) - rec_buffer.sum : 0) :
		    (has_repeat_gap(remote) ? remote->repeat_gap : min_gap(remote))
		    ))
		return 0;
	return 1;
}

static ir_code get_data(struct ir_remote* remote, int bits, int done)
{
	ir_code code;
	int i;

	code = 0;

	if (is_rcmm(remote)) {
		lirc_t deltap, deltas, sum;

		if (bits % 2 || done % 2) {
			log_error("invalid bit number.");
			return (ir_code) -1;
		}
		if (!sync_pending_space(remote))
			return 0;
		for (i = 0; i < bits; i += 2) {
			code <<= 2;
			deltap = get_next_pulse(remote->pzero + remote->pone + remote->ptwo + remote->pthree);
			deltas = get_next_space(remote->szero + remote->sone + remote->stwo + remote->sthree);
			if (deltap == 0 || deltas == 0) {
				log_error("failed on bit %d", done + i + 1);
				return (ir_code) -1;
			}
			sum = deltap + deltas;
			log_trace2("rcmm: sum %ld", (uint32_t)sum);
			if (expect(remote, sum, remote->pzero + remote->szero)) {
				code |= 0;
				log_trace1("00");
			} else if (expect(remote, sum, remote->pone + remote->sone)) {
				code |= 1;
				log_trace1("01");
			} else if (expect(remote, sum, remote->ptwo + remote->stwo)) {
				code |= 2;
				log_trace1("10");
			} else if (expect(remote, sum, remote->pthree + remote->sthree)) {
				code |= 3;
				log_trace1("11");
			} else {
				log_trace1("no match for %d+%d=%d", deltap, deltas, sum);
				return (ir_code) -1;
			}
		}
		return code;
	} else if (is_grundig(remote)) {
		lirc_t deltap, deltas, sum;
		int state, laststate;

		if (bits % 2 || done % 2) {
			log_error("invalid bit number.");
			return (ir_code) -1;
		}
		if (!sync_pending_pulse(remote))
			return (ir_code) -1;
		for (laststate = state = -1, i = 0; i < bits; ) {
			deltas = get_next_space(remote->szero + remote->sone + remote->stwo + remote->sthree);
			deltap = get_next_pulse(remote->pzero + remote->pone + remote->ptwo + remote->pthree);
			if (deltas == 0 || deltap == 0) {
				log_error("failed on bit %d", done + i + 1);
				return (ir_code) -1;
			}
			sum = deltas + deltap;
			log_trace2("grundig: sum %ld", (uint32_t)sum);
			if (expect(remote, sum, remote->szero + remote->pzero)) {
				state = 0;
				log_trace1("2T");
			} else if (expect(remote, sum, remote->sone + remote->pone)) {
				state = 1;
				log_trace1("3T");
			} else if (expect(remote, sum, remote->stwo + remote->ptwo)) {
				state = 2;
				log_trace1("4T");
			} else if (expect(remote, sum, remote->sthree + remote->pthree)) {
				state = 3;
				log_trace2("6T");
			} else {
				log_trace1("no match for %d+%d=%d", deltas, deltap, sum);
				return (ir_code) -1;
			}
			if (state == 3) {       /* 6T */
				i += 2;
				code <<= 2;
				state = -1;
				code |= 0;
			} else if (laststate == 2 && state == 0) {      /* 4T2T */
				i += 2;
				code <<= 2;
				state = -1;
				code |= 1;
			} else if (laststate == 1 && state == 1) {      /* 3T3T */
				i += 2;
				code <<= 2;
				state = -1;
				code |= 2;
			} else if (laststate == 0 && state == 2) {      /* 2T4T */
				i += 2;
				code <<= 2;
				state = -1;
				code |= 3;
			} else if (laststate == -1) {
				/* 1st bit */
			} else {
				log_error("invalid state %d:%d", laststate, state);
				return (ir_code) -1;
			}
			laststate = state;
		}
		return code;
	} else if (is_serial(remote)) {
		int received;
		int space, stop_bit, parity_bit;
		int parity;
		lirc_t delta, origdelta, pending, expecting, gap_delta;
		lirc_t base, stop;
		lirc_t max_space, max_pulse;

		base = 1000000 / remote->baud;

		/* start bit */
		set_pending_pulse(base);

		received = 0;
		space = (rec_buffer.pendingp == 0);     /* expecting space ? */
		stop_bit = 0;
		parity_bit = 0;
		delta = origdelta = 0;
		stop = base * remote->stop_bits / 2;
		parity = 0;
		gap_delta = 0;

		max_space = remote->sone * remote->bits_in_byte + stop;
		max_pulse = remote->pzero * (1 + remote->bits_in_byte);
		if (remote->parity != IR_PARITY_NONE) {
			parity_bit = 1;
			max_space += remote->sone;
			max_pulse += remote->pzero;
			bits += bits / remote->bits_in_byte;
		}

		while (received < bits || stop_bit) {
			if (delta == 0) {
				delta = space ? get_next_space(max_space) : get_next_pulse(max_pulse);
				if (delta == 0 && space && received + remote->bits_in_byte + parity_bit >= bits)
					/* open end */
					delta = max_space;
				origdelta = delta;
			}
			if (delta == 0) {
				log_trace("failed before bit %d", received + 1);
				return (ir_code) -1;
			}
			pending = (space ? rec_buffer.pendings : rec_buffer.pendingp);
			if (expect(remote, delta, pending)) {
				delta = 0;
			} else if (delta > pending) {
				delta -= pending;
			} else {
				log_trace("failed before bit %d", received + 1);
				return (ir_code) -1;
			}
			if (pending > 0) {
				if (stop_bit) {
					log_trace2("delta: %lu", delta);
					gap_delta = delta;
					delta = 0;
					set_pending_pulse(base);
					set_pending_space(0);
					stop_bit = 0;
					space = 0;
					log_trace2("stop bit found");
				} else {
					log_trace2("pending bit found");
					set_pending_pulse(0);
					set_pending_space(0);
					if (delta == 0)
						space = (space ? 0 : 1);
				}
				continue;
			}
			expecting = (space ? remote->sone : remote->pzero);
			if (delta > expecting || expect(remote, delta, expecting)) {
				delta -= (expecting > delta ? delta : expecting);
				received++;
				code <<= 1;
				code |= space;
				parity ^= space;
				log_trace1("adding %d", space);
				if (received % (remote->bits_in_byte + parity_bit) == 0) {
					ir_code temp;

					if ((remote->parity == IR_PARITY_EVEN && parity)
					    || (remote->parity == IR_PARITY_ODD && !parity)) {
						log_trace("parity error after %d bits", received + 1);
						return (ir_code) -1;
					}
					parity = 0;

					/* parity bit is filtered out */
					temp = code >> (remote->bits_in_byte + parity_bit);
					code =
						temp << remote->bits_in_byte | reverse(code >> parity_bit,
										       remote->bits_in_byte);

					if (space && delta == 0) {
						log_trace("failed at stop bit after %d bits", received + 1);
						return (ir_code) -1;
					}
					log_trace2("awaiting stop bit");
					set_pending_space(stop);
					stop_bit = 1;
				}
			} else {
				if (delta == origdelta) {
					log_trace("framing error after %d bits", received + 1);
					return (ir_code) -1;
				}
				delta = 0;
			}
			if (delta == 0)
				space = (space ? 0 : 1);
		}
		if (gap_delta)
			unget_rec_buffer_delta(gap_delta);
		set_pending_pulse(0);
		set_pending_space(0);
		return code;
	} else if (is_bo(remote)) {
		int lastbit = 1;
		lirc_t deltap, deltas;
		lirc_t pzero, szero;
		lirc_t pone, sone;

		for (i = 0; i < bits; i++) {
			code <<= 1;
			deltap = get_next_pulse(remote->pzero + remote->pone + remote->ptwo + remote->pthree);
			deltas = get_next_space(remote->szero + remote->sone + remote->stwo + remote->sthree);
			if (deltap == 0 || deltas == 0) {
				log_error("failed on bit %d", done + i + 1);
				return (ir_code) -1;
			}
			if (lastbit == 1) {
				pzero = remote->pone;
				szero = remote->sone;
				pone = remote->ptwo;
				sone = remote->stwo;
			} else {
				pzero = remote->ptwo;
				szero = remote->stwo;
				pone = remote->pthree;
				sone = remote->sthree;
			}
			log_trace2("%lu %lu %lu %lu", pzero, szero, pone, sone);
			if (expect(remote, deltap, pzero)) {
				if (expect(remote, deltas, szero)) {
					code |= 0;
					lastbit = 0;
					log_trace1("0");
					continue;
				}
			}

			if (expect(remote, deltap, pone)) {
				if (expect(remote, deltas, sone)) {
					code |= 1;
					lastbit = 1;
					log_trace1("1");
					continue;
				}
			}
			log_error("failed on bit %d", done + i + 1);
			return (ir_code) -1;
		}
		return code;
	} else if (is_xmp(remote)) {
		lirc_t deltap, deltas, sum;
		ir_code n;

		if (bits % 4 || done % 4) {
			log_error("invalid bit number.");
			return (ir_code) -1;
		}
		if (!sync_pending_space(remote))
			return 0;
		for (i = 0; i < bits; i += 4) {
			code <<= 4;
			deltap = get_next_pulse(remote->pzero);
			deltas = get_next_space(remote->szero + 16 * remote->sone);
			if (deltap == 0 || deltas == 0) {
				log_error("failed on bit %d", done + i + 1);
				return (ir_code) -1;
			}
			sum = deltap + deltas;

			sum -= remote->pzero + remote->szero;
			n = (sum + remote->sone / 2) / remote->sone;
			if (n >= 16) {
				log_error("failed on bit %d", done + i + 1);
				return (ir_code) -1;
			}
			log_trace("%d: %lx", i, n);
			code |= n;
		}
		return code;
	}

	for (i = 0; i < bits; i++) {
		code = code << 1;
		if (expectone(remote, done + i)) {
			log_trace1("1");
			code |= 1;
		} else if (expectzero(remote, done + i)) {
			log_trace1("0");
			code |= 0;
		} else {
			log_trace("failed on bit %d", done + i + 1);
			return (ir_code) -1;
		}
	}
	return code;
}

static ir_code get_pre(struct ir_remote* remote)
{
	ir_code pre;
	ir_code remote_pre;
	ir_code match_pre;
	ir_code toggle_mask;

	pre = get_data(remote, remote->pre_data_bits, 0);

	if (pre == (ir_code) -1) {
		log_trace("Failed on pre_data: cannot get it");
		return (ir_code) -1;
	}
	if (update_mode) {
		/*
		 * toggle_bit_mask is applied to the concatenated
		 * pre_data - data - post_data. We dont check post data, but
		 * adjusts for the length.
		 */
		toggle_mask =
			remote->toggle_bit_mask >> remote->post_data_bits;
		remote_pre = remote->pre_data & ~toggle_mask;
		match_pre = pre & ~toggle_mask;
		if (remote->pre_data != 0 && remote_pre != match_pre) {
			log_trace("Failed on pre_data: bad data: %x", pre);
			return (ir_code) -1;
		}
	}
	if (remote->pre_p > 0 && remote->pre_s > 0) {
		if (!expectpulse(remote, remote->pre_p))
			return (ir_code) -1;
		set_pending_space(remote->pre_s);
	}
	return pre;
}

static ir_code get_post(struct ir_remote* remote)
{
	ir_code post;

	if (remote->post_p > 0 && remote->post_s > 0) {
		if (!expectpulse(remote, remote->post_p))
			return (ir_code) -1;
		set_pending_space(remote->post_s);
	}

	post = get_data(remote, remote->post_data_bits, remote->pre_data_bits + remote->bits);

	if (post == (ir_code) -1) {
		log_trace("failed on post_data");
		return (ir_code) -1;
	}
	return post;
}

int receive_decode(struct ir_remote* remote, struct decode_ctx_t* ctx)
{
	lirc_t sync;
	int header;
	struct timeval current;

	sync = 0;               /* make compiler happy */
	memset(ctx, 0, sizeof(struct decode_ctx_t));
	ctx->code = ctx->pre = ctx->post = 0;
	header = 0;

	if (rec_buffer.at_eof && rec_buffer.wptr - rec_buffer.rptr <= 1) {
		log_debug("Decode: found EOF");
		ctx->code = LIRC_EOF;
		rec_buffer.at_eof = 0;
		return 1;
	}
	if (curr_driver->rec_mode == LIRC_MODE_MODE2 ||
	    curr_driver->rec_mode == LIRC_MODE_PULSE ||
	    curr_driver->rec_mode == LIRC_MODE_RAW) {
		rec_buffer_rewind();
		rec_buffer.is_biphase = is_biphase(remote) ? 1 : 0;

		/* we should get a long space first */
		sync = sync_rec_buffer(remote);
		if (!sync) {
			log_trace("failed on sync");
			return 0;
		}
		log_trace("sync");

		if (has_repeat(remote) && last_remote == remote) {
			if (remote->flags & REPEAT_HEADER && has_header(remote)) {
				if (!get_header(remote)) {
					log_trace("failed on repeat header");
					return 0;
				}
				log_trace("repeat header");
			}
			if (get_repeat(remote)) {
				if (remote->last_code == NULL) {
					log_notice("repeat code without last_code received");
					return 0;
				}

				ctx->pre = remote->pre_data;
				ctx->code = remote->last_code->code;
				ctx->post = remote->post_data;
				ctx->repeat_flag = 1;

				ctx->min_remaining_gap =
					is_const(remote) ? (min_gap(remote) >
							    rec_buffer.sum ? min_gap(remote) -
							    rec_buffer.sum : 0) : (has_repeat_gap(remote) ? remote->
										   repeat_gap : min_gap(remote));
				ctx->max_remaining_gap =
					is_const(remote) ? (max_gap(remote) >
							    rec_buffer.sum ? max_gap(remote) -
							    rec_buffer.sum : 0) : (has_repeat_gap(remote) ? remote->
										   repeat_gap : max_gap(remote));
				return 1;
			}
			log_trace("no repeat");
			rec_buffer_rewind();
			sync_rec_buffer(remote);
		}

		if (has_header(remote)) {
			header = 1;
			if (!get_header(remote)) {
				header = 0;
				if (!(remote->flags & NO_HEAD_REP && expect_at_most(remote, sync, max_gap(remote)))) {
					log_trace("failed on header");
					return 0;
				}
			}
			log_trace("header");
		}
	}

	if (is_raw(remote)) {
		struct ir_ncode* codes;
		struct ir_ncode* found;
		int i;

		if (curr_driver->rec_mode == LIRC_MODE_LIRCCODE)
			return 0;

		codes = remote->codes;
		found = NULL;
		while (codes->name != NULL && found == NULL) {
			found = codes;
			for (i = 0; i < codes->length; ) {
				if (!expectpulse(remote, codes->signals[i++])) {
					found = NULL;
					rec_buffer_rewind();
					sync_rec_buffer(remote);
					break;
				}
				if (i < codes->length && !expectspace(remote, codes->signals[i++])) {
					found = NULL;
					rec_buffer_rewind();
					sync_rec_buffer(remote);
					break;
				}
			}
			codes++;
			if (found != NULL) {
				if (!get_gap
					    (remote, is_const(remote) ?
					    min_gap(remote) - rec_buffer.sum :
					    min_gap(remote)))
					found = NULL;
			}
		}
		if (found == NULL)
			return 0;
		ctx->code = found->code;
	} else {
		if (curr_driver->rec_mode == LIRC_MODE_LIRCCODE) {
			lirc_t sum;
			ir_code decoded = rec_buffer.decoded;

			log_trace("decoded: %llx", decoded);
			if (curr_driver->rec_mode == LIRC_MODE_LIRCCODE
			    && curr_driver->code_length != bit_count(remote))
				return 0;

			ctx->post = decoded & gen_mask(remote->post_data_bits);
			decoded >>= remote->post_data_bits;
			ctx->code = decoded & gen_mask(remote->bits);
			ctx->pre = decoded >> remote->bits;

			gettimeofday(&current, NULL);
			sum = remote->phead + remote->shead +
			      lirc_t_max(remote->pone + remote->sone,
					 remote->pzero + remote->szero) * bit_count(remote) + remote->plead +
			      remote->ptrail + remote->pfoot + remote->sfoot + remote->pre_p + remote->pre_s +
			      remote->post_p + remote->post_s;

			rec_buffer.sum = sum >= remote->gap ? remote->gap - 1 : sum;
			sync = time_elapsed(&remote->last_send, &current) - rec_buffer.sum;
		} else {
			if (!get_lead(remote)) {
				log_trace("failed on leading pulse");
				return 0;
			}

			if (has_pre(remote)) {
				ctx->pre = get_pre(remote);
				if (ctx->pre == (ir_code) -1) {
					log_trace("failed on pre");
					return 0;
				}
				log_trace("pre: %llx", ctx->pre);
			}

			ctx->code = get_data(remote, remote->bits, remote->pre_data_bits);
			if (ctx->code == (ir_code) -1) {
				log_trace("failed on code");
				return 0;
			}
			log_trace("code: %llx", ctx->code);

			if (has_post(remote)) {
				ctx->post = get_post(remote);
				if (ctx->post == (ir_code) -1) {
					log_trace("failed on post");
					return 0;
				}
				log_trace("post: %llx", ctx->post);
			}
			if (!get_trail(remote)) {
				log_trace("failed on trailing pulse");
				return 0;
			}
			if (has_foot(remote)) {
				if (!get_foot(remote)) {
					log_trace("failed on foot");
					return 0;
				}
			}
			if (header == 1 && is_const(remote) && (remote->flags & NO_HEAD_REP))
				rec_buffer.sum -= remote->phead + remote->shead;
			if (is_rcmm(remote)) {
				if (!get_gap(remote, 1000))
					return 0;
			} else if (is_const(remote)) {
				if (!get_gap(remote, min_gap(remote) > rec_buffer.sum ?
					     min_gap(remote) - rec_buffer.sum :
					     0))
					return 0;
			} else {
				if (!get_gap(remote, min_gap(remote)))
					return 0;
			}
		}               /* end of mode specific code */
	}
	if ((!has_repeat(remote) || remote->reps < remote->min_code_repeat)
	    && expect_at_most(remote, sync, remote->max_remaining_gap))
		ctx->repeat_flag = 1;
	else
		ctx->repeat_flag = 0;
	if (curr_driver->rec_mode == LIRC_MODE_LIRCCODE) {
		/* Most TV cards don't pass each signal to the
		 * driver. This heuristic should fix repeat in such
		 * cases. */
		if (time_elapsed(&remote->last_send, &current) < 325000)
			ctx->repeat_flag = 1;
	}
	if (is_const(remote)) {
		ctx->min_remaining_gap = min_gap(remote) > rec_buffer.sum ? min_gap(remote) - rec_buffer.sum : 0;
		ctx->max_remaining_gap = max_gap(remote) > rec_buffer.sum ? max_gap(remote) - rec_buffer.sum : 0;
	} else {
		ctx->min_remaining_gap = min_gap(remote);
		ctx->max_remaining_gap = max_gap(remote);
	}
	return 1;
}
