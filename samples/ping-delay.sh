#!/bin/bash

bash -c '
for ip in cloudflare.com lua.org yahoo.com ya.ru google.com; do
    ping -i 0.5 $ip  |rg --line-buffered "time=([\d.]+)" -or "ping $ip time=\$1;" &
done;
wait'
