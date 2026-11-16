fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/icon.ico");
    resource.set("ProductName", "Template App");
    resource.set("FileDescription", "Template App");
    if let Err(problem) = resource.compile() {
        panic!("the Windows icon resource could not be compiled: {problem}");
    }
}
