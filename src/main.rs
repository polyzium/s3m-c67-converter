use std::{fs::File, io::Write, env};

use format_s3m::{S3MAdlibInstrument, S3MInstrument, S3MModule};

mod format_s3m;
mod format_c67;
mod conversion;

fn main() {
    let args: Vec<String> = env::args().collect();
    let module_file = File::open(&args[1]).unwrap();
    let module = S3MModule::load(module_file).unwrap();
    
    // for i in &module.instruments {
    //     if let S3MInstrument::Adlib(ai) = i {
    //         println!("{:?}", String::from_utf8(ai.filename.to_vec()).unwrap());
    //     }
    // }

    let converter = conversion::Converter::new(&module);
    let converted_module = converter.convert();
    dbg!("{:?}", &converted_module);
    let serialized_module = converted_module.serialize();
    let mut file = File::create("out.c67").unwrap();
    file.write(&serialized_module).unwrap();
}