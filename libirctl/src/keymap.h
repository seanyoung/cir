/* SPDX-License-Identifier: GPL-2.0 */
#ifndef __KEYMAP_H
#define __KEYMAP_H

#include <stdint.h>

struct keymap {
	struct keymap *next;
	char *name;
	char *protocol;
	char *variant;
	struct protocol_param *param;
	struct scancode_entry *scancode;
	struct raw_entry *raw;
};

struct protocol_param {
	struct protocol_param *next;
	char *name;
	long int value;
};

struct scancode_entry {
	struct scancode_entry *next;
	uint64_t scancode;
	char *keycode;
};

struct raw_entry {
	struct raw_entry *next;
	uint64_t scancode;
	uint32_t raw_length;
	char *keycode;
	uint32_t raw[1];
};

void free_keymap(struct keymap *map);
int parse_keymap(char *fname, struct keymap **keymap, bool verbose);
int keymap_param(struct keymap *map, const char *name, int fallback);

#endif
