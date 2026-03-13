#!/bin/bash
#Usage: ./log.sh [binary name]
#ex
#./log.sh target/thumbv8m.main-none-eabihf/debug/scan_i2c

PORT=$(ls /dev/tty.usbmodem* 2>/dev/null | head -1)
ELF=$1

if [ -z "$PORT" ]; then
  echo "No device found"
  exit 1
fi

if [ -z "$ELF" ]; then
  echo "Usage: $0 <elf-file>"
  exit 1
fi

FIFO=$(mktemp -t defmt)
rm "$FIFO"
mkfifo "$FIFO"

python3 -u -c "
import serial, sys, time
s = serial.Serial('$PORT', 115200, dsrdtr=True, rtscts=False, timeout=0)
s.dtr = True
s.rts = False
time.sleep(0.1)
with open('$FIFO', 'wb', buffering=0) as f:
    while True:
        data = s.read(4096)
        if data:
            f.write(data)
            f.flush()
" &

defmt-print -e "$ELF" < "$FIFO"