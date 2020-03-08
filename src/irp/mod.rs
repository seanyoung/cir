pub mod ast;

#[allow(clippy::all,unused_parens)]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub mod irp;

pub fn parse(input: &str) {
    let parser = irp::protocolParser::new();

    match parser.parse(input) {
        Ok(s) => {
            println!("irp: {:?}", s);
        }
        Err(r) => eprintln!("irp parse error {:?}", r),
    };
}
