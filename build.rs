// Simple build script to avoid cmake dependency issues
fn main() {
    // Set environment variables to disable problematic features
    println!("cargo:rustc-env=AWS_LC_SYS_NO_ASM=1");
    println!("cargo:rustc-env=AWS_LC_SYS_STATIC=1");
}