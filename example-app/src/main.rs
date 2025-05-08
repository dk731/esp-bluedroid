fn main() {
    #[cfg(feature = "esp-bluedroid")]
    example_app::esp_bluedroid_example::main().unwrap();

    #[cfg(feature = "esp-idf")]
    example_app::esp_idf_example::example::main().unwrap();

    #[cfg(feature = "esp-hello-world")]
    example_app::hello_world::main().unwrap();
}
