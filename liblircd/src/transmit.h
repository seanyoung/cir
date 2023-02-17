/****************************************************************************
** transmit.h **************************************************************
****************************************************************************/
/**
 * @file transmit.h
 * @brief Functions that prepare IR codes for transmitting
 * @author Christoph Bartelmus
 * @ingroup driver_api
 *
 * Operations in this module applies to the transmit buffer. The buffer
 * is initiated using send_buffer_init(), filled with data using send_buffer_put()
 * and accessed using  send_buffer_data() and send_buffer_length().
 *
 * A prepared buffer contains an even number of unsigned ints, each of
 * which representing a pulse width in microseconds. The first item represents
 * a pulse and the last thus a space.
 *
 * Copyright (C) 1999 Christoph Bartelmus <lirc@bartelmus.de>
 *
 * @addtogroup driver_api
 * @{
 */



#ifndef _TRANSMIT_H
#define _TRANSMIT_H

#include "ir_remote.h"

#ifdef __cplusplus
extern "C" {
#endif

#define WBUF_SIZE 256

/** Clear and re-initiate the buffer. */
void send_buffer_init(void);

/**
 * Initializes the global send buffer for transmitting the code in
 * the second argument, residing in the remote in the first.
 * @param remote ir_remote containing code to send.
 * @param code ir_ncode to send.
 * @return 0 on failures, else 1.
 */
int send_buffer_put(struct ir_remote* remote, struct ir_ncode* code);

/** @cond */
int init_sim(struct ir_remote*	remote,
	     struct ir_ncode*	code,
	     int		repeat_preset);
/** @endcond */

/** @return Number of items accessible in array send_buffer_data(). */
int send_buffer_length(void);

/** @return Pointer to timing data in microseconds for pulses/spaces. */
const lirc_t* send_buffer_data(void);

/** @return Total length of send buffer in microseconds. */
lirc_t send_buffer_sum(void);

/** @} */

#ifdef __cplusplus
}
#endif

#endif
