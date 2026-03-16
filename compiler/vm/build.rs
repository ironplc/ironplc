use std::{
    env,
    error::Error,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process,
};

struct VCodeDef {
    code: String,
    name: String,
    has_data: bool,
    is_struct: bool,
}

fn generate_trap_codes() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=resources/problem-codes.csv");

    let mut src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    src_path.push("resources");
    src_path.push("problem-codes.csv");

    let src = fs::read_to_string(src_path).expect("Unable to read 'problem-codes.csv'");
    let src = src.as_bytes();

    let mut defs = vec![];
    let mut rdr = csv::Reader::from_reader(src);
    for result in rdr.records() {
        let record = result?;
        let code = record
            .get(0)
            .ok_or_else(|| format!("Record {record:?} is not valid at column 0"))?
            .to_string();
        let name = record
            .get(1)
            .ok_or_else(|| format!("Record {record:?} is not valid at column 1"))?
            .to_string();
        let has_data_str = record
            .get(3)
            .ok_or_else(|| format!("Record {record:?} is not valid at column 3"))?;
        let is_struct = has_data_str == "struct";
        let has_data = has_data_str == "true" || is_struct;
        defs.push(VCodeDef {
            code,
            name,
            has_data,
            is_struct,
        });
    }

    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("trap_codes.rs");
    let mut out = File::create(out_path)?;

    out.write_all(b"impl Trap {\n")?;

    // Generate v_code()
    out.write_all(b"    /// Returns the V-code string for this trap (e.g., \"V4001\").\n")?;
    out.write_all(b"    ///\n")?;
    out.write_all(
        b"    /// V4xxx codes are runtime execution errors caused by the user's program.\n",
    )?;
    out.write_all(
        b"    /// V9xxx codes are internal VM errors indicating a compiler or VM bug.\n",
    )?;
    out.write_all(b"    pub fn v_code(&self) -> &'static str {\n")?;
    out.write_all(b"        match self {\n")?;
    for def in &defs {
        let pattern = if def.is_struct {
            " { .. }"
        } else if def.has_data {
            "(..)"
        } else {
            ""
        };
        out.write_all(
            format!(
                "            Trap::{}{} => \"{}\",\n",
                def.name, pattern, def.code,
            )
            .as_bytes(),
        )?;
    }
    out.write_all(b"        }\n")?;
    out.write_all(b"    }\n\n")?;

    // Generate exit_code()
    out.write_all(b"    /// Returns the process exit code for this trap's category.\n")?;
    out.write_all(b"    ///\n")?;
    out.write_all(b"    /// - 1: Runtime execution error (user's program faulted)\n")?;
    out.write_all(b"    /// - 3: Internal VM error (compiler or VM bug)\n")?;
    out.write_all(b"    pub fn exit_code(&self) -> u8 {\n")?;
    out.write_all(b"        match self {\n")?;
    for def in &defs {
        let exit_code = if def.code.starts_with("V4") {
            1
        } else if def.code.starts_with("V9") {
            3
        } else {
            panic!("Unexpected V-code prefix for trap: {}", def.code);
        };
        let pattern = if def.is_struct {
            " { .. }"
        } else if def.has_data {
            "(..)"
        } else {
            ""
        };
        out.write_all(
            format!(
                "            Trap::{}{} => {exit_code},\n",
                def.name, pattern,
            )
            .as_bytes(),
        )?;
    }
    out.write_all(b"        }\n")?;
    out.write_all(b"    }\n")?;

    out.write_all(b"}\n")?;
    out.flush()?;

    Ok(())
}

fn main() {
    if let Err(err) = generate_trap_codes() {
        eprintln!("problem generating trap_codes.rs: {err}");
        process::exit(1);
    }
}
