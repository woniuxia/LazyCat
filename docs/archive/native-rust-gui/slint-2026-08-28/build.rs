fn main() {
    // ComponentContainer is deliberately experimental in Slint 1.17.1. Keep the
    // opt-in local to this disposable prototype so the risk remains explicit.
    unsafe { std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1") };
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");
}
