# rs-refresh-ddns

一个基于Rust的DDNS更新工具，支持定时更新DuckDNS的IPv6地址。

## 功能特性

- 定时执行任务（可配置cron表达式）
- 支持两种IPv6地址获取方式：
  - 通过外部服务（如 `curl 6.ipw.cn`）
  - 直接获取本地IPv6地址
- 自动更新DuckDNS记录
- 完整的日志记录

## 配置

可以通过环境变量或配置文件进行配置：
```shell
# Cron表达式，定义任务执行时间，默认为每5分钟执行一次
export CRON="0 */15 * * * *"

# IPv6获取方式，可选值：external（通过外部服务获取）, local（获取本地地址）
export IPV6_METHOD="external"
export HOSTS_INTERFACE="eth0"
export SHELL_COMMAND="ip -6 addr show wlp3s0 | grep 'inet6.*::.*scope global' | awk '{print $2}' | cut -d'/' -f1"

# 外部IPv6获取服务地址
export IP_SERVICE_URL="https://6.ipw.cn"

# DuckDNS域名（不包含.duckdns.org）
export DUCKDNS_DOMAIN="your-domain"

# DuckDNS令牌
export DUCKDNS_TOKEN="your-token"
```


### 配置文件方式

创建 `config.toml` 文件：
```
# Cron表达式，定义任务执行时间，默认为每5分钟执行一次
cron = "0 */15 * * * *"

# IPv6获取方式，可选值：external（通过外部服务获取）, local（获取本地地址）
ipv6_method = "external"
hosts_interface = "eth0"
shell_command = "ip -6 addr show wlp3s0 | grep 'inet6.*::.*scope global' | awk '{print $2}' | cut -d'/' -f1"

# 外部IPv6获取服务地址
ip_service_url = "https://6.ipw.cn"

# DuckDNS域名（不包含.duckdns.org）
duckdns_domain = "your-domain"

# DuckDNS令牌
duckdns_token = "your-token"
```

