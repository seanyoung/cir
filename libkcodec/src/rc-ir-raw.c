// SPDX-License-Identifier: GPL-2.0
// rc-ir-raw.c - handle IR pulse/space events
//
// Copyright (C) 2010 by Mauro Carvalho Chehab

// #include <linux/export.h>
// #include <linux/kthread.h>
// #include <linux/mutex.h>
// #include <linux/kmod.h>
// #include <linux/sched.h>
#include "rc-core-priv.h"


/**
 * ir_raw_gen_manchester() - Encode data with Manchester (bi-phase) modulation.
 * @ev:		Pointer to pointer to next free event. *@ev is incremented for
 *		each raw event filled.
 * @max:	Maximum number of raw events to fill.
 * @timings:	Manchester modulation timings.
 * @n:		Number of bits of data.
 * @data:	Data bits to encode.
 *
 * Encodes the @n least significant bits of @data using Manchester (bi-phase)
 * modulation with the timing characteristics described by @timings, writing up
 * to @max raw IR events using the *@ev pointer.
 *
 * Returns:	0 on success.
 *		-ENOBUFS if there isn't enough space in the array to fit the
 *		full encoded data. In this case all @max events will have been
 *		written.
 */
int ir_raw_gen_manchester(struct ir_raw_event **ev, unsigned int max,
			  const struct ir_raw_timings_manchester *timings,
			  unsigned int n, u64 data)
{
	bool need_pulse;
	u64 i;
	int ret = -ENOBUFS;

	i = BIT_ULL(n - 1);

	if (timings->leader_pulse) {
		if (!max--)
			return ret;
		init_ir_raw_event_duration((*ev), 1, timings->leader_pulse);
		if (timings->leader_space) {
			if (!max--)
				return ret;
			init_ir_raw_event_duration(++(*ev), 0,
						   timings->leader_space);
		}
	} else {
		/* continue existing signal */
		--(*ev);
	}
	/* from here on *ev will point to the last event rather than the next */

	while (n && i > 0) {
		need_pulse = !(data & i);
		if (timings->invert)
			need_pulse = !need_pulse;
		if (need_pulse == !!(*ev)->pulse) {
			(*ev)->duration += timings->clock;
		} else {
			if (!max--)
				goto nobufs;
			init_ir_raw_event_duration(++(*ev), need_pulse,
						   timings->clock);
		}

		if (!max--)
			goto nobufs;
		init_ir_raw_event_duration(++(*ev), !need_pulse,
					   timings->clock);
		i >>= 1;
	}

	if (timings->trailer_space) {
		if (!(*ev)->pulse)
			(*ev)->duration += timings->trailer_space;
		else if (!max--)
			goto nobufs;
		else
			init_ir_raw_event_duration(++(*ev), 0,
						   timings->trailer_space);
	}

	ret = 0;
nobufs:
	/* point to the next event rather than last event before returning */
	++(*ev);
	return ret;
}

/**
 * ir_raw_gen_pd() - Encode data to raw events with pulse-distance modulation.
 * @ev:		Pointer to pointer to next free event. *@ev is incremented for
 *		each raw event filled.
 * @max:	Maximum number of raw events to fill.
 * @timings:	Pulse distance modulation timings.
 * @n:		Number of bits of data.
 * @data:	Data bits to encode.
 *
 * Encodes the @n least significant bits of @data using pulse-distance
 * modulation with the timing characteristics described by @timings, writing up
 * to @max raw IR events using the *@ev pointer.
 *
 * Returns:	0 on success.
 *		-ENOBUFS if there isn't enough space in the array to fit the
 *		full encoded data. In this case all @max events will have been
 *		written.
 */
int ir_raw_gen_pd(struct ir_raw_event **ev, unsigned int max,
		  const struct ir_raw_timings_pd *timings,
		  unsigned int n, u64 data)
{
	int i;
	int ret;
	unsigned int space;

	if (timings->header_pulse) {
		ret = ir_raw_gen_pulse_space(ev, &max, timings->header_pulse,
					     timings->header_space);
		if (ret)
			return ret;
	}

	if (timings->msb_first) {
		for (i = n - 1; i >= 0; --i) {
			space = timings->bit_space[(data >> i) & 1];
			ret = ir_raw_gen_pulse_space(ev, &max,
						     timings->bit_pulse,
						     space);
			if (ret)
				return ret;
		}
	} else {
		for (i = 0; i < n; ++i, data >>= 1) {
			space = timings->bit_space[data & 1];
			ret = ir_raw_gen_pulse_space(ev, &max,
						     timings->bit_pulse,
						     space);
			if (ret)
				return ret;
		}
	}

	ret = ir_raw_gen_pulse_space(ev, &max, timings->trailer_pulse,
				     timings->trailer_space);
	return ret;
}

/**
 * ir_raw_gen_pl() - Encode data to raw events with pulse-length modulation.
 * @ev:		Pointer to pointer to next free event. *@ev is incremented for
 *		each raw event filled.
 * @max:	Maximum number of raw events to fill.
 * @timings:	Pulse distance modulation timings.
 * @n:		Number of bits of data.
 * @data:	Data bits to encode.
 *
 * Encodes the @n least significant bits of @data using space-distance
 * modulation with the timing characteristics described by @timings, writing up
 * to @max raw IR events using the *@ev pointer.
 *
 * Returns:	0 on success.
 *		-ENOBUFS if there isn't enough space in the array to fit the
 *		full encoded data. In this case all @max events will have been
 *		written.
 */
int ir_raw_gen_pl(struct ir_raw_event **ev, unsigned int max,
		  const struct ir_raw_timings_pl *timings,
		  unsigned int n, u64 data)
{
	int i;
	int ret = -ENOBUFS;
	unsigned int pulse;

	if (!max--)
		return ret;

	init_ir_raw_event_duration((*ev)++, 1, timings->header_pulse);

	if (timings->msb_first) {
		for (i = n - 1; i >= 0; --i) {
			if (!max--)
				return ret;
			init_ir_raw_event_duration((*ev)++, 0,
						   timings->bit_space);
			if (!max--)
				return ret;
			pulse = timings->bit_pulse[(data >> i) & 1];
			init_ir_raw_event_duration((*ev)++, 1, pulse);
		}
	} else {
		for (i = 0; i < n; ++i, data >>= 1) {
			if (!max--)
				return ret;
			init_ir_raw_event_duration((*ev)++, 0,
						   timings->bit_space);
			if (!max--)
				return ret;
			pulse = timings->bit_pulse[data & 1];
			init_ir_raw_event_duration((*ev)++, 1, pulse);
		}
	}

	if (!max--)
		return ret;

	init_ir_raw_event_duration((*ev)++, 0, timings->trailer_space);

	return 0;
}
