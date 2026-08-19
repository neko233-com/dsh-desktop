fn main() {
    println!("cargo:rerun-if-changed=packaging/windows/dsh-desktop.ico");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("packaging/windows/dsh-desktop.ico");
        resource
            .compile()
            .expect("compile Windows application resources");
    }
}
