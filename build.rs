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
        res.set("LegalCopyright", "Unofficial engine port. Mine Bombers is (c) Skitso Productions.");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }

        copy_runtime_dlls(&dir);
    }
}

/// Put the SDL2 DLLs next to the freshly built exe.
///
/// `lib/` holds both the import libraries the linker needs and the DLLs the exe loads at run time,
/// but cargo only produces the exe - so `target/<profile>/MineBombers.exe` on its own cannot start,
/// and Windows says so with a "SDL2.dll is missing" dialog rather than anything that points at the
/// cause. Running it from the repository root happens to work (the DLLs sit there too, and the
/// working directory is on the search path), which is exactly what makes this confusing: the same
/// binary works or fails depending on where it is launched from.
///
/// `scripts/package_windows.ps1` already copies these into the release zip; this does the same for
/// the build tree, so a plain `cargo build` produces a directory that runs anywhere.
#[cfg(target_os = "windows")]
fn copy_runtime_dlls(manifest_dir: &str) {
    use std::path::Path;

    let lib_dir = Path::new(manifest_dir).join("lib");
    println!("cargo:rerun-if-changed={}", lib_dir.display());

    // OUT_DIR is target/<profile>/build/<pkg>-<hash>/out, so the exe's directory is four components up.
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let Some(exe_dir) = Path::new(&out_dir).ancestors().nth(3) else {
        println!("cargo:warning=cannot locate the build output directory from OUT_DIR={out_dir}");
        return;
    };

    let entries = match std::fs::read_dir(&lib_dir) {
        Ok(entries) => entries,
        Err(e) => {
            println!("cargo:warning=cannot read {}: {e}", lib_dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let source = entry.path();
        if source.extension().and_then(|ext| ext.to_str()) != Some("dll") {
            continue;
        }
        let Some(name) = source.file_name() else { continue };
        let target = exe_dir.join(name);

        // Watch the copy as well as the original. Cargo treats a path it is told to watch as changed
        // when the file is missing, so deleting one of these copies - or never having had it, in a
        // build tree from before this script existed - re-runs this and puts it back. Without this
        // the script only re-runs when `lib/` itself changes, and a build tree whose DLLs went away
        // stays broken until `cargo clean`.
        println!("cargo:rerun-if-changed={}", target.display());

        // Skip an identical copy: the DLLs never change between builds, and rewriting them would
        // fail while a previously built copy of the game is running.
        if let (Ok(from), Ok(to)) = (source.metadata(), target.metadata()) {
            if from.len() == to.len() {
                continue;
            }
        }

        if let Err(e) = std::fs::copy(&source, &target) {
            // Not fatal: the exe still builds, it just may not start from this directory. A running
            // instance holding the old DLL open is the usual reason.
            println!("cargo:warning=could not copy {} to {}: {e}", source.display(), target.display());
        }
    }
}
