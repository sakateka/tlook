#!/bin/bash

while sleep 0.5; do
    free -m|rg 'Mem:\s+(\d+)\s+(\d+)\s+(\d+)' -or 'total=$1;used=$2;free=$3'
done
