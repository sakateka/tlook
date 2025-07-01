# 📊 tlook - Terminal Metrics Visualizer

Real-time charting of metrics in your terminal. Monitor anything that produces numbers!

## 🚀 Quick Start

```bash
# Monitor CPU usage every second
tlook -c "top -bn1 | grep 'Cpu(s)' | awk '{print \"cpu_usage=\" \$2}' | tr -d '%us,'"

# Watch memory usage
tlook -c "free | awk '/^Mem:/ {printf \"memory_used=%.1f\\n\", \$3/\$2*100}'"

# Monitor disk I/O
tlook -p "iostat -x 1 | stdbuf -o0 awk '/^nvme/ {print \$1 \"_util=\" \$10}'"
```

## 💡 How It Works

**tlook** expects metrics in simple `name=value` format:
```
cpu=75.5
memory=60.2
disk_io=23.1;network_rx=156.7
```

## 📖 Usage

```bash
tlook [OPTIONS]
```

### Command Types

| Flag | Type | Description | Best For |
|------|------|-------------|----------|
| `-p` | **Process** | Long-running commands that continuously output data | `ping`, `iostat`, `top`, log tails |
| `-c` | **Command** | Short commands that run repeatedly | `free`, `df`, `uptime`, quick checks |

### Options
- `--interval <SECONDS>` - How often to repeat commands (default: 1)
- `--stdin` - Read from stdin pipe
- `-f <FILE>` - Read from named pipe (FIFO) for real-time streaming

## 🎯 Real-World Examples

### 🌐 Network Monitoring
```bash
# Ping multiple hosts
tlook \
  -p "ping google.com | grep --line-buffered -o 'time=[0-9.]*' | sed -u 's/time=/google=/'" \
  -p "ping github.com | grep --line-buffered -o 'time=[0-9.]*' | sed -u 's/time=/github=/'"

# Network traffic
tlook -c "cat /proc/net/dev | awk '/wlp0/ {print \"rx_mb=\" \$2/1024/1024 \";tx_mb=\" \$10/1024/1024}'" --interval 2
```

### 💾 System Resources
```bash
# Complete system overview
tlook \
  -c "free | awk '/^Mem:/ {printf \"memory=%.1f\\n\", \$3/\$2*100}'" \
  -c "df -h / | awk 'NR==2 {gsub(/%/, \"\"); print \"disk=\" \$5}'" \
  -c "uptime | awk '{print \"load=\" \$(NF-2)}' | tr -d ','"
```

### 🐳 Docker Containers
```bash
# Monitor container stats
tlook -c "docker stats --no-stream --format 'table {{.Container}}\t{{.CPUPerc}}\t{{.MemPerc}}' | awk 'NR>1 {gsub(/%/, \"\"); print \$1 \"_cpu=\" \$2 \";\" \$1 \"_mem=\" \$3}'"
```

### 📊 Custom Metrics
```bash
# Using named pipe for real-time app metrics
mkfifo /tmp/metrics
your_app_logger > /tmp/metrics &
tlook -f /tmp/metrics

# Database connections
tlook -c "mysql -e 'SHOW STATUS LIKE \"Threads_connected\"' | awk 'NR==2 {print \"db_connections=\" \$2}'" --interval 5
```

## ⌨️ Controls

| Key | Action | Key | Action |
|-----|--------|-----|--------|
| `?` | Show help | `q` | Quit |
| `w/W` | Zoom time window | `h/H` | Adjust history |
| `a` | Toggle axis labels | `l` | Toggle legend |
| `s` | Scale mode (linear/asinh) | `c` | Toggle cursor |
| `←/→` | Move cursor | `Space` | Pause/resume |

## 🎬 Demo

```bash
python samples/graph-on-screenshot.py | cargo run --release -- --stdin
```

[![asciicast](https://asciinema.org/a/AzSyFitAXabbis29pVNx9uTCe.svg)](https://asciinema.org/a/AzSyFitAXabbis29pVNx9uTCe)