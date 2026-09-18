fn main() {
    let version = rustc_version::version().expect("rustc version is available");
    println!("cargo:rustc-env=TYPESAFE_RUSTC_VERSION={version}");
}
