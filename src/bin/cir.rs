use clap::{
    error::{Error, ErrorKind},
    value_parser, ArgAction, ArgMatches, Args, Command, FromArgMatches, Parser, Subcommand,
};
use log::{Level, LevelFilter, Metadata, Record};
use std::{ffi::OsString, path::PathBuf};

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

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Decode IR", arg_required_else_help = true)]
    Decode(Decode),
    #[command(about = "Transmit IR", arg_required_else_help = true)]
    Transmit(Transmit),
    #[cfg(target_os = "linux")]
    #[command(about = "List IR and CEC devices")]
    Config(Config),
    #[cfg(target_os = "linux")]
    #[command(about = "Receive IR and print to stdout")]
    Test(Test),
    #[cfg(target_os = "linux")]
    #[command(about = "Auto-load keymaps based on configuration")]
    Auto(Auto),
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

    #[clap(flatten)]
    options: DecodeOptions,

    #[command(subcommand)]
    commands: DecodeCommands,
}

#[derive(Args)]
struct DecodeOptions {
    /// Absolute tolerance in microseconds
    #[arg(
            long = "absolute-tolerance",
            value_parser = value_parser!(u32).range(0..100000),
            global = true,
            default_value_t = 100,
            name = "AEPS",
            help_heading = "DECODING"
        )]
    aeps: u32,

    /// Relative tolerance in %
    #[arg(
            long = "relative-tolerance",
            value_parser = value_parser!(u32).range(0..1000),
            global = true,
            default_value_t = 3,
            name = "EPS",
            help_heading = "DECODING"
        )]
    eps: u32,

    /// Save the NFA
    #[arg(long = "save-nfa", global = true, help_heading = "DECODING")]
    save_nfa: bool,

    /// Save the DFA
    #[arg(long = "save-dfa", global = true, help_heading = "DECODING")]
    save_dfa: bool,
}
#[derive(Subcommand)]
enum DecodeCommands {
    #[command(about = "Decode using IRP Notation")]
    Irp(DecodeIrp),

    #[command(about = "Decode using lircd.conf file")]
    Lircd(DecodeLircd),
}

#[derive(Args)]
struct DecodeIrp {
    /// IRP Notation
    irp: String,
}

#[derive(Args)]
struct DecodeLircd {
    /// lircd.conf file
    lircdconf: OsString,
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
struct Config {
    #[cfg(target_os = "linux")]
    #[clap(flatten)]
    device: RcDevice,

    /// Display the scancode to keycode mapping
    #[arg(long = "read-mapping", short = 'm')]
    mapping: bool,

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

    /// Load toml or lircd.conf keymap
    #[arg(long = "keymap", short = 'w', name = "KEYMAP")]
    keymaps: Vec<PathBuf>,

    // TODO:
    // --irp '{}<>()[]'
    // set scancode (like ir-keytable --set-key/-k)
    // set protocol (like ir-keytabke -P)
    #[clap(flatten)]
    options: DecodeOptions,

    /// Save the LLVM IR
    #[arg(long = "save-llvm-ir", help_heading = "DECODING")]
    save_llvm_ir: bool,

    /// Save the Assembly
    #[arg(long = "save-asm", help_heading = "DECODING")]
    save_assembly: bool,

    /// Save the Object
    #[arg(long = "save-object", help_heading = "DECODING")]
    save_object: bool,
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
        global = true,
        value_delimiter = ',',
        help_heading = "DEVICE"
    )]
    transmitters: Vec<u32>,

    /// Encode IR but do not actually send
    #[arg(long = "dry-run", short = 'n', global = true)]
    dry_run: bool,

    #[command(subcommand)]
    commands: TransmitCommands,
}

#[derive(Debug)]
enum TransmitCommands {
    Irp(TransmitIrp),
    Pronto(TransmitPronto),
    RawIR(TransmitRawIR),
    Keymap(TransmitKeymap),
}

#[derive(Args, Debug)]
struct TransmitIrp {
    #[arg(long, hide = true)]
    pronto: bool,

    /// Set carrier in Hz, 0 for unmodulated
    #[arg(long = "carrier", short = 'c', value_parser = value_parser!(i64).range(1..1_000_000), hide = true, help_heading = "DEVICE")]
    carrier: Option<i64>,

    /// Override duty cycle % (1 to 99)
    #[arg(long = "duty-cycle", short = 'u', value_parser = value_parser!(u8).range(1..99), help_heading = "DEVICE")]
    duty_cycle: Option<u8>,

    /// Number of IRP repeats to encode
    #[arg(long = "repeats", short = 'r', value_parser = value_parser!(u64).range(0..99), default_value_t = 1)]
    repeats: u64,

    /// Set input variable like KEY=VALUE
    #[arg(long = "field", short = 'f', value_delimiter = ',')]
    fields: Vec<String>,

    /// IRP protocol
    #[arg(name = "IRP")]
    irp: String,
}

#[derive(Args, Debug)]
struct TransmitPronto {
    /// Number of times to repeat signal
    #[arg(long = "repeats", short = 'r', value_parser = value_parser!(u64).range(0..99), default_value_t = 1)]
    repeats: u64,

    /// Pronto Hex code
    #[arg(name = "PRONTO")]
    pronto: String,
}

#[derive(Args, Debug)]
struct TransmitRawIR {
    /// Read from rawir or mode2 file
    #[arg(long = "file", short = 'f', name = "FILE", help_heading = "INPUT")]
    files: Vec<OsString>,

    /// Send scancode using old linux kernel protocols
    #[arg(
        long = "scancode",
        short = 'S',
        name = "SCANCODE",
        help_heading = "INPUT"
    )]
    scancodes: Vec<String>,

    /// Set gap after each file
    #[arg(long = "gap", short = 'g', name = "GAP", help_heading = "INPUT")]
    gaps: Vec<u32>,

    /// Raw IR text
    #[arg(name = "RAWIR", help_heading = "INPUT")]
    rawir: Vec<String>,

    /// Set carrier in Hz, 0 for unmodulated
    #[arg(long = "carrier", short = 'c', value_parser = value_parser!(i64).range(1..1_000_000), hide = true, help_heading = "DEVICE")]
    carrier: Option<i64>,

    /// Set send duty cycle % (1 to 99)
    #[arg(long = "duty-cycle", short = 'u', value_parser = value_parser!(u8).range(1..99), help_heading = "DEVICE")]
    duty_cycle: Option<u8>,

    #[arg(skip)]
    transmitables: Vec<Transmitables>,
}

impl TransmitRawIR {
    fn transmitables(&mut self, matches: &ArgMatches) {
        let mut part = Vec::new();

        if let Some(files) = matches.get_many::<OsString>("FILE") {
            let mut indices = matches.indices_of("FILE").unwrap();

            for file in files {
                part.push((Transmitables::File(file.clone()), indices.next().unwrap()));
            }
        }

        if let Some(rawirs) = matches.get_many::<String>("RAWIR") {
            let mut indices = matches.indices_of("RAWIR").unwrap();

            for rawir in rawirs {
                part.push((Transmitables::RawIR(rawir.clone()), indices.next().unwrap()));
            }
        }

        if let Some(scancodes) = matches.get_many::<String>("SCANCODE") {
            let mut indices = matches.indices_of("SCANCODE").unwrap();

            for scancode in scancodes {
                part.push((
                    Transmitables::Scancode(scancode.clone()),
                    indices.next().unwrap(),
                ));
            }
        }

        if let Some(gaps) = matches.get_many::<u32>("GAP") {
            let mut indices = matches.indices_of("GAP").unwrap();

            for gap in gaps {
                part.push((Transmitables::Gap(*gap), indices.next().unwrap()));
            }
        }

        part.sort_by(|a, b| a.1.cmp(&b.1));

        self.transmitables = part.into_iter().map(|(t, _)| t).collect();
    }
}

#[derive(Debug)]
enum Transmitables {
    File(OsString),
    RawIR(String),
    Gap(u32),
    Scancode(String),
}

#[derive(Args, Debug)]
struct TransmitKeymap {
    /// Override carrier in Hz, 0 for unmodulated
    #[arg(long = "carrier", short = 'c', value_parser = value_parser!(i64).range(0..1_000_000), help_heading = "DEVICE")]
    carrier: Option<i64>,

    /// Override duty cycle % (1 to 99)
    #[arg(long = "duty-cycle", short = 'u', value_parser = value_parser!(u8).range(1..99), help_heading = "DEVICE")]
    duty_cycle: Option<u8>,

    /// Keymap or lircd.conf file
    #[arg(name = "KEYMAP")]
    keymap: PathBuf,

    /// Remote to use from lircd.conf file
    #[arg(name = "REMOTE", long = "remote", short = 'm')]
    remote: Option<String>,

    /// Number of times to repeat signal
    #[arg(long = "repeats", short = 'r', value_parser = value_parser!(u64).range(0..99), default_value_t = 0)]
    repeats: u64,

    /// Code to send, leave empty to list codes
    #[arg(name = "CODES")]
    codes: Vec<String>,
}

impl FromArgMatches for TransmitCommands {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        match matches.subcommand() {
            Some(("irp", args)) => Ok(Self::Irp(TransmitIrp::from_arg_matches(args)?)),
            Some(("rawir", args)) => {
                let mut rawir = TransmitRawIR::from_arg_matches(args)?;

                rawir.transmitables(args);

                Ok(Self::RawIR(rawir))
            }
            Some(("pronto", args)) => Ok(Self::Pronto(TransmitPronto::from_arg_matches(args)?)),
            Some(("keymap", args)) => Ok(Self::Keymap(TransmitKeymap::from_arg_matches(args)?)),
            Some((_, _)) => Err(Error::raw(
                ErrorKind::InvalidSubcommand,
                "Valid subcommands are `irp`, `keymap`, `pronto`,  and `rawir`",
            )),
            None => Err(Error::raw(
                ErrorKind::MissingSubcommand,
                "Valid subcommands are `irp`, `keymap`, `pronto`,  and `rawir`",
            )),
        }
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        match matches.subcommand() {
            Some(("irp", args)) => *self = Self::Irp(TransmitIrp::from_arg_matches(args)?),
            Some(("rawir", args)) => {
                let mut rawir = TransmitRawIR::from_arg_matches(args)?;

                rawir.transmitables(args);

                *self = Self::RawIR(rawir);
            }
            Some(("pronto", args)) => *self = Self::Pronto(TransmitPronto::from_arg_matches(args)?),
            Some(("keymap", args)) => *self = Self::Keymap(TransmitKeymap::from_arg_matches(args)?),
            Some((_, _)) => {
                return Err(Error::raw(
                    ErrorKind::InvalidSubcommand,
                    "Valid subcommands are `irp`, `keymap`, `pronto`,  and `rawir`",
                ))
            }
            None => (),
        }

        Ok(())
    }
}

impl Subcommand for TransmitCommands {
    fn augment_subcommands(cmd: Command) -> Command {
        cmd.subcommand(TransmitIrp::augment_args(
            Command::new("irp").about("Encode using IRP language and transmit"),
        ))
        .subcommand(TransmitKeymap::augment_args(
            Command::new("keymap").about("Transmit codes from keymap or lircd.conf file"),
        ))
        .subcommand(TransmitPronto::augment_args(
            Command::new("pronto").about("Parse pronto hex code and transmit"),
        ))
        .subcommand(TransmitRawIR::augment_args(
            Command::new("rawir").about("Parse raw IR and transmit"),
        ))
        .subcommand_required(true)
    }
    fn augment_subcommands_for_update(cmd: Command) -> Command {
        cmd.subcommand(TransmitIrp::augment_args(
            Command::new("irp").about("Encode using IRP language and transmit"),
        ))
        .subcommand(TransmitKeymap::augment_args(
            Command::new("keymap").about("Transmit codes from keymap or lircd.conf file"),
        ))
        .subcommand(TransmitPronto::augment_args(
            Command::new("pronto").about("Parse pronto hex code and transmit"),
        ))
        .subcommand(TransmitRawIR::augment_args(
            Command::new("rawir").about("Parse raw IR and transmit"),
        ))
        .subcommand_required(true)
    }
    fn has_subcommand(name: &str) -> bool {
        matches!(name, "irp" | "keymap" | "pronto" | "rawir")
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
        Commands::Decode(decode) => match &decode.commands {
            DecodeCommands::Irp(irp) => {
                commands::decode::decode_irp(decode, &irp.irp);
            }
            DecodeCommands::Lircd(lircd) => {
                commands::decode::decode_lircd(decode, &lircd.lircdconf);
            }
        },
        Commands::Transmit(transmit) => commands::transmit::transmit(transmit),
        #[cfg(target_os = "linux")]
        Commands::Config(config) => commands::config::config(config),
        #[cfg(target_os = "linux")]
        Commands::Test(test) => commands::test::test(test),
        #[cfg(target_os = "linux")]
        Commands::Auto(auto) => commands::config::auto(auto),
    }
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
