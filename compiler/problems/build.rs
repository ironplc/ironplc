use std::{
    env,
    error::Error,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process,
};

struct ProblemDef {
    /// The code that users know this as. This should remain stable
    /// between releases to facilitate consistent documentation.
    code: String,
    /// The internal name that this error is known as. This makes for
    /// easy reading, but we don't promise that this remains consistent
    /// between releases.
    name: String,
    /// A message describing the type of error.
    message: String,
}

fn create_problems() -> Result<(), Box<dyn Error>> {
    // Tell Cargo that if the error definitions change, to rerun this build script.
    println!("cargo:rerun-if-changed=resources/problem-codes.csv");

    // Read the problem-codes.csv into bytes so that we can use it with
    let mut src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    src_path.push("resources");
    src_path.push("problem-codes.csv");

    let src = fs::read_to_string(src_path).expect("Unable to read 'problem-codes.csv'");
    println!("{}", src);
    let src = src.as_bytes();

    // Read the file into the definition (we'll iterate over the structs more than once)
    let mut defs = vec![];
    let mut rdr = csv::Reader::from_reader(src);
    for result in rdr.records() {
        let record = result?;
        println!("{:?}", record);
        let code = record
            .get(0)
            .ok_or_else(|| format!("Record {:?} is not valid at column 0", record))?;
        let name = record
            .get(1)
            .ok_or_else(|| format!("Record {:?} is not valid at column 1", record))?;
        let message = record
            .get(2)
            .ok_or_else(|| format!("Record {:?} is not valid at column 2", record))?;
        defs.push(ProblemDef {
            code: code.to_string(),
            name: name.to_string(),
            message: message.to_string(),
        });
    }

    // Create the output directory and file problems.rs that will have the definitions
    let mut out_path = PathBuf::from(env::var("OUT_DIR")?);
    fs::create_dir_all(out_path.clone())
        .map_err(|e| format!("Unable to create directory 'problems': {}", e))?;

    out_path.push("problems.rs");
    let mut out =
        File::create(out_path).map_err(|e| format!("Unable to create 'problems.rs': {}", e))?;

    // Create the enumeration definition
    out.write_all(b"pub enum Problem {\n")?;
    for def in &defs {
        out.write_all(format!("    {},\n", def.name).as_bytes())?;
    }
    out.write_all(b"}\n\n")?;

    // Create the function to return information about each definition
    out.write_all(b"impl Problem {\n")?;

    // Define code()
    out.write_all(b"    /// Returns the code for the particular problem as a string.\n")?;
    out.write_all(b"    pub fn code(&self) -> &str {\n")?;
    out.write_all(b"        match self {\n")?;
    for def in &defs {
        out.write_all(
            format!("            Problem::{} => \"{}\",\n", def.name, def.code).as_bytes(),
        )?;
    }
    out.write_all(b"        }\n")?;
    out.write_all(b"    }\n\n")?;

    // Define message()
    out.write_all(b"    /// Returns the message for the particular problem as a string.\n")?;
    out.write_all(b"    /// The message is constant and does not depend on the particular instance of the problem.\n")?;
    out.write_all(b"    pub fn message(&self) -> &str {\n")?;
    out.write_all(b"        match self {\n")?;
    for def in &defs {
        out.write_all(
            format!(
                "            Problem::{} => \"{}\",\n",
                def.name, def.message
            )
            .as_bytes(),
        )?;
    }
    out.write_all(b"        }\n")?;
    out.write_all(b"    }\n")?;

    out.write_all(b"}\n")?;

    // Finalize the out file
    out.flush()?;

    Ok(())
}

fn main() {
    if let Err(err) = create_problems() {
        println!("problem generating problems.rs: {}", err);
        process::exit(1);
    }
}
