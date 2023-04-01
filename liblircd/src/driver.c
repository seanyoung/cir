
#include <stdio.h>

#include "driver.h"
#include "receive.h"

static int offset;
static int *data;
static int len;
static int leading;

int set_fake_data(int *d, int l)
{
    data = d;
    len = l;
    offset = 0;
    leading = 1;
}

int fake_data_done() {
    return offset >= len;
}

int readdata(int timeout) {
    if (leading) {
        leading = 0;
        return 100000;
    }
    if (offset < len) {
        int x = data[offset++];
        if (offset & 1) {
            x |= PULSE_BIT;
        }
        return x;
    } else {
        return 0;
    }
}

static struct driver fake_driver = {
    .name = "it's me",
    .device = "liblircd fake driver",
    .rec_mode = LIRC_MODE_MODE2,
    .readdata = readdata,
    .decode_func = receive_decode,
};

const struct driver *const curr_driver = &fake_driver;

