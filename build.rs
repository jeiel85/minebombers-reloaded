fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}/lib", dir);

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("res/minebombers.ico");
        res.set("FileDescription", "Mine Bombers (Native PC Edition)");
        res.set("ProductName", "Mine Bombers");
        res.set("OriginalFilename", "MineBombers.exe");
        res.set("LegalCopyright", "1995-1996 Sami Lehtinen & Skaven");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }
    }
}
