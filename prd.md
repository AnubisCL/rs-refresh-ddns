## 需求

- 1.定时执行某个任务(可配置执行周期cron)
- 2.外部接口或者直接获取本地ipv6地址（两种方式都要支持）
  - a.`curl 6.ipw.cn` （可配置）
  - b.直接获取本地的ipv6地址
- 3.调用外部接口用于更新ddns
  - a.`curl --location 'https://www.duckdns.org/update?domains=[配置]&token=[配置]&ipv6=[获取]&verbose=true'`
- 4.日志打印