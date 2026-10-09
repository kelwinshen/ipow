// `declare_program!` reads the IDLs at compile time without telling cargo:
// rebuild when one changes.
fn main() {
    println!("cargo:rerun-if-changed=idls");
}
