#[cfg(target_env = "gnu")]
extern crate libc;

#[cfg(target_env = "gnu")]
pub fn force_memory_release(min_allocated_memory: usize) {
    use log::debug;

    unsafe {
        let result = libc::malloc_trim(min_allocated_memory * 1024 * 1024);
        if result == 0 {
            debug!("Memory release failed");
        } else {
            debug!("Memory released successfully: {}", result);
        }
    }
}

// 非GNU环境的空实现
#[cfg(not(target_env = "gnu"))]
pub fn force_memory_release(_min_allocated_memory: usize) {
    use log::debug;
    debug!("Memory release is not supported on this platform");
}
    