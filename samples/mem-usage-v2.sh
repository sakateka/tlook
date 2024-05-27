#!/bin/bash

while sleep 0.2; do
    rg '(MemFree|Active|AnonPages|Dirty):\s+(\d+)' -or '$1=$2' /proc/meminfo;
done
