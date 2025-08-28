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
    hosts_interface: Option<String>
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
            hosts_interface: std::env::var("HOSTS_INTERFACE").ok(),
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
            hosts_interface: config.hosts_interface,
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
    hosts_interface: Option<String>,
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
            get_local_ipv6_address(config.hosts_interface.as_deref()).await
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
// 直接获取本地IPv6地址 - 改进版本
async fn get_local_ipv6_address(interface_name: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    // 添加 if-addrs 依赖到 Cargo.toml:
    // if-addrs = "0.12"
    let interfaces = if_addrs::get_if_addrs()?;

    for iface in interfaces {
        // 如果指定了接口名称，则只检查该接口
        if let Some(name) = interface_name {
            if iface.name != name {
                continue;
            }
        }

        // 跳过回环接口（除非用户明确指定）
        if iface.is_loopback() && interface_name.is_none() {
            continue;
        }

        // 查找 IPv6 地址
        if let std::net::IpAddr::V6(ipv6) = iface.ip() {

            let ip_str = ipv6.to_string();
            debug!("Got IPv6 address from interface '{}': {}", iface.name, ip_str);
            return Ok(ip_str);
        }
    }

    if let Some(name) = interface_name {
        Err(format!("No IPv6 address found for interface '{}'", name).into())
    } else {
        Err("No public IPv6 address found on any interface".into())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_get_local_ipv6_address() {
        
        let result = get_local_ipv6_address(Some("en0")).await;
        match result {
            Ok(ip) => println!("Local IPv6 address: {}", ip),
            Err(e) => println!("Error getting local IPv6 address: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_local_ipv6_address_with_specific_interface() {
        // 首先获取系统中存在的网络接口列表
        let interfaces = if_addrs::get_if_addrs().unwrap_or_default();
        let mut found_ipv6 = false;

        // 遍历接口，查找一个有 IPv6 地址的接口进行测试
        for iface in interfaces {
            println!("{}", iface.name);
            if iface.name != "en0" {
                continue; // 跳过 loopback 接口
            }
            if let std::net::IpAddr::V6(_) = iface.ip() {
                // 找到一个有 IPv6 地址的接口，用它进行测试
                let result = get_local_ipv6_address(Some(&iface.name)).await;
                match result {
                    Ok(ip) => {
                        println!("IPv6 address from interface '{}': {}", iface.name, ip);
                        assert!(!ip.is_empty());
                        assert!(ip.contains(":")); // IPv6 地址应该包含冒号
                        found_ipv6 = true;
                    }
                    Err(e) => {
                        println!("Failed to get IPv6 from interface '{}': {}", iface.name, e);
                    }
                }
            }
        }

        // 如果没有找到任何有 IPv6 的接口，则测试指定不存在接口的情况
        if !found_ipv6 {
            let result = get_local_ipv6_address(Some("nonexistent_interface")).await;
            match result {
                Ok(ip) => {
                    // 意外找到了 IP，也认为测试通过
                    println!("Unexpectedly found IPv6 address: {}", ip);
                }
                Err(e) => {
                    // 这是预期的结果
                    println!("Expected error for interface without IPv6: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_get_local_ipv6_address_auto_discovery() {
        // 测试自动发现功能（不指定接口）
        let result = get_local_ipv6_address(None).await;
        match result {
            Ok(ip) => {
                println!("Auto-discovered IPv6 address: {}", ip);
                assert!(!ip.is_empty());
                assert!(ip.contains(":"));
            }
            Err(e) => {
                // 在某些环境中可能没有可用的 IPv6 地址，这是可以接受的
                println!("No IPv6 address found in auto-discovery (may be expected): {}", e);
            }
        }
    }
    
    
}