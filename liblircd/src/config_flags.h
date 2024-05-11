/****************************************************************************
** config_flags.h ***********************************************************
****************************************************************************
*
*/

/**
* @file config_flags.h
* @brief Flags shared between config_file and dump_config.
*/

#ifndef _CONFIG_FLAGS_H
#define _CONFIG_FLAGS_H

/** Description of flag to print. */
struct flaglist {
	char*	name;                   /**< Name of flag. */
	int	flag;                   /**< Flag bitmask.*/
};

/** All flags i config file: Their name and mask. */
extern const struct flaglist all_flags[];

#endif
