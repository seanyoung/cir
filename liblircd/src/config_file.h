/****************************************************************************
** config_file.h ***********************************************************
****************************************************************************
*
* Copyright (C) 1998 Pablo d'Angelo (pablo@ag-trek.allgaeu.org)
*
*/

/**
 * @file config_file.h
 * @brief  Parses the lircd.conf config file.
 * @author Pablo d'Angelo
 */

/**
 * @addtogroup private_api
 * @brief Internal API for lirc applications.
 * @{
 */

#ifndef _CONFIG_FILE_H
#define  _CONFIG_FILE_H

#ifdef __cplusplus
extern "C" {
#endif

#include "ir_remote.h"

/**
 * Parse a lircd.conf config file.
 *
 * @param f Open FILE* connection to file.
 * @param name Normally the path for the open file f.
 * @return Pointer to dynamically allocated ir_remote or NULL on errors,
 */
struct ir_remote* read_config(FILE* f, const char* name);

/** Free() an ir_remote instance obtained using read_config(). */
void free_config(struct ir_remote* remotes);

/** @} */

#ifdef __cplusplus
}
#endif

#endif
