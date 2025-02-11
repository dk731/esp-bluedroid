use std::sync::Arc;
use std::u32;

use anyhow;
use esp_bluedroid::ble;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::prelude::Peripherals;

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let Ok(peripherals) = Peripherals::take() else {
        log::error!("Failed to take peripherals");
        return;
    };

    let Ok(ble) = ble::Ble::new(peripherals.modem) else {
        log::error!("Failed to create BLE instance");
        return;
    };

    // log::info!("Hello, world! 123");

    if let Err(err) = ble.gap.start_advertising() {
        log::error!("Failed to start advertising: {:?}", err);
        return;
    }

    log::info!("Advertising started");

    // let (tx, rx) = std::sync::mpsc::channel::<String>();

    // std::thread::spawn(move || loop {
    //     let Ok(msg) = rx.recv() else {
    //         log::error!("Failed to receive message from channel");
    //         return;
    //     };
    //     log::info!("Received message: {}", msg);
    // });

    // let tx = Arc::new(tx);
    // for i in 0..5 {
    //     let thread_tx = tx.clone();

    //     std::thread::spawn(move || loop {
    //         let sleep_duration = unsafe { esp_idf_svc::sys::esp_random() };
    //         let sleep_duration = sleep_duration as f32 / u32::MAX as f32;
    //         let sleep_duration = sleep_duration * 10.0f32;

    //         std::thread::sleep(std::time::Duration::from_secs_f32(sleep_duration));
    //         log::info!(
    //             "Thread {} slept for {} seconds, sending a message",
    //             i,
    //             sleep_duration
    //         );

    //         if let Err(err) = thread_tx.send(format!("Hello from thread {}", i)) {
    //             log::error!("Failed to send message from thread {}: {:?}", i, err);
    //         } else {
    //             log::info!("Message sent from thread {}", i);
    //         }
    //     });
    // }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        log::info!("Still running...");
    }
}
