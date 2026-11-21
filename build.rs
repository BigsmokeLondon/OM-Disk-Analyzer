fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/om.ico");
        res.set("ProductName", "OM Disk Analyzer Professional");
        res.set("FileDescription", "OM Disk Analyzer Professional");
        res.set("CompanyName", "OM Industries Inc");
        res.set("LegalCopyright", "© OM Industries Inc. All rights reserved.");
        let _ = res.compile();
    }
    println!("cargo:rerun-if-changed=assets/om.ico");
}
