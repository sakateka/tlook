# Term Look
A tool for fast charting in the terminal.

# How to
You have metrics in the form
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
![2024-05-27T15:05:35,263416706+03:00](https://github.com/sakateka/tlook/assets/2256154/54a1dcee-e98a-4c40-96cc-997cad92b440)

Monitor memory usage
```
while sleep 0.5; do free -m|rg 'Mem:\s+(\d+)\s+(\d+)\s+(\d+)' -or 'total=$1;used=$2;free=$3'; done|tlook
```
or
```
while sleep 0.2; do rg '(MemFree|Active|AnonPages|Dirty):\s+(\d+)' -or '$1=$2' /proc/meminfo; done |tlook
```

# Demo

`python samples/graph-on-screenshot.py |cargo run --release`

Press `?` for help.

[![asciicast](https://asciinema.org/a/AzSyFitAXabbis29pVNx9uTCe.svg)](https://asciinema.org/a/AzSyFitAXabbis29pVNx9uTCe)
