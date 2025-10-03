use std::env;

use sdk::tracing::debug;

pub struct Envs {
    /**
     * 包消费者数量
     *
     * 用于处理包的线程数量
     * 环境变量: PHLOW_PACKAGE_CONSUMERS_COUNT
     * 默认值: 10
     */
    pub package_consumer_count: i32,
    /**
     * 最小分配内存(MB)
     *
     * 进程将分配的最小内存量
     * 环境变量: PHLOW_MIN_ALLOCATED_MEMORY_MB
     * 默认值: 10
     */
    #[cfg(target_env = "gnu")]
    pub min_allocated_memory: usize,
    /**
     * 启用垃圾回收
     *
     * 控制是否启用垃圾回收
     * 环境变量: PHLOW_GARBAGE_COLLECTION_ENABLED
     * 默认值: true
     */
    #[cfg(target_env = "gnu")]
    pub garbage_collection: bool,
    /**
     * 垃圾回收间隔(秒)
     *
     * 执行垃圾回收的时间间隔
     * 环境变量: PHLOW_GARBAGE_COLLECTION_INTERVAL_SECONDS
     * 默认值: 60
     */
    #[cfg(target_env = "gnu")]
    pub garbage_collection_interval: u64,

    /**
     * 默认包仓库URL
     *
     * 用于获取包的默认仓库URL
     * 环境变量: PHLOW_DEFAULT_PACKAGE_REPOSITORY_URL
     * 默认值: phlowdotdev/phlow-packages
     */
    pub default_package_repository_url: String,
    /**
     * 默认phlow文件主入口
     *
     * 用于运行phlow文件的默认主入口文件
     * 环境变量: PHLOW_MAIN
     * 默认值: None
     */
    pub main: String,
}

impl Envs {
    pub fn load() -> Self {
        let package_consumer_count = env::var("PHLOW_PACKAGE_CONSUMERS_COUNT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10) as i32;
        #[cfg(target_env = "gnu")]
        let min_allocated_memory = env::var("PHLOW_MIN_ALLOCATED_MEMORY_MB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);

        #[cfg(target_env = "gnu")]
        let garbage_collection = env::var("PHLOW_GARBAGE_COLLECTION_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);
        #[cfg(target_env = "gnu")]
        let garbage_collection_interval = env::var("PHLOW_GARBAGE_COLLECTION_INTERVAL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);

        let default_package_repository_url = match env::var("PHLOW_DEFAULT_PACKAGE_REPOSITORY_URL")
        {
            Ok(repo) => repo,
            Err(_) => "phlowdotdev/phlow-packages".to_string(),
        };

        let main = env::var("PHLOW_MAIN").unwrap_or(".".to_string());

        debug!("PHLOW_PACKAGE_CONSUMERS_COUNT = {}", package_consumer_count);
        #[cfg(target_env = "gnu")]
        debug!("PHLOW_MIN_ALLOCATED_MEMORY_MB = {}", min_allocated_memory);
        #[cfg(target_env = "gnu")]
        debug!("PHLOW_GARBAGE_COLLECTION_ENABLED = {}", garbage_collection);
        #[cfg(target_env = "gnu")]
        debug!(
            "PHLOW_GARBAGE_COLLECTION_INTERVAL_SECONDS = {}",
            garbage_collection_interval
        );
        debug!(
            "PHLOW_DEFAULT_PACKAGE_REPOSITORY_URL = {}",
            default_package_repository_url
        );

        Self {
            package_consumer_count,
            #[cfg(target_env = "gnu")]
            min_allocated_memory,
            #[cfg(target_env = "gnu")]
            garbage_collection,
            #[cfg(target_env = "gnu")]
            garbage_collection_interval,
            default_package_repository_url,
            main,
        }
    }
}
    