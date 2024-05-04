//! Interface to lirc chardevs on Linux

use aya::programs::{Link, LircMode2, ProgramError};
use nix::{ioctl_read, ioctl_write_ptr};
use num_integer::Integer;
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{self, Error, ErrorKind, Read, Write},
    mem,
    ops::Range,
    os::{
        fd::BorrowedFd,
        unix::io::{AsFd, AsRawFd, RawFd},
    },
    path::{Path, PathBuf},
    thread::sleep,
    time::{Duration, Instant},
};

const LIRC_MAGIC: u8 = b'i';

const LIRC_SET_SEND_CARRIER: u8 = 0x13;
const LIRC_SET_SEND_DUTY_CYCLE: u8 = 0x15;
const LIRC_SET_TRANSMITTER_MASK: u8 = 0x17;
const LIRC_GET_FEATURES: u8 = 0x00;
const LIRC_GET_REC_TIMEOUT: u8 = 0x24;
const LIRC_SET_REC_TIMEOUT: u8 = 0x18;
const LIRC_GET_MIN_TIMEOUT: u8 = 0x08;
const LIRC_GET_MAX_TIMEOUT: u8 = 0x09;
const LIRC_SET_WIDEBAND_RECEIVER: u8 = 0x23;
const LIRC_SET_MEASURE_CARRIER_MODE: u8 = 0x1d;
const LIRC_SET_REC_MODE: u8 = 0x12;
const LIRC_GET_REC_RESOLUTION: u8 = 0x07;

ioctl_read!(lirc_get_features, LIRC_MAGIC, LIRC_GET_FEATURES, u32);
ioctl_read!(lirc_get_rec_timeout, LIRC_MAGIC, LIRC_GET_REC_TIMEOUT, u32);
ioctl_read!(lirc_get_min_timeout, LIRC_MAGIC, LIRC_GET_MIN_TIMEOUT, u32);
ioctl_read!(lirc_get_max_timeout, LIRC_MAGIC, LIRC_GET_MAX_TIMEOUT, u32);
ioctl_read!(
    lirc_get_rec_resolution,
    LIRC_MAGIC,
    LIRC_GET_REC_RESOLUTION,
    u32
);
ioctl_write_ptr!(
    lirc_set_send_carrier,
    LIRC_MAGIC,
    LIRC_SET_SEND_CARRIER,
    u32
);
ioctl_write_ptr!(
    lirc_set_send_duty_cycle,
    LIRC_MAGIC,
    LIRC_SET_SEND_DUTY_CYCLE,
    u32
);
ioctl_write_ptr!(
    lirc_set_transmitter_mask,
    LIRC_MAGIC,
    LIRC_SET_TRANSMITTER_MASK,
    u32
);
ioctl_write_ptr!(
    lirc_set_wideband_receiver,
    LIRC_MAGIC,
    LIRC_SET_WIDEBAND_RECEIVER,
    u32
);
ioctl_write_ptr!(
    lirc_set_measure_carrier,
    LIRC_MAGIC,
    LIRC_SET_MEASURE_CARRIER_MODE,
    u32
);
ioctl_write_ptr!(lirc_set_rec_timeout, LIRC_MAGIC, LIRC_SET_REC_TIMEOUT, u32);
ioctl_write_ptr!(lirc_set_rec_mode, LIRC_MAGIC, LIRC_SET_REC_MODE, u32);

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
const LIRC_MODE2_OVERFLOW: u32 = 0x04000000;

const LIRC_VALUE_MASK: u32 = 0x00FFFFFF;
const LIRC_MODE2_MASK: u32 = 0xFF000000;

/// A physical or virtual lirc device
pub struct Lirc {
    path: PathBuf,
    file: File,
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

    pub fn is_overflow(&self) -> bool {
        (self.0 & LIRC_MODE2_MASK) == LIRC_MODE2_OVERFLOW
    }

    pub fn value(&self) -> u32 {
        self.0 & LIRC_VALUE_MASK
    }
}

/// Open a lirc chardev, which should have a path like "/dev/lirc0"
pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Lirc> {
    lirc_open(path.as_ref())
}

fn lirc_open(path: &Path) -> io::Result<Lirc> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut features = 0u32;

    if let Ok(0) = unsafe { lirc_get_features(file.as_raw_fd(), &mut features) } {
        Ok(Lirc {
            path: PathBuf::from(path),
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
            match self.write(data) {
                Err(err) if err.kind() == io::ErrorKind::InvalidInput => {
                    // The hardware may not be capable of sending long IR, so split it into smaller chunks.
                    let mut data = data;

                    while !data.is_empty() {
                        // find a space of at least 20 microseconds
                        let chunk = if let Some(pos) = data
                            .iter()
                            .enumerate()
                            .position(|(i, val)| i.is_odd() && *val > 20000)
                        {
                            &data[..pos + 1]
                        } else {
                            data
                        };

                        let start = Instant::now();

                        // this duraction will include the trailing space
                        let chunk_duration =
                            Duration::from_micros(chunk.iter().sum::<u32>() as u64);

                        // The write syscall on a lirc chardev waits until the IR is transmitted. There might
                        // be some time to set up this transmission, so we measure the time it takes to do
                        // the entire transmission and sleep if the transmission is faster (we remove trailing
                        // space during send)
                        self.write(chunk)?;

                        let elapsed = start.elapsed();

                        if elapsed < chunk_duration {
                            sleep(chunk_duration - elapsed);
                        }

                        data = &data[chunk.len()..];
                    }
                    Ok(())
                }
                res => res,
            }
        } else {
            Err(Error::new(
                ErrorKind::Unsupported,
                String::from("device does not support sending"),
            ))
        }
    }

    /// transmit some data
    fn write(&mut self, data: &[u32]) -> io::Result<()> {
        assert!(!data.is_empty());

        let data = if (data.len() % 2) == 0 {
            // remove trailing space
            &data[..data.len() - 1]
        } else {
            data
        };

        let data = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
        };

        let res = self.file.write(data)?;

        // linux will either send all of it, or return an error. If we're getting a
        // different result, throw toys out of the pram.
        assert_eq!(
            res,
            std::mem::size_of_val(data),
            "linux driver incomplete send, please send report bug to linux-media@vger.kernel.org"
        );

        Ok(())
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
        unsafe { lirc_set_send_carrier(self.file.as_raw_fd(), &carrier)? };

        Ok(())
    }

    /// Set the send carrier. A carrier of 0 means unmodulated
    pub fn set_send_duty_cycle(&mut self, duty_cycle: u32) -> io::Result<()> {
        debug_assert!(duty_cycle > 1 && duty_cycle < 100);

        unsafe { lirc_set_send_duty_cycle(self.file.as_raw_fd(), &duty_cycle)? };

        Ok(())
    }

    pub fn num_transmitters(&mut self) -> io::Result<u32> {
        // If the LIRC_SET_TRANSMITTER_MASK is called with an invalid mask, the number of transmitters are returned
        let count = unsafe { lirc_set_transmitter_mask(self.file.as_raw_fd(), &!0)? };

        Ok(count.try_into().unwrap())
    }

    /// Set the send carrier. A carrier of 0 means unmodulated
    pub fn set_transmitter_mask(&mut self, transmitter_mask: u32) -> io::Result<()> {
        let res = unsafe { lirc_set_transmitter_mask(self.file.as_raw_fd(), &transmitter_mask)? };

        if res != 0 {
            Err(Error::new(
                ErrorKind::Unsupported,
                format!("device only supports {res} transmitters"),
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
        unsafe { lirc_set_rec_timeout(self.file.as_raw_fd(), &timeout)? };

        Ok(())
    }

    /// Get the current receiving timeout in microseconds
    pub fn get_timeout(&self) -> io::Result<u32> {
        let mut timeout = 0u32;

        unsafe { lirc_get_rec_timeout(self.file.as_raw_fd(), &mut timeout)? };

        Ok(timeout)
    }

    /// Get the minimum and maximum timeout this lirc device supports
    pub fn get_min_max_timeout(&self) -> io::Result<Range<u32>> {
        let mut min = 0u32;
        let mut max = 0u32;
        unsafe { lirc_get_min_timeout(self.file.as_raw_fd(), &mut min)? };
        unsafe { lirc_get_max_timeout(self.file.as_raw_fd(), &mut max)? };

        Ok(min..max)
    }

    /// Does this lirc device support setting send carrier
    pub fn can_use_wideband_receiver(&self) -> bool {
        (self.features & LIRC_CAN_USE_WIDEBAND_RECEIVER) != 0
    }

    /// Set the receiving timeout in microseconds
    pub fn set_wideband_receiver(&mut self, enable: bool) -> io::Result<()> {
        let enable = enable.into();

        unsafe { lirc_set_wideband_receiver(self.file.as_raw_fd(), &enable)? };

        Ok(())
    }

    /// Does this lirc device support measuring the carrier
    pub fn can_measure_carrier(&self) -> bool {
        (self.features & LIRC_CAN_MEASURE_CARRIER) != 0
    }

    /// Enabling measuring the carrier
    pub fn set_measure_carrier(&mut self, enable: bool) -> io::Result<()> {
        let enable = enable.into();

        unsafe { lirc_set_measure_carrier(self.file.as_raw_fd(), &enable)? };

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

            unsafe { lirc_set_rec_mode(self.file.as_raw_fd(), &mode)? };

            self.raw_mode = true;
        }

        let length = result.capacity() * mem::size_of::<LircRaw>();
        let data = unsafe { std::slice::from_raw_parts_mut(result.as_ptr() as *mut u8, length) };

        let res = match self.file.read(data) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        unsafe { result.set_len(res / mem::size_of::<LircRaw>()) };

        Ok(())
    }

    /// Does this lirc device support receiving in decoded scancode format
    pub fn can_receive_scancodes(&self) -> bool {
        (self.features & (LIRC_CAN_REC_MODE2 | LIRC_CAN_REC_SCANCODE)) != 0
    }

    /// Switch to scancode mode
    pub fn scancode_mode(&mut self) -> io::Result<()> {
        if self.raw_mode {
            let mode = LIRC_MODE_SCANCODE;

            unsafe { lirc_set_rec_mode(self.file.as_raw_fd(), &mode)? };

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
        let mut res = 0u32;

        unsafe { lirc_get_rec_resolution(self.file.as_raw_fd(), &mut res)? };

        Ok(res)
    }

    pub fn as_file(&self) -> &File {
        &self.file
    }

    /// Load and attach bpf program.
    pub fn attach_bpf(&self, bpf: &[u8]) -> Result<(), String> {
        let mut bpf = match aya::Bpf::load(bpf) {
            Ok(bpf) => bpf,
            Err(e) => {
                return Err(format!("{e}"));
            }
        };

        let mut iter = bpf.programs_mut();

        let Some((_, program)) = iter.next() else {
            return Err("missing program".into());
        };

        if iter.next().is_some() {
            return Err("only single program expected".into());
        }

        let program: &mut LircMode2 = match program.try_into() {
            Ok(program) => program,
            Err(e) => {
                return Err(format!("{e}"));
            }
        };

        if let Err(e) = program.load() {
            return Err(format!("{e}"));
        }

        match program.attach(self.as_fd()) {
            Ok(link) => {
                program.take_link(link).unwrap();

                Ok(())
            }
            Err(e) => Err(format!("{e}")),
        }
    }

    /// query bpf programs
    pub fn query_bpf(&self) -> Result<Vec<String>, ProgramError> {
        let links = LircMode2::query(self.as_fd())?;
        let mut res = Vec::new();

        for link in links {
            let info = link.info()?;
            match info.name_as_str() {
                Some(name) => res.push(name.to_owned()),
                None => res.push(format!("{}", info.id())),
            }
        }

        Ok(res)
    }

    /// Remove all attached bpf programs
    pub fn clear_bpf(&self) -> Result<(), ProgramError> {
        let links = LircMode2::query(self.as_fd())?;
        for link in links {
            link.detach()?;
        }
        Ok(())
    }
}

impl AsRawFd for Lirc {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl AsFd for Lirc {
    fn as_fd(&self) -> BorrowedFd {
        self.file.as_fd()
    }
}

impl fmt::Display for Lirc {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.path.display())
    }
}
