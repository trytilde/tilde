fn main() {
    // Recompile embedded migrations/assets when their source changes; do not run codegen or Node here.
    println!("cargo:rerun-if-changed=../../migrations");
    println!("cargo:rerun-if-changed=../../queries");
    println!("cargo:rerun-if-changed=../../.sqlx");
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=../../web/provider-dist");
}
