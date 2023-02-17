/****************************************************************************
** driver.h **************************************************************
****************************************************************************
*
* Copyright (C) 1999 Christoph Bartelmus <lirc@bartelmus.de>
*
*/

/**
 * @file driver.h
 * @brief Interface to the userspace drivers.
 * @ingroup driver_api
 * @ingroup private_api
 */

/** @addtogroup driver_api
 *  @brief  User-space driver API.
 *  @{
 */
#ifndef _HARDWARE_H
#define _HARDWARE_H

#include <glob.h>
#include <stdint.h>

#include "lirc.h"

#include "ir_remote_types.h"

#ifndef MAXPATHLEN
#define MAXPATHLEN 4096
#endif

/** Testable flag for get_server_version() presence. */
#define HAVE_SERVER_VERSION 1

/** Return numeric server version, m.v.r => 10000 * m + 100 * v + r. */
int get_server_version(void);


#ifdef __cplusplus
extern "C" {
#endif

/** drvctl definitions */
#define DRV_ERR_NOT_IMPLEMENTED         1

/** Stores path in drv.device if non-null. */
int default_open(const char* path);

/** For now, a placeholder. */
int default_close(void);

/** Return DRV_ERR_NOTIMPLEMENTED. */
int default_drvctl(unsigned int cmd, void* arg);

/** Argument for DRV_SET_OPTION. */
struct option_t {
	char	key[32];
	char	value[64];
};

/**
 * Parse an option string "key:value;key:value..." and invoke
 * drvctl DRV_SET_OPTION as appropriate.
 */
int drv_handle_options(const char* options);


/** Drvctl cmd:  return current state as an int in *arg. */
#define DRVCTL_GET_STATE                1

/** Drvctl cmd:  Send long space. Arg is pulselength (us, an int). */
#define DRVCTL_SEND_SPACE               2

/** Drvctl cmd: Set driver options. Arg is   *struct option_t. */
#define DRVCTL_SET_OPTION               3

/**
* Drvctl cmd: get raw length to read, if different than codelength.
* Arg is an unsigned int* which is updated on successfull return.
*/
#define DRVCTL_GET_RAW_CODELENGTH       4

/**
* Drvctl cmd: get list of possible devices. Argument is a *glob_t as
* defined in <glob.h>.  The returned memory is owned by driver and
* should be free()'d using DRVCTL_FREE_DEVICES.
*
* Each string in glob is a space-separated list of words. The first
* word is the mandatory device path, the optional reminder is
* information about the device suitable in user interfaces.
*/
#define DRVCTL_GET_DEVICES              5

/** drvctl cmd: Free memory in argument obtained using DRVCTL_GET_DEVICES. */
#define DRVCTL_FREE_DEVICES             6

/**
 * The former LIRC_NOTIFY_DECODE, informs drier that signal is successfully
 * decoded e. g., to initiate some visual feedback through a LED.
 */

#define DRVCTL_NOTIFY_DECODE            7

/** Last well-known command. Remaining is used in driver-specific controls.*/
#define  DRVCTL_MAX                     128

/** drvctl error. */
#define  DRV_ERR_NOT_IMPLEMENTED        1

/** drvctl error: cmd and arg is OK, but other errors. */
#define  DRV_ERR_BAD_STATE              2

/** drvctl error: cmd is bad */
#define  DRV_ERR_BAD_OPTION		3

/** drvctl error: arg is bad */
#define  DRV_ERR_BAD_VALUE		4

/** No requested data available. */
#define  DRV_ERR_ENUM_EMPTY		5

/** drvctl error: "Should not happen" type of errors.  */
#define  DRV_ERR_INTERNAL		6

/**
 * The data the driver exports i. e., lirc accesses the driver as
 * defined here.
 */
struct driver {
// Old-style implicit API version 1:

	/**
	 * Name of the device (string). Set by open_func() before init(),
	 * possibly using the hard-coded driver default value.
	 */
	const char* device;

	/** Set by the driver after init(). */
	int		fd;

	/** Code for the features of the present device, valid after init(). */
	uint32_t	features;

	/**
	 * Possible values are: LIRC_MODE_RAW, LIRC_MODE_PULSE, LIRC_MODE_MODE2,
	 * LIRC_MODE_LIRCCODE. These can be combined using bitwise or.
	 */
	uint32_t	send_mode;

	/**
	 * Possible values are: LIRC_MODE_RAW, LIRC_MODE_PULSE, LIRC_MODE_MODE2,
	 * LIRC_MODE_LIRCCODE. These can be combined using bitwise or.
	 */
	uint32_t	rec_mode;

	/** Length in bits of the code. */
	const uint32_t	code_length;

	 /**
	 *  Function called to do basic driver setup.
	 *  @param device String describing what device driver should
	 *      communicate with. Often (but not always) a /dev/... path.
	 *  @return 0 if everything is fine, else positive error code.
	 */
	int (*const open_func) (const char* device);

	/**
	 * Function called for initializing the driver and the hardware.
	 * Zero return value indicates failure, all other return values success.
	 */
	int (*const init_func)(void);

	/**
	 * Function called when transmitting/receiving stops. Zero return value
	 *  indicates failure, all other return values success.
	 */
	int (*const deinit_func) (void);

	/**
	 * Send data to the remote.
	 * @param remote The remote used to send.
	 * @param code Code(s) to send, a single code or the head of a
	 *             list of codes.
	 */
	int (*const send_func)(struct ir_remote* remote,
			       struct ir_ncode* code);

	/**
	 * Receive data from remote. Might close device on error conditions.
	 * @param The remote to read from.
	 * @return Formatted, statically allocated string with decoded
	 *         data: "remote-name code-name code repetitions"
	 */
	char* (*const rec_func)(struct ir_remote* remotes);

	/**
	 * TODO
	 */
	int (*const decode_func)(struct ir_remote* remote,
				 struct decode_ctx_t* ctx);

	/**
	 * Generic driver control function with semantics as defined by driver
	 * Returns 0 on success, else a positive error code.
	 */
	int (*const drvctl_func)(unsigned int cmd, void* arg);

	/**
	 * Get length of next pulse/space from hardware.
	 * @param timeout Max time to wait (us).
	 * @return Length of pulse in lower 24 bits (us). PULSE_BIT
	 * is set to reflect if this is a pulse or space. 0
	 * indicates errors.
	 */
	lirc_t (*const readdata)(lirc_t timeout);

	/**
	 * Driver name, as listed by -H help and used as argument to i
	 * --driver.
	 */
	const char*	name;

	/**
	 * The resolution in microseconds of the recorded durations when
	 * reading signals.
	 */
	unsigned int	resolution;

/* API version 2 addons: */

	const int	api_version;            /**< API version (from version 2+).*/
	const char*	driver_version;         /**< Driver version (free text). */
	const char*	info;                   /**< Free text driver info. */

	int (*const close_func)(void);          /**< Hard closing, returns 0 on OK.*/

/* API version 3 addons: */
	/**
	 *  device_hint is a mean for config tools to autodetect devices.
	 *    - /dev/tty*     User selects a tty.
	 *    - drvctl        Driver supports DRVCTL_GET_DEVICES drvctl.
	 *    - auto          No device configured, a message is displayed.
	 *    - /dev/foo\*    A wildcard listing possible devices, general
	 *                    fallback.
	 *
	 *   The following hints are not longer supported:
	 *    - /dev/event\*  A devinput device
	 *    - /dev/usb/\*   A serial, USB-connected port.
	 *    - /bin/sh ...   Shell command listing possible devices.
	 *    - None          No device is silently configured.
	 */
	const char* const  device_hint;
};

/** @} */

#ifdef IN_DRIVER
/** Complete r/w access to drv for driver code including lirc_driver.h. */
extern struct driver drv;
#endif

/** Read-only access to drv for application.*/
extern const struct driver* const curr_driver;


#ifdef __cplusplus
}
#endif

#endif
