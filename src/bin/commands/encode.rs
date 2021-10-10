use super::encode_args;

pub fn encode(matches: &clap::ArgMatches) {
    let message = encode_args(matches);

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
