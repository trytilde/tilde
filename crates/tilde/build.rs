use std::{env, fs, path::Path};

/// Embed `migrations/*.sql` as an ordered table so the runtime migrator needs no
/// filesystem access. File names are `<version>_<description>.sql`, matching the
/// `_tilde_migrations` history table that existing databases already carry.
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=../../web/provider-dist");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .expect("migrations directory")
        .map(|entry| entry.expect("migration entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    let mut out = String::from("&[\n");
    let mut versions = std::collections::BTreeSet::new();
    for path in files {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let (version, description) = name
            .split_once('_')
            .unwrap_or_else(|| panic!("migration {name} must be <version>_<description>.sql"));
        let version: i64 = version
            .parse()
            .unwrap_or_else(|_| panic!("migration {name} has a non-numeric version"));
        assert!(
            versions.insert(version),
            "duplicate migration version {version}: {name}"
        );
        let sql = path.canonicalize().unwrap();
        out.push_str(&format!(
            "    Migration {{ version: {version}, description: {description:?}, sql: include_str!({:?}) }},\n",
            sql.to_str().unwrap()
        ));
    }
    out.push_str("]\n");
    let dest = Path::new(&env::var("OUT_DIR").unwrap()).join("migrations.rs");
    fs::write(dest, out).unwrap();
}
