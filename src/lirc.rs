use iocuddle::*;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{Error, ErrorKind, Read, Write};
use std::mem;
use std::ops::Range;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const LIRC: Group = Group::new(b'i');

const LIRC_SET_SEND_CARRIER: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x13) };
const LIRC_SET_SEND_DUTY_CYCLE: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x15) };
const LIRC_SET_TRANSMITTER_MASK: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x17) };
const LIRC_GET_FEATURES: Ioctl<iocuddle::Read, &u32> = unsafe { LIRC.read(0x00) };
const LIRC_GET_REC_TIMEOUT: Ioctl<iocuddle::Read, &u32> = unsafe { LIRC.read(0x24) };
const LIRC_SET_REC_TIMEOUT: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x18) };
const LIRC_GET_MIN_TIMEOUT: Ioctl<iocuddle::Read, &u32> = unsafe { LIRC.read(0x08) };
const LIRC_GET_MAX_TIMEOUT: Ioctl<iocuddle::Read, &u32> = unsafe { LIRC.read(0x09) };
const LIRC_SET_WIDEBAND_RECEIVER: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x23) };
const LIRC_SET_MEASURE_CARRIER_MODE: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x1d) };
const LIRC_SET_REC_MODE: Ioctl<iocuddle::Write, &u32> = unsafe { LIRC.write(0x12) };
const LIRC_GET_REC_RESOLUTION: Ioctl<iocuddle::Read, &u32> = unsafe { LIRC.read(0x07) };

const LIRC_CAN_SET_SEND_CARRIER: u32 = 0x100;
const LIRC_CAN_SET_SEND_DUTY_CYCLE: u32 = 0x200;
const LIRC_CAN_SET_TRANSMITTER_MASK: u32 = 0x400;
const LIRC_CAN_SEND_PULSE: u32 = 2;
const LIRC_CAN_SET_REC_TIMEOUT: u32 = 0x10000000;
const LIRC_CAN_GET_REC_RESOLUTION: u32 = 0x20000000;
const LIRC_CAN_MEASURE_CARRIER: u32 = 0x02000000;
const LIRC_CAN_USE_WIDEBAND_RECEIVER: u32 = 0x04000000;
const LIRC_CAN_REC_MODE2: u32 = 0x00040000;
const LIRC_CAN_REC_SCANCODE: u32 = 0x00080000;

const LIRC_MODE_MODE2: u32 = 0x00000004;
const LIRC_MODE_SCANCODE: u32 = 0x00000008;

const LIRC_MODE2_SPACE: u32 = 0x00000000;
const LIRC_MODE2_PULSE: u32 = 0x01000000;
const LIRC_MODE2_FREQUENCY: u32 = 0x02000000;
const LIRC_MODE2_TIMEOUT: u32 = 0x03000000;

const LIRC_VALUE_MASK: u32 = 0x00FFFFFF;
const LIRC_MODE2_MASK: u32 = 0xFF000000;

///
pub struct Lirc {
    pub file: File,
    features: u32,
    raw_mode: bool,
}

pub const LIRC_SCANCODE_FLAG_TOGGLE: u16 = 1;
pub const LIRC_SCANCODE_FLAG_REPEAT: u16 = 2;

/// Type used for receiving decoded IR.
#[repr(C)]
pub struct LircScancode {
    pub timestamp: u64,
    pub flags: u16,
    pub rc_proto: u16,
    pub keycode: u32,
    pub scancode: u64,
}

/// Type used for receiving raw IR (aka mode2)
pub struct LircRaw(u32);

impl LircRaw {
    pub fn is_pulse(&self) -> bool {
        (self.0 & LIRC_MODE2_MASK) == LIRC_MODE2_PULSE
    }

    pub fn is_space(&self) -> bool {
        (self.0 & LIRC_MODE2_MASK) == LIRC_MODE2_SPACE
    }

    pub fn is_frequency(&self) -> bool {
        (self.0 & LIRC_MODE2_MASK) == LIRC_MODE2_FREQUENCY
    }

    pub fn is_timeout(&self) -> bool {
        (self.0 & LIRC_MODE2_MASK) == LIRC_MODE2_TIMEOUT
    }

    pub fn value(&self) -> u32 {
        self.0 & LIRC_VALUE_MASK
    }
}

/// Open a lirc chardev, which should have a path like "/dev/lirc0"
pub fn lirc_open(path: &Path) -> io::Result<Lirc> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;

    if let Ok((0, features)) = LIRC_GET_FEATURES.ioctl(&file) {
        Ok(Lirc {
            file,
            features,
            raw_mode: true,
        })
    } else {
        Err(Error::new(
            ErrorKind::NotFound,
            String::from("not a lirc device"),
        ))
    }
}

impl Lirc {
    /// Transmit infrared. Each element in the array describes the number of microseconds the IR should be on and off,
    /// respectively.
    pub fn send(&mut self, data: &[u32]) -> io::Result<()> {
        assert!(!data.is_empty());

        if (self.features & LIRC_CAN_SEND_PULSE) != 0 {
            let bs_length = if (data.len() % 2) == 0 {
                // remove trailing space
                (data.len() - 1) * mem::size_of::<u32>()
            } else {
                data.len() * mem::size_of::<u32>()
            };

            // there must be a nicer way to write an array of u32s..
            let data = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, bs_length) };
            let res = self.file.write(data)?;

            if res != bs_length {
                Err(Error::new(
                    ErrorKind::Other,
                    String::from("send incomplete"),
                ))
            } else {
                Ok(())
            }
        } else {
            Err(Error::new(
                ErrorKind::Other,
                String::from("device does not support sending"),
            ))
        }
    }

    /// Does this lirc device support sending
    pub fn can_send(&self) -> bool {
        (self.features & LIRC_CAN_SEND_PULSE) != 0
    }

    /// Does this lirc device support setting send carrier
    pub fn can_set_send_carrier(&self) -> bool {
        (self.features & LIRC_CAN_SET_SEND_CARRIER) != 0
    }

    /// Does this lirc device support setting send duty cycle
    pub fn can_set_send_duty_cycle(&self) -> bool {
        (self.features & LIRC_CAN_SET_SEND_DUTY_CYCLE) != 0
    }

    /// Does this lirc device support setting transmitter mask
    pub fn can_set_send_transmitter_mask(&self) -> bool {
        (self.features & LIRC_CAN_SET_TRANSMITTER_MASK) != 0
    }

    /// Set the send carrier. A carrier of 0 means unmodulated
    pub fn set_send_carrier(&mut self, carrier: u32) -> io::Result<()> {
        // The ioctl should return 0, but on old kernels it may return the new carrier setting; just ignore
        LIRC_SET_SEND_CARRIER.ioctl(&mut self.file, &carrier)?;

        Ok(())
    }

    /// Set the send carrier. A carrier of 0 means unmodulated
    pub fn set_send_duty_cycle(&mut self, duty_cycle: u32) -> io::Result<()> {
        debug_assert!(duty_cycle > 1 && duty_cycle < 100);

        LIRC_SET_SEND_DUTY_CYCLE.ioctl(&mut self.file, &duty_cycle)?;

        Ok(())
    }

    pub fn num_transmitters(&mut self) -> io::Result<u32> {
        // If the LIRC_SET_TRANSMITTER_MASK is called with an invalid mask, the number of transmitters are returned
        LIRC_SET_TRANSMITTER_MASK.ioctl(&mut self.file, &!0)
    }

    /// Set the send carrier. A carrier of 0 means unmodulated
    pub fn set_transmitter_mask(&mut self, transmitter_mask: u32) -> io::Result<()> {
        let res = LIRC_SET_TRANSMITTER_MASK.ioctl(&mut self.file, &transmitter_mask)?;

        if res != 0 {
            Err(Error::new(
                ErrorKind::Other,
                format!("device only supports {} transmitters", res),
            ))
        } else {
            Ok(())
        }
    }

    /// Does this lirc device support setting send carrier
    pub fn can_set_timeout(&self) -> bool {
        (self.features & LIRC_CAN_SET_REC_TIMEOUT) != 0
    }

    /// Set the receiving timeout in microseconds
    pub fn set_timeout(&mut self, timeout: u32) -> io::Result<()> {
        LIRC_SET_REC_TIMEOUT.ioctl(&mut self.file, &timeout)?;

        Ok(())
    }

    /// Get the current receiving timeout in microseconds
    pub fn get_timeout(&self) -> io::Result<u32> {
        let (_, timeout) = LIRC_GET_REC_TIMEOUT.ioctl(&self.file)?;

        Ok(timeout)
    }

    /// Get the minimum and maximum timeout this lirc device supports
    pub fn get_min_max_timeout(&self) -> io::Result<Range<u32>> {
        let (_, min) = LIRC_GET_MIN_TIMEOUT.ioctl(&self.file)?;
        let (_, max) = LIRC_GET_MAX_TIMEOUT.ioctl(&self.file)?;

        Ok(min..max)
    }

    /// Does this lirc device support setting send carrier
    pub fn can_use_wideband_receiver(&self) -> bool {
        (self.features & LIRC_CAN_USE_WIDEBAND_RECEIVER) != 0
    }

    /// Set the receiving timeout in microseconds
    pub fn set_wideband_receiver(&mut self, enable: bool) -> io::Result<()> {
        let enable = if enable { 1 } else { 0 };
        LIRC_SET_WIDEBAND_RECEIVER.ioctl(&mut self.file, &enable)?;

        Ok(())
    }

    /// Does this lirc device support measuring the carrier
    pub fn can_measure_carrier(&self) -> bool {
        (self.features & LIRC_CAN_MEASURE_CARRIER) != 0
    }

    /// Enabling measuring the carrier
    pub fn set_measure_carrier(&mut self, enable: bool) -> io::Result<()> {
        let enable = if enable { 1 } else { 0 };
        LIRC_SET_MEASURE_CARRIER_MODE.ioctl(&mut self.file, &enable)?;

        Ok(())
    }

    /// Does this lirc device support receiving in raw format
    pub fn can_receive_raw(&self) -> bool {
        (self.features & LIRC_CAN_REC_MODE2) != 0
    }

    /// Read the raw IR. If there is nothing to be read, the result vector will be
    /// set to length 0. Otherwise, up to the capacity of result entries will be read.
    pub fn receive_raw(&mut self, result: &mut Vec<LircRaw>) -> io::Result<()> {
        if !self.raw_mode {
            let mode = LIRC_MODE_MODE2;

            LIRC_SET_REC_MODE.ioctl(&mut self.file, &mode)?;

            self.raw_mode = true;
        }

        let length = result.capacity() * mem::size_of::<LircRaw>();
        let data = unsafe { std::slice::from_raw_parts_mut(result.as_ptr() as *mut u8, length) };

        let res = match self.file.read(data) {
            Ok(res) => res,
            Err(err) if err.raw_os_error() == Some(libc::EAGAIN) => 0,
            Err(err) => return Err(err),
        };

        unsafe { result.set_len(res / mem::size_of::<LircRaw>()) };

        Ok(())
    }

    /// Does this lirc device support receiving in decoded scancode format
    pub fn can_receive_scancodes(&self) -> bool {
        (self.features & LIRC_CAN_REC_MODE2 | LIRC_CAN_REC_SCANCODE) != 0
    }

    /// Switch to scancode mode
    pub fn scancode_mode(&mut self) -> io::Result<()> {
        if self.raw_mode {
            let mode = LIRC_MODE_SCANCODE;

            LIRC_SET_REC_MODE.ioctl(&mut self.file, &mode)?;

            self.raw_mode = false;
        }

        Ok(())
    }

    /// Read the decoded IR.
    pub fn receive_scancodes(&mut self, result: &mut Vec<LircScancode>) -> io::Result<()> {
        self.scancode_mode()?;

        let length = result.capacity() * mem::size_of::<LircScancode>();
        let data = unsafe { std::slice::from_raw_parts_mut(result.as_ptr() as *mut u8, length) };

        let res = match self.file.read(data) {
            Ok(res) => res,
            Err(err) if err.raw_os_error() == Some(libc::EAGAIN) => 0,
            Err(err) => return Err(err),
        };

        unsafe {
            result.set_len(res / mem::size_of::<LircScancode>());
        }

        Ok(())
    }

    /// Can we get the receiver resolution
    pub fn can_get_rec_resolution(&self) -> bool {
        (self.features & LIRC_CAN_GET_REC_RESOLUTION) != 0
    }

    /// Enabling measuring the carrier
    pub fn receiver_resolution(&self) -> io::Result<u32> {
        let (_, res) = LIRC_GET_REC_RESOLUTION.ioctl(&self.file)?;

        Ok(res)
    }
}
