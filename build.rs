fn main() {
    nasm_rs::compile_library("load_gdt", &["src/gdt/load_gdt.asm"]).unwrap();
}
