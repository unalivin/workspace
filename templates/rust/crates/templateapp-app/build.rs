fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    println!("cargo:rerun-if-changed=../../LICENSE");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let license = match std::fs::read_to_string("../../LICENSE") {
        Ok(text) => text,
        Err(problem) => panic!("the license file could not be read: {problem}"),
    };
    let Some(copyright) = license.lines().find(|line| line.starts_with("Copyright (c) ")) else {
        panic!("the license file has no copyright line");
    };
    let holder = copyright.trim_start_matches("Copyright (c) ").split_once(' ').map(|(_, holder)| holder).unwrap_or_default();
    if holder.is_empty() {
        panic!("the copyright line of the license file names no holder");
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/icon.ico");
    resource.set("ProductName", "Template App");
    resource.set("FileDescription", "Template App");
    resource.set("CompanyName", holder);
    resource.set("LegalCopyright", copyright);
    if let Err(problem) = resource.compile() {
        panic!("the Windows resource could not be compiled: {problem}");
    }
}
