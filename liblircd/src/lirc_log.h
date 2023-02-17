/****************************************************************************
** lirc_log.h **************************************************************
****************************************************************************
*
*/

/**
 * @file lirc_log.h
 * @brief Logging functionality.
 * @ingroup private_api
 * @ingroup driver_api
 */


#ifndef _LIRC_LOG_H
#define _LIRC_LOG_H

#include <syslog.h>
#include <sys/time.h>
#include <stdio.h>
#include <unistd.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @addtogroup driver_api
 */


/**
 * The defined loglevels. LIRC_TRACE..LIRC_TRACE2 is mapped to LIRC_DEBUG in
 * outputted messages, but generates more messages than DEBUG.
 */
typedef enum {
	LIRC_TRACE2 = 10,
	LIRC_TRACE1 = 9,
	LIRC_TRACE = 8,
	LIRC_DEBUG = LOG_DEBUG,
	LIRC_INFO = LOG_INFO,
	LIRC_NOTICE = LOG_NOTICE,
	LIRC_WARNING = LOG_WARNING,
	LIRC_ERROR = LOG_ERR,
	LIRC_NOLOG = 0,
	LIRC_BADLEVEL = -1
} loglevel_t;

/**
 * Log channels used to filter messages.
 */

typedef enum {
	LOG_DRIVER = 1,
	LOG_LIB = 4,
	LOG_APP = 8,
	LOG_ALL = 255
} logchannel_t;

/** Max loglevel (for validation). */
#define LIRC_MAX_LOGLEVEL LIRC_TRACE2

/** Mix loglevel (for validation). */
#define LIRC_MIN_LOGLEVEL LIRC_ERROR

/** Adds printf-style arguments to perror(3). */
void perrorf(const char* format, ...);

/** The actual loglevel. Should not be changed directly by external code.*/
extern loglevel_t loglevel;

/** The actual logchannel. Should not be changed directly by external code.*/
extern logchannel_t logged_channels;

/* Set by lirc_log_open, convenience copy for clients. */
extern char progname[128];

/** Default loglevel (last resort). */
#define DEFAULT_LOGLEVEL LIRC_INFO

/** Max level logged in actual logfile. */
#ifdef __cplusplus
#define logmax(l) (l > LIRC_DEBUG ? LIRC_DEBUG : static_cast <loglevel_t>(l))
#else
#define logmax(l) (l > LIRC_DEBUG ? LIRC_DEBUG : l)
#endif

/** perror wrapper logging with level LIRC_ERROR. */
#define log_perror_err(fmt, ...) \
	{ if ((logchannel & logged_channels) && LIRC_ERROR <= loglevel) \
		{ logperror(LIRC_ERROR, fmt, ##__VA_ARGS__); } }

/** perror wrapper logging with level LIRC_WARNING. */
#define log_perror_warn(fmt, ...) \
	{ if ((logchannel & logged_channels) && LIRC_WARNING <= loglevel) \
		{ logperror(LIRC_WARNING, fmt, ##__VA_ARGS__); } }

/** perror wrapper logging with level LIRC_DEBUG. */
#define log_perror_debug(fmt, ...) \
	{ if ((logchannel & logged_channels) && LIRC_DEBUG <= loglevel) \
		{ logperror(LIRC_WARNING, fmt, ##__VA_ARGS__); } }

/** Log an error message. */
#define log_error(fmt, ...) \
	{ if ((logchannel & logged_channels) && LIRC_ERROR <= loglevel) \
		{ logprintf(LIRC_ERROR, fmt, ##__VA_ARGS__); } }

/** Log a warning message. */
#define log_warn(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_WARNING <= loglevel) \
		{ logprintf(LIRC_WARNING, fmt, ##__VA_ARGS__); } }

/** Log an info message. */
#define log_info(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_INFO <= loglevel) \
		{ logprintf(LIRC_INFO, fmt, ##__VA_ARGS__); } }

/** Log a notice message. */
#define log_notice(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_NOTICE <= loglevel) \
		{ logprintf(LIRC_NOTICE, fmt, ##__VA_ARGS__); } }

/** Log a debug message. */
#define log_debug(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_DEBUG <= loglevel) \
		{ logprintf(LIRC_DEBUG, fmt, ##__VA_ARGS__); } }

/** Log a trace message. */
#define log_trace(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_TRACE <= loglevel) \
		{ logprintf(LIRC_TRACE, fmt, ##__VA_ARGS__); } }

/** Log a trace1 message. */
#define log_trace1(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_TRACE1 <= loglevel) \
		{ logprintf(LIRC_TRACE1, fmt, ##__VA_ARGS__); } }

/** Log a trace2 message. */
#define log_trace2(fmt, ...)  \
	{ if ((logchannel & logged_channels) && LIRC_TRACE2 <= loglevel) \
		{ logprintf(LIRC_TRACE2, fmt, ##__VA_ARGS__); } }


/**
 * Convert a string, either a number or 'info', 'trace1', error etc.
 * to a loglevel.
 */
loglevel_t string2loglevel(const char* level);

/** Set the level. Returns 1 if ok, 0 on errors. */
int lirc_log_setlevel(loglevel_t level);

/** Get the default level, from environment or hardcoded. */
loglevel_t lirc_log_defaultlevel(void);

/** Check if a given, standard loglevel should be printed.  */
#define lirc_log_is_enabled_for(level) (level <= loglevel)

/** Check if log is set up to use syslog or not. */
int lirc_log_use_syslog(void);

/**
 * Write a message to the log.
 * Caller should use the log_ macros and not call this directly.
 *
 * @param prio Level of message
 * @param format_str,... printf-style string.
 */
void logprintf(loglevel_t prio, const char* format_str, ...);

/** Log current kernel error with a given level. */
void logperror(loglevel_t prio, const char* format, ...);
int lirc_log_reopen(void);

/**
 * Open the log for upcoming logging
 *
 * @param progname Name of application, made available in global progname
 * @param nodaemon If true, program runs in foreground and logging is on also
 *     on stdout.
 * @param level The lowest level of messages to actually be logged.
 * @return 0 if OK, else positive error code.
 */
int lirc_log_open(const char* progname, int _nodaemon, loglevel_t level);

/** Close the log previosly opened with lirc_log_open(). */
int lirc_log_close(void);

/**
 * Set logfile. Either a regular path or the string 'syslog'; the latter
 * does indeed use syslog(1) instead. Must be called before lirc_log_open().
 */
void lirc_log_set_file(const char* s);

/**
 * Retrieve a client path for logging according to freedesktop specs.
 *
 * @param basename  Basename for the logfile.
 * @param buff Buffer to store result in.
 * @param size Size of buffer
 * @return 0 if OK, otherwise -1
 */
int lirc_log_get_clientlog(const char* basename, char* buffer, ssize_t size);

/** Print prefix + a hex dump of len bytes starting at  *buf. */
void hexdump(char* prefix, unsigned char* buf, int len);

/** Helper macro for STR().*/
#define STRINGIFY(x) #x

/** Return x in (double) quotes. */
#define STR(x) STRINGIFY(x)

/** Wrapper for write(2) which logs errors. */
#define chk_write(fd, buf, count) \
	do_chk_write(fd, buf, count, STR(__FILE__) ":" STR(__LINE__))


/** Wrapper for read(2) which logs errors. */
#define chk_read(fd, buf, count) \
	do_chk_read(fd, buf, count, STR(__FILE__) ":" STR(__LINE__))


/** Implement the chk_write() macro. */
static inline void
do_chk_write(int fd, const void* buf, size_t count, const char* msg)
{
	if (write(fd, buf, count) == -1)
		logperror(LIRC_WARNING, msg);
}


/** Implement the chk_read() macro. */
static inline void
do_chk_read(int fd, void* buf, size_t count, const char* msg)
{
	if (read(fd, buf, count) == -1)
		logperror(LIRC_WARNING, msg);
}



/** @} */

#ifdef __cplusplus
}
#endif

#endif /* _LIRC_LOG_H */
