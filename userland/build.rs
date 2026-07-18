fn main() {
    println!("cargo:rustc-link-arg=-T{}/linker.ld", env!("CARGO_MANIFEST_DIR"));
    println!("cargo:rustc-link-arg=-no-pie");
}