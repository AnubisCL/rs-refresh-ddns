use std::time::Duration;
use tokio::time;
use tokio_cron_scheduler::{Job, JobScheduler};
use reqwest::Client;
use tracing::{info, error, debug};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    info!("Starting DDNS updater");
    
    // 从环境变量或配置文件读取配置
    let config = Config::from_env();
    
    // 创建调度器
    let scheduler = JobScheduler::new().await?;
    
    // 克隆需要的数据，避免借用冲突
    let cron_expr = config.cron.clone();
    
    // 创建定时任务
    let job = Job::new_async(cron_expr.as_str(), move |_uuid, _l| {
        let config_clone = config.clone();
        Box::pin(async move {
            match update_ddns(&config_clone).await {
                Ok(_) => info!("DDNS update completed successfully"),
                Err(e) => error!("Failed to update DDNS: {}", e),
            }
        })
    })?;
    
    scheduler.add(job).await?;
    
    scheduler.start().await?;
    
    // 保持程序运行
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
}


// 配置结构体
#[derive(Clone, Debug)]
struct Config {
    cron: String,
    ipv6_method: String,
    ip_service_url: String,
    duckdns_domain: String,
    duckdns_token: String,
}

impl Config {
    fn from_env() -> Self {
        // 尝试从配置文件读取
        if let Ok(config) = Self::from_file("config.toml") {
            return config;
        }

        // 如果配置文件不存在，则从环境变量读取
        Self {
            cron: std::env::var("CRON").unwrap_or_else(|_| "0 */5 * * * *".to_string()), // 默认每5分钟执行一次
            ipv6_method: std::env::var("IPV6_METHOD").unwrap_or_else(|_| "external".to_string()), // 默认使用外部服务
            ip_service_url: std::env::var("IP_SERVICE_URL").unwrap_or_else(|_| "https://6.ipw.cn".to_string()),
            duckdns_domain: std::env::var("DUCKDNS_DOMAIN").expect("DUCKDNS_DOMAIN must be set"),
            duckdns_token: std::env::var("DUCKDNS_TOKEN").expect("DUCKDNS_TOKEN must be set"),
        }
    }

    fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        use std::fs;
        let contents = fs::read_to_string(path)?; // 这里是安全的，因为 path 是 &str
        let config: ConfigFile = toml::from_str(&contents)?;

        Ok(Self {
            cron: config.cron.unwrap_or_else(|| "0 */5 * * * *".to_string()),
            ipv6_method: config.ipv6_method.unwrap_or_else(|| "external".to_string()),
            ip_service_url: config.ip_service_url.unwrap_or_else(|| "https://6.ipw.cn".to_string()),
            duckdns_domain: config.duckdns_domain.ok_or("DUCKDNS_DOMAIN must be set")?,
            duckdns_token: config.duckdns_token.ok_or("DUCKDNS_TOKEN must be set")?,
        })
    }
}

#[derive(serde::Deserialize)]
struct ConfigFile {
    cron: Option<String>,
    ipv6_method: Option<String>,
    ip_service_url: Option<String>,
    duckdns_domain: Option<String>,
    duckdns_token: Option<String>,
}

// 更新DDNS的主函数
async fn update_ddns(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting DDNS update process");
    
    // 获取IPv6地址
    let ipv6 = get_ipv6_address(config).await?;
    info!("Current IPv6 address: {}", ipv6);
    
    // 调用DuckDNS更新接口
    update_duckdns(config, &ipv6).await?;
    
    Ok(())
}

// 获取IPv6地址
async fn get_ipv6_address(config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    match config.ipv6_method.as_str() {
        "external" => {
            // 通过外部服务获取IPv6地址
            get_ipv6_from_external_service(&config.ip_service_url).await
        },
        "local" => {
            // 直接获取本地IPv6地址
            get_local_ipv6_address().await
        },
        _ => {
            error!("Invalid IPV6_METHOD: {}. Using external service.", config.ipv6_method);
            get_ipv6_from_external_service(&config.ip_service_url).await
        }
    }
}

// 通过外部服务获取IPv6地址
async fn get_ipv6_from_external_service(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    debug!("Fetching IPv6 from external service: {}", url);
    
    let client = Client::new();
    let response = client.get(url).send().await?;
    let ip = response.text().await?;
    
    debug!("Got IPv6 from external service: {}", ip);
    Ok(ip)
}

// 直接获取本地IPv6地址
async fn get_local_ipv6_address() -> Result<String, Box<dyn std::error::Error>> {
    debug!("Fetching local IPv6 address");
    
    let addrs = tokio::net::lookup_host("localhost:0").await?;
    for addr in addrs {
        if addr.is_ipv6() {
            let ip = addr.ip().to_string();
            debug!("Got local IPv6 address: {}", ip);
            return Ok(ip);
        }
    }
    
    Err("No local IPv6 address found".into())
}

// 更新DuckDNS
async fn update_duckdns(config: &Config, ipv6: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!(
        "https://www.duckdns.org/update?domains={}&token={}&ipv6={}&verbose=true",
        config.duckdns_domain,
        config.duckdns_token,
        ipv6
    );
    
    info!("Updating DuckDNS with URL: {}", url);
    
    let client = Client::new();
    let response = client.get(&url).send().await?;
    
    let status = response.status();
    let body = response.text().await?;
    
    info!("DuckDNS update response - Status: {}, Body: {}", status, body);
    
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("DuckDNS update failed with status: {}", status).into())
    }
}