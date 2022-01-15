use super::{encode_args, open_lirc, Purpose};

pub fn transmit(global_matches: &clap::ArgMatches) {
    let (message, matches) = encode_args(global_matches);

    let verbose = global_matches.is_present("VERBOSE") | matches.is_present("VERBOSE");

    let mut lircdev = open_lirc(matches, Purpose::Transmit);

    if let Some(values) = global_matches
        .values_of("TRANSMITTERS")
        .or_else(|| matches.values_of("TRANSMITTERS"))
    {
        let mut transmitters: Vec<u32> = Vec::new();
        for t in values {
            let mut found_transmitters = false;
            for t in t.split(&[' ', ';', ':', ','][..]) {
                if t.is_empty() {
                    continue;
                }
                match t.parse() {
                    Ok(0) | Err(_) => {
                        eprintln!("error: ‘{}’ is not a valid transmitter number", t);
                        std::process::exit(1);
                    }
                    Ok(v) => transmitters.push(v),
                }
                found_transmitters = true;
            }

            if !found_transmitters {
                eprintln!("error: ‘{}’ is not a valid transmitter number", t);
                std::process::exit(1);
            }
        }

        if !transmitters.is_empty() {
            if !lircdev.can_set_send_transmitter_mask() {
                eprintln!(
                    "error: {}: device does not support setting transmitters",
                    lircdev
                );

                std::process::exit(1);
            }

            let transmitter_count = match lircdev.num_transmitters() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error: {}: failed to get transmitter count: {}", lircdev, e);

                    std::process::exit(1);
                }
            };

            if let Some(t) = transmitters.iter().find(|t| **t > transmitter_count) {
                eprintln!(
                    "error: transmitter {} not valid, device has {} transmitters",
                    t, transmitter_count
                );

                std::process::exit(1);
            }

            let mask: u32 = transmitters.iter().fold(0, |acc, t| acc | (1 << (t - 1)));

            if matches.is_present("VERBOSE") {
                eprintln!("debug: setting transmitter mask {:08x}", mask);
            }

            match lircdev.set_transmitter_mask(mask) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error: {}: failed to set transmitter mask: {}", lircdev, e);

                    std::process::exit(1);
                }
            }
        }
    }

    let duty_cycle = if let Some(value) = matches.value_of("DUTY_CYCLE") {
        match value.parse() {
            Ok(d @ 1..=99) => Some(d),
            _ => {
                eprintln!("error: ‘{}’ duty cycle is not valid", value);

                std::process::exit(1);
            }
        }
    } else {
        message.duty_cycle
    };

    let carrier = if let Some(value) = matches.value_of("CARRIER") {
        match value.parse() {
            Ok(c @ 0..=1_000_000) => Some(c),
            _ => {
                eprintln!("error: ‘{}’ carrier is not valid", value);

                std::process::exit(1);
            }
        }
    } else {
        message.carrier
    };

    if verbose {
        if let Some(carrier) = &carrier {
            if *carrier == 0 {
                println!("carrier: unmodulated (no carrier)");
            } else {
                println!("carrier: {}Hz", carrier);
            }
        }
        if let Some(duty_cycle) = &duty_cycle {
            println!("duty cycle: {}%", duty_cycle);
        }
        println!("rawir: {}", message.print_rawir());
    }

    if let Some(duty_cycle) = duty_cycle {
        if lircdev.can_set_send_duty_cycle() {
            if let Err(s) = lircdev.set_send_duty_cycle(duty_cycle as u32) {
                eprintln!("error: {}: {}", lircdev, s);

                std::process::exit(1);
            }
        } else {
            eprintln!(
                "warning: {}: device does not support setting send duty cycle",
                lircdev
            );
        }
    }

    if let Some(carrier) = carrier {
        if lircdev.can_set_send_carrier() {
            if let Err(s) = lircdev.set_send_carrier(carrier as u32) {
                eprintln!("error: {}: {}", lircdev, s);

                if carrier == 0 {
                    eprintln!("info: not all lirc devices can send unmodulated");
                }
                std::process::exit(1);
            }
        } else {
            eprintln!(
                "warning: {}: device does not support setting carrier",
                lircdev
            );
        }
    }

    if let Err(s) = lircdev.send(&message.raw) {
        eprintln!("error: {}: {}", lircdev, s);
        std::process::exit(1);
    }
}
