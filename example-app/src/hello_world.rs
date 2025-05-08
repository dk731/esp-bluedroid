pub fn main() -> anyhow::Result<()> {
    esp_bluedroid::svc::sys::link_patches();
    esp_bluedroid::svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    Ok(())
}
