use lcsa_core::{ContextApi, SignalType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = ContextApi::new()?;

    println!(
        "device={} platform={:?}",
        api.device_context().device_id,
        api.device_context().platform
    );

    for signal_type in [
        SignalType::Clipboard,
        SignalType::Selection,
        SignalType::Focus,
    ] {
        println!("{signal_type:?}: {:?}", api.signal_support(signal_type));
    }

    println!("supported={:?}", api.supported_signals());
    Ok(())
}
