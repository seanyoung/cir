use super::encode_args;
use linux_infrared::log::Log;

pub fn encode(matches: &clap::ArgMatches, log: &Log) {
    let (message, _) = encode_args(matches, log);

    if let Some(carrier) = &message.carrier {
        if *carrier == 0 {
            println!("carrier: unmodulated (no carrier)");
        } else {
            println!("carrier: {}Hz", carrier);
        }
    }
    if let Some(duty_cycle) = &message.duty_cycle {
        println!("duty cycle: {}%", duty_cycle);
    }
    println!("rawir: {}", message.print_rawir());
}
