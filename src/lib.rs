extern crate ironplc_dsl;
extern crate ironplc_parser;

pub fn main() {
    ironplc_parser::parse_program("");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/test");
        path.push(name);

        fs::read_to_string(path).expect("Unable to read file")
    }

    #[test]
    fn first_steps() {
        let src = read_resource("parts.st");
        assert_eq!(ironplc_parser::parse_program(src.as_str()), Ok(()))
    }
}
