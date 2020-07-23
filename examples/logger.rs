use libsystemd::logging;

fn main() {
    logging::journal_print(logging::Priority::Alert, "LOG FROM RUST LOGGER").expect("Error logging");
}
