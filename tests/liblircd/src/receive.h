/****************************************************************************
** receive.h ***************************************************************
****************************************************************************
*
* functions that decode IR codes
*
* Copyright (C) 1999 Christoph Bartelmus <lirc@bartelmus.de>
*
*/

/**
 * @file receive.h
 * @author Christoph Bartelmus
 * @brief Functions that decode IR codes.
 * @ingroup driver_api
 */

#ifndef _RECEIVE_H
#define _RECEIVE_H

#include <stdint.h>
#include "ir_remote.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @addtogroup driver_api
 * @{
 */


/** Min value returned by receive_timeout. */
#define MIN_RECEIVE_TIMEOUT 100000

/**
 * Set update mode, where recorded pre_data is verified to match
 * the template pre_data. By defaulöt false.
 */
void rec_set_update_mode(int mode);

/**
 * Set a file logging input from driver in same format as mode2(1).
 * @param f Open file to write on or NULL to disable logging.
 */
void rec_buffer_set_logfile(FILE* f);

/** Return actual timeout to use given MIN_RECEIVE_TIMEOUT limitation. */
static inline lirc_t receive_timeout(lirc_t usec)
{
	return 2 * usec < MIN_RECEIVE_TIMEOUT ? MIN_RECEIVE_TIMEOUT : 2 * usec;
}

/**
 * If set_waitfordata(func) is called, invoke and return function set this
 * way. Otherwise wait until data is available in drv.fd, timeout or a
 * signal is raised.
 *
 * @param maxusec timeout in micro seconds, given to poll(2). If <= 0, the
 *       function will block indefinitely until data is available or a
 *       signal is processed. If positive, a timeout value in microseconds.
 * @return True (1) if there is data available in drv.fd, else 0 indicating
 *       timeout.
 */
int waitfordata(uint32_t maxusec);

/** Set the function used by waitfordata().  */
void set_waitfordata_func(int (*func)(uint32_t maxusec));


/** Clear internal buffer to pristine state. */
void rec_buffer_init(void);

/**
 * Flush the internal fifo and store a single code read
 * from the driver in it.
 */
int rec_buffer_clear(void);

/**
 * Decode data from remote
 *
 * @param ctx Undefined on enter. On exit, the fields in the
 *     structure are defined.
 */
int receive_decode(struct ir_remote* remote, struct decode_ctx_t* ctx);

/**
 * Reset the modules's internal fifo's read state to initial values
 * where the nothing is read. The write pointer is not affected.
 */
void rec_buffer_rewind(void);

/** Reset internal fifo's write pointer.  */
void rec_buffer_reset_wptr(void);


/** @} */
#ifdef __cplusplus
}
#endif

#endif
