fn main() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let icons_list = manifest_dir.join("icons.list");
    let generate_script = manifest_dir.join("scripts/generate-icons.mjs");
    let generated_icons = manifest_dir.join("ui/icons/icons.generated.slint");

    println!("cargo:rerun-if-changed={}", icons_list.display());
    println!("cargo:rerun-if-changed={}", generate_script.display());

    if icons_list.exists() && generate_script.exists() {
        let status = std::process::Command::new("node")
            .arg(&generate_script)
            .current_dir(manifest_dir)
            .status();

        match status {
            Ok(exit) if !exit.success() => {
                eprintln!(
                    "warning: icon generation failed; using committed ui/icons/icons.generated.slint"
                );
            }
            Err(err) => {
                eprintln!(
                    "warning: could not run node for icon generation ({err}); using committed generated icons"
                );
            }
            _ => {}
        }
    }

    if !generated_icons.exists() {
        panic!(
            "missing {} — run `npm run generate-icons` in browser-ui/",
            generated_icons.display()
        );
    }

    slint_build::compile("ui/appwindow.slint").unwrap();
}
