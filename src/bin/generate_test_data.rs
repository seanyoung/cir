use ir_ctl::protocols;
use std::fs::File;
use std::io::prelude::*;
use std::process::Command;

mod transmogrifier;

fn main() {
    let protocols = protocols::read_protocols();
    let mut test_data = Vec::new();

    for protocol in protocols {
        for n in 0..10 {
            let repeats = if n < 3 {
                n
            } else {
                (rand::random::<u8>() % 20) + n
            };
            let number_repeats = format!("{}", repeats);
            let output = Command::new("irptransmogrifier.sh")
                .args(&[
                    "render",
                    "--random",
                    "-r",
                    "--number-repeats",
                    &number_repeats,
                    "-P",
                    &protocol.name,
                ])
                .output()
                .expect("Failed to execute irptransmogrifier.sh");

            if !output.status.success() {
                continue;
            }

            let result = String::from_utf8(output.stdout).unwrap();

            let data = transmogrifier::parse_output(protocol.name.clone(), repeats, &result);

            test_data.push(data);
        }
    }

    let test_data = serde_json::to_string(&test_data).unwrap();

    let mut file = File::create("transmogrifier_test_data.json").unwrap();
    file.write_all(test_data.as_bytes()).unwrap();
}
