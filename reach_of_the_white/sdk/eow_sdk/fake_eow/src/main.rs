fn main() {
    if let Err(error) = fake_eow::run() {
        eprintln!("[FATAL] {error}");
        std::process::exit(1);
    }
}
