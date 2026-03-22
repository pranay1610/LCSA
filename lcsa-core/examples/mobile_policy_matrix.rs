use lcsa_core::{MobilePolicy, Platform, SignalType};

fn main() {
    print_row(Platform::Android, "9");
    print_row(Platform::Android, "10");
    print_row(Platform::Android, "14");
    print_row(Platform::Ios, "15");
    print_row(Platform::Ios, "16");
    print_row(Platform::Ios, "18");
}

fn print_row(platform: Platform, version: &str) {
    let Some(policy) = MobilePolicy::for_platform(platform, Some(version)) else {
        return;
    };

    println!(
        "{platform:?} {version}: clipboard={:?} selection={:?} focus={:?}",
        policy.signal_delivery(SignalType::Clipboard),
        policy.signal_delivery(SignalType::Selection),
        policy.signal_delivery(SignalType::Focus),
    );
}
