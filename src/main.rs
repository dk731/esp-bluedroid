use esp_bluedroid::example;

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    // esp_idf_svc::sys::link_patches();

    // // Bind the log crate to the ESP Logging facilities
    // esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world! 123");

    let result = example::main();
    if let Err(e) = result {
        log::error!("Error: {:?}", e);
    }

    // let mut i = 0;
    // loop {
    //     i += 1;
    //     log::info!("Hello, world! {}", i);
    //     std::thread::sleep(std::time::Duration::from_secs(1));
    // }
}
