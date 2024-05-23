use clap::{
    error::{Error, ErrorKind},
    value_parser, ArgAction, ArgMatches, Args, Command, FromArgMatches, Parser, Subcommand,
};
use irp::Protocol;
use log::{Level, LevelFilter, Metadata, Record};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

mod commands;

#[derive(Parser)]
#[command(
    name = "cir",
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = "Consumer Infrared",
    subcommand_required = true
)]
struct App {
    /// Increase message verbosity
    #[arg(long, short, action = ArgAction::Count, global = true, conflicts_with = "quiet")]
    verbose: u8,

    /// Silence all warnings
    #[arg(long, short, global = true, conflicts_with = "verbose")]
    quiet: bool,

    /// Location of IrpProtocols.xml
    #[arg(
        long = "irp-protocols",
        global = true,
        default_value = "/usr/share/rc_keymaps/IrpProtocols.xml"
    )]
    irp_protocols: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

enum Commands {
    #[cfg(target_os = "linux")]
    List(List),
    #[cfg(target_os = "linux")]
    Keymap(Keymap),
    Decode(Decode),
    Transmit(Transmit),
    #[cfg(target_os = "linux")]
    Test(Test),
}

#[derive(Args)]
struct Decode {
    #[cfg(target_os = "linux")]
    #[clap(flatten)]
    device: RcDevice,

    /// Use short-range learning mode
    #[cfg(target_os = "linux")]
    #[arg(
        long = "learning-mode",
        short = 'l',
        global = true,
        help_heading = "DEVICE"
    )]
    learning: bool,

    /// Read from rawir or mode2 file
    #[arg(
        long = "file",
        short = 'f',
        global = true,
        name = "FILE",
        help_heading = "INPUT"
    )]
    file: Vec<OsString>,

    /// Raw IR text
    #[arg(
        long = "raw",
        short = 'r',
        global = true,
        name = "RAWIR",
        help_heading = "INPUT"
    )]
    rawir: Vec<String>,

    /// IRP Notation
    #[arg(long = "irp", short = 'i', required_unless_present = "keymap")]
    irp: Vec<String>,

    /// Keymap or lircd.conf file
    #[arg(long = "keymap", short = 'k', required_unless_present = "irp")]
    keymap: Option<PathBuf>,

    #[clap(flatten)]
    options: DecodeOptions,
}

#[derive(Args)]
struct DecodeOptions {
    /// Absolute tolerance in microseconds
    #[arg(
            long = "absolute-tolerance",
            value_parser = value_parser!(u32).range(0..100000),
            global = true,
            name = "AEPS",
            help_heading = "DECODING"
        )]
    aeps: Option<u32>,

    /// Relative tolerance in %
    #[arg(
            long = "relative-tolerance",
            value_parser = value_parser!(u32).range(0..1000),
            global = true,
            name = "EPS",
            help_heading = "DECODING"
        )]
    eps: Option<u32>,

    /// Save the NFA
    #[arg(long = "save-nfa", global = true, help_heading = "DEBUGGING")]
    save_nfa: bool,

    /// Save the DFA
    #[arg(long = "save-dfa", global = true, help_heading = "DEBUGGING")]
    save_dfa: bool,
}

#[derive(Args)]
struct BpfDecodeOptions {
    /// Save the LLVM IR
    #[arg(long = "save-llvm-ir", help_heading = "DEBUGGING")]
    save_llvm_ir: bool,

    /// Save the Assembly
    #[arg(long = "save-asm", help_heading = "DEBUGGING")]
    save_assembly: bool,

    /// Save the Object
    #[arg(long = "save-object", help_heading = "DEBUGGING")]
    save_object: bool,
}

#[cfg(target_os = "linux")]
#[derive(Args)]
struct RcDevice {
    /// Select device to use by lirc chardev (e.g. /dev/lirc1)
    #[arg(
        long = "device",
        short = 'd',
        conflicts_with = "RCDEV",
        name = "LIRCDEV",
        global = true,
        help_heading = "DEVICE"
    )]
    lirc_dev: Option<String>,

    /// Select device to use by rc core device (e.g. rc0)
    #[arg(
        long = "rcdev",
        short = 's',
        conflicts_with = "LIRCDEV",
        name = "RCDEV",
        global = true,
        help_heading = "DEVICE"
    )]
    rc_dev: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Args)]
struct List {
    #[cfg(target_os = "linux")]
    #[clap(flatten)]
    device: RcDevice,

    /// Display the scancode to keycode mapping
    #[arg(long = "read-mapping", short = 'r')]
    mapping: bool,
}

#[cfg(target_os = "linux")]
fn parse_scankey(arg: &str) -> Result<(u64, String), String> {
    if let Some((scancode, keycode)) = arg.split_once([':', '=']) {
        let scancode = if let Some(hex) = scancode.strip_prefix("0x") {
            u64::from_str_radix(hex, 16)
        } else {
            str::parse(scancode)
        }
        .map_err(|e| format!("{e}"))?;

        Ok((scancode, keycode.to_owned()))
    } else {
        Err("missing `=` separator".into())
    }
}

fn parse_scancode(arg: &str) -> Result<(String, u64), String> {
    if let Some((protocol, scancode)) = arg.split_once(':') {
        let scancode = if let Some(hex) = scancode.strip_prefix("0x") {
            u64::from_str_radix(hex, 16)
        } else {
            str::parse(scancode)
        }
        .map_err(|e| format!("{e}"))?;

        Ok((protocol.to_owned(), scancode))
    } else {
        Err("missing `:` separator".into())
    }
}

#[cfg(target_os = "linux")]
#[derive(Args)]
struct Keymap {
    #[cfg(target_os = "linux")]
    #[clap(flatten)]
    device: RcDevice,

    /// Auto-load keymaps, based on a configuration file.
    #[arg(long = "auto-load", short = 'a', conflicts_with_all = ["clear", "KEYMAP", "IRP", "PROTOCOL", "SCANKEY"])]
    auto_load: Option<PathBuf>,

    /// Clear existing configuration
    #[arg(long = "clear", short = 'c')]
    clear: bool,

    /// Set receiving timeout
    #[arg(long = "timeout", short = 't')]
    timeout: Option<u32>,

    /// Sets the delay before repeating a keystroke
    #[arg(long = "delay", short = 'D', name = "DELAY")]
    delay: Option<u32>,

    /// Sets the period before repeating a keystroke
    #[arg(long = "period", short = 'P', name = "PERIOD")]
    period: Option<u32>,

    /// Load decoder based on IRP Notation
    #[arg(long = "irp", short = 'i', name = "IRP")]
    irp: Option<String>,

    /// Protocol to enable
    #[arg(
        long = "protocol",
        short = 'p',
        value_delimiter = ',',
        name = "PROTOCOL"
    )]
    protocol: Vec<String>,

    /// Scancode to keycode mapping to add
    #[arg(long = "set-key", short = 'k', value_parser = parse_scankey, value_delimiter = ',', name = "SCANKEY")]
    scankey: Vec<(u64, String)>,

    /// Load toml or lircd.conf keymap
    #[arg(name = "KEYMAP")]
    write: Vec<PathBuf>,

    #[clap(flatten)]
    options: DecodeOptions,

    #[clap(flatten)]
    bpf_options: BpfDecodeOptions,
}

#[cfg(target_os = "linux")]
#[derive(Args)]
struct Test {
    #[cfg(target_os = "linux")]
    #[clap(flatten)]
    device: RcDevice,

    /// Use short-range learning mode
    #[arg(long = "learning", short = 'l')]
    learning: bool,

    /// Set receiving timeout
    #[arg(long = "timeout", short = 't')]
    timeout: Option<u32>,

    /// Stop receiving after first timeout message
    #[arg(long = "one-shot", short = '1')]
    one_shot: bool,

    /// Only print raw IR
    #[arg(long = "raw", short = 'r')]
    raw: bool,
}

#[cfg(target_os = "linux")]
#[derive(Args)]
struct Auto {
    #[clap(flatten)]
    device: RcDevice,

    /// Configuration file
    #[arg(name = "CFGFILE", default_value = "/etc/rc_maps.cfg")]
    cfgfile: PathBuf,
}

#[derive(Args)]
struct Transmit {
    #[cfg(target_os = "linux")]
    #[clap(flatten)]
    device: RcDevice,

    /// Comma separated list of transmitters to use, starting from 1
    #[cfg(target_os = "linux")]
    #[arg(
        long = "transmitters",
        short = 'e',
        value_delimiter = ',',
        help_heading = "DEVICE"
    )]
    transmitters: Vec<u32>,

    /// Encode IR but do not actually send
    #[arg(long = "dry-run", short = 'n')]
    dry_run: bool,

    /// List the codes in keymap
    #[arg(long = "list-codes", short = 'l', requires = "KEYMAP")]
    list_codes: bool,

    /// Read from rawir or mode2 file
    #[arg(long = "file", short = 'f', name = "FILE", help_heading = "INPUT")]
    files: Vec<OsString>,

    /// Send scancode using linux kernel protocol
    #[arg(
        long = "scancode",
        short = 'S',
        name = "SCANCODE",
        value_parser = parse_scancode,
        help_heading = "INPUT"
    )]
    scancodes: Vec<(String, u64)>,

    /// Trailing gap length if none present
    #[arg(long = "gap", short = 'g', name = "GAP", help_heading = "INPUT")]
    gaps: Vec<u32>,

    /// Pronto Hex code
    #[arg(long = "pronto", short = 'p', name = "PRONTO", help_heading = "INPUT")]
    pronto: Vec<String>,

    /// Transmit raw IR
    #[arg(long = "raw", short = 'r', name = "RAWIR", help_heading = "INPUT")]
    rawir: Vec<String>,

    /// Number of repeats to encode
    #[arg(
        long = "repeats",
        short = 'R',
        value_parser = value_parser!(u64).range(0..99),
        default_value_t = 0,
        help_heading = "INPUT"
    )]
    repeats: u64,

    /// Set IRP parameter like KEY=VALUE
    #[arg(
        long = "argument",
        short = 'a',
        value_delimiter = ',',
        help_heading = "INPUT",
        name = "ARGUMENT"
    )]
    arguments: Vec<String>,

    /// Transmit using IRP Notation
    #[arg(long = "irp", short = 'i', name = "IRP", help_heading = "INPUT")]
    irp: Vec<String>,

    /// Keymap or lircd.conf file
    #[arg(name = "KEYMAP", long = "keymap", short = 'k', help_heading = "INPUT")]
    keymap: Option<PathBuf>,

    /// Remote to use from lircd.conf file
    #[arg(name = "REMOTE", long = "remote", short = 'm', help_heading = "INPUT")]
    remote: Option<String>,

    /// Code from keymap to transmit
    #[arg(name = "CODE", long = "keycode", short = 'K', help_heading = "INPUT")]
    codes: Vec<String>,

    /// Set carrier in Hz, 0 for unmodulated
    #[cfg(target_os = "linux")]
    #[arg(long = "carrier", short = 'c', value_parser = value_parser!(i64).range(1..1_000_000), help_heading = "DEVICE")]
    carrier: Option<i64>,

    /// Set send duty cycle % (1 to 99)
    #[cfg(target_os = "linux")]
    #[arg(long = "duty-cycle", short = 'u', value_parser = value_parser!(u8).range(1..99), help_heading = "DEVICE")]
    duty_cycle: Option<u8>,

    #[arg(skip)]
    transmitables: Vec<Transmitables>,
}

impl Transmit {
    fn transmitables(&mut self, matches: &ArgMatches) {
        let mut part = Vec::new();

        macro_rules! arg {
            ($id:literal, $ty:ty, $transmitable:ident) => {{}
            if let Some(values) = matches.get_many::<$ty>($id) {
                let mut indices = matches.indices_of($id).unwrap();

                for value in values {
                    part.push((
                        Transmitables::$transmitable(value.clone()),
                        indices.next().unwrap(),
                    ));
                }
            }};
        }

        arg!("FILE", OsString, File);
        arg!("RAWIR", String, RawIR);
        arg!("PRONTO", String, Pronto);
        arg!("CODE", String, Code);
        arg!("IRP", String, Irp);
        arg!("GAP", u32, Gap);
        arg!("SCANCODE", (String, u64), Scancode);

        part.sort_by(|a, b| a.1.cmp(&b.1));

        self.transmitables = part.into_iter().map(|(t, _)| t).collect();
    }
}

enum Transmitables {
    File(OsString),
    RawIR(String),
    Pronto(String),
    Code(String),
    Irp(String),
    Gap(u32),
    Scancode((String, u64)),
}

impl FromArgMatches for Commands {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        match matches.subcommand() {
            Some(("decode", args)) => Ok(Self::Decode(Decode::from_arg_matches(args)?)),
            Some(("transmit", args)) => {
                let mut tx = Transmit::from_arg_matches(args)?;

                tx.transmitables(args);

                Ok(Self::Transmit(tx))
            }
            #[cfg(target_os = "linux")]
            Some(("list", args)) => Ok(Self::List(List::from_arg_matches(args)?)),
            #[cfg(target_os = "linux")]
            Some(("keymap", args)) => Ok(Self::Keymap(Keymap::from_arg_matches(args)?)),
            #[cfg(target_os = "linux")]
            Some(("test", args)) => Ok(Self::Test(Test::from_arg_matches(args)?)),
            Some((_, _)) => Err(Error::raw(
                ErrorKind::InvalidSubcommand,
                "Valid subcommands are `decode`, `transmit`, `list`, `keymap`, and `test`",
            )),
            None => Err(Error::raw(
                ErrorKind::MissingSubcommand,
                "Valid subcommands are `decode`, `transmit`, `list`, `keymap`, and `test``",
            )),
        }
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        match matches.subcommand() {
            Some(("decode", args)) => *self = Self::Decode(Decode::from_arg_matches(args)?),
            Some(("transmit", args)) => {
                let mut tx = Transmit::from_arg_matches(args)?;

                tx.transmitables(args);

                *self = Self::Transmit(tx);
            }
            #[cfg(target_os = "linux")]
            Some(("list", args)) => *self = Self::List(List::from_arg_matches(args)?),
            #[cfg(target_os = "linux")]
            Some(("keymap", args)) => *self = Self::Keymap(Keymap::from_arg_matches(args)?),
            #[cfg(target_os = "linux")]
            Some(("test", args)) => *self = Self::Test(Test::from_arg_matches(args)?),
            Some((_, _)) => {
                return Err(Error::raw(
                    ErrorKind::InvalidSubcommand,
                    "Valid subcommands are `decode`, `transmit`, `list`, `keymap`, and `test`",
                ))
            }
            None => (),
        }

        Ok(())
    }
}

impl Subcommand for Commands {
    #[allow(clippy::let_and_return)]
    fn augment_subcommands(cmd: Command) -> Command {
        let cmd = cmd
            .subcommand(Decode::augment_args(
                Command::new("decode").about("Decode IR"),
            ))
            .subcommand(Transmit::augment_args(
                Command::new("transmit").about("Transmit IR"),
            ));

        #[cfg(target_os = "linux")]
        let cmd = cmd
            .subcommand(List::augment_args(
                Command::new("list").about("List IR and CEC devices"),
            ))
            .subcommand_required(true)
            .subcommand(List::augment_args(
                Command::new("keymap").about("Configure IR and CEC devices"),
            ))
            .subcommand_required(true)
            .subcommand(List::augment_args(
                Command::new("test").about("Receive IR and print to stdout"),
            ))
            .subcommand_required(true);

        cmd
    }

    #[allow(clippy::let_and_return)]
    fn augment_subcommands_for_update(cmd: Command) -> Command {
        let cmd = cmd
            .subcommand(Decode::augment_args(
                Command::new("decode").about("Decode IR"),
            ))
            .subcommand(Transmit::augment_args(
                Command::new("transmit").about("Transmit IR"),
            ));

        #[cfg(target_os = "linux")]
        let cmd = cmd
            .subcommand(List::augment_args(
                Command::new("list").about("List IR and CEC devices"),
            ))
            .subcommand_required(true)
            .subcommand(List::augment_args(
                Command::new("keymap").about("Configure IR and CEC devices"),
            ))
            .subcommand_required(true)
            .subcommand(List::augment_args(
                Command::new("test").about("Receive IR and print to stdout"),
            ))
            .subcommand_required(true);

        cmd
    }

    fn has_subcommand(_name: &str) -> bool {
        false
    }
}

fn main() {
    let args = App::parse();

    log::set_logger(&CLI_LOGGER).unwrap();

    let level = if args.quiet {
        LevelFilter::Error
    } else {
        match args.verbose {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    log::set_max_level(level);

    match &args.command {
        Commands::Decode(decode) => {
            if !decode.irp.is_empty() {
                commands::decode::decode_irp(&args.irp_protocols, decode)
            } else {
                let keymap = decode.keymap.as_ref().unwrap();

                if keymap.to_string_lossy().ends_with(".lircd.conf") {
                    commands::decode::decode_lircd(decode, keymap);
                } else {
                    commands::decode::decode_keymap(decode, keymap);
                }
            }
        }
        Commands::Transmit(tx) => commands::transmit::transmit(&args, tx),
        #[cfg(target_os = "linux")]
        Commands::List(args) => commands::list::list(args),
        #[cfg(target_os = "linux")]
        Commands::Keymap(args) => commands::keymap::keymap(args),
        #[cfg(target_os = "linux")]
        Commands::Test(args) => commands::test::test(args),
    }
}

static IRP_PROTOCOLS: OnceLock<io::Result<Vec<Protocol>>> = OnceLock::new();

fn get_irp_protocols(path: &Path) -> &'static io::Result<Vec<Protocol>> {
    IRP_PROTOCOLS.get_or_init(|| Protocol::parse(path))
}

static CLI_LOGGER: CliLogger = CliLogger;

struct CliLogger;

impl log::Log for CliLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "{}: {}",
                match record.level() {
                    Level::Trace => "trace",
                    Level::Debug => "debug",
                    Level::Info => "info",
                    Level::Warn => "warn",
                    Level::Error => "error",
                },
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
