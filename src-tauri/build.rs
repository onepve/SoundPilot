fn main() {
    // lib.rs 使用 tauri 的 mobile_entry_point 属性宏（`cfg(mobile)`），
    // 声明该自定义 cfg 以消除 unexpected_cfgs 告警。
    println!("cargo::rustc-check-cfg=cfg(mobile)");
    tauri_build::build()
}
