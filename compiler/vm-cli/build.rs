fn main() {
    println!(
        "cargo:rustc-env=BUILD_OPT_LEVEL={}",
        std::env::var("OPT_LEVEL").unwrap()
    );
}
