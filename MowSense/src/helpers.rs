fn print_memory_info() {
    unsafe {
        let free_heap = esp_get_free_heap_size();
        let min_heap = esp_get_minimum_free_heap_size();
        let free_internal = heap_caps_get_free_size(0); // MALLOC_CAP_8BIT | MALLOC_CAP_INTERNAL
        let free_psram = heap_caps_get_free_size(0x1000_0000); // MALLOC_CAP_SPIRAM

        println!("Free heap: {} bytes", free_heap);
        println!("Min free heap: {} bytes", min_heap);
        println!("Free internal heap: {} bytes", free_internal);
        println!("Free PSRAM: {} bytes", free_psram);
    }
}