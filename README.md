# Term Look
A tool for fast charting in the terminal.

# How to
You have a metric in the form
```
key1=value1
key2=value2
key3=value3;key4=value4
...
```
You want to display them in the terminal and zoom in/out in the history. You are in the right place.

# Examples
Measure and plot ping timings
```
bash -c 'for ip in cloudflare.com lua.org yahoo.com; do ping -i 0.5 $ip  |rg --line-buffered "time=([\d.]+)" -or "ping $ip time=\$1;" & done; wait'|tlook
```
Monitor memory usage
```
while sleep 0.5; do free -m|rg 'Mem:\s+(\d+)\s+(\d+)\s+(\d+)' -or 'total=$1;used=$2;free=$3'; done|tlook
```
or
```
while sleep 0.2; do rg '(MemFree|Active|AnonPages|Dirty):\s+(\d+)' -or '$1=$2' /proc/meminfo; done |tlook
```

# Screenshot
![2024-05-19T12:46:18,396730332+03:00](https://github.com/sakateka/tlook/assets/2256154/640684cc-a456-4008-9b04-58fdd54bb927)
