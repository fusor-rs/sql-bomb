use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "../../assets/brand";
    let destination = "public/brand";
    fs::create_dir_all(destination)?;
    for name in ["sql-bomb.svg", "sql-bomb-horizontal.svg"] {
        println!("cargo:rerun-if-changed={source}/{name}");
        fs::copy(format!("{source}/{name}"), format!("{destination}/{name}"))?;
    }
    let installer = "../../install.sh";
    println!("cargo:rerun-if-changed={installer}");
    fs::copy(installer, "public/install.sh")?;
    fusor_build::compile_app()
}
