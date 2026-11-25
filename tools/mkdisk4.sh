#!/bin/bash
# usage: mkdisk4.sh out.img apfs.raw payload.img [helper.img]
# gpt disk: esp, [helper hfs+ (patch stage), only when given], payload hfs+ (gpu driver files), apfs recovery container
set -e
OUT=$1
APFS=$2
PAY=$3
HELP=$4
PS=$(stat -c %s "$PAY")
AS=$(stat -c %s "$APFS")
HS=0
[ -n "$HELP" ] && HS=$(stat -c %s "$HELP")
ESP_MB=${ESP_MB:-200}
TOTAL=$(( (ESP_MB * 1024 * 1024) + HS + PS + AS + 12 * 1024 * 1024 ))
truncate -s $TOTAL "$OUT"
sgdisk -o "$OUT" >/dev/null
sgdisk -n 1:2048:+${ESP_MB}M -t 1:EF00 -c 1:ESP "$OUT" >/dev/null
N=2
if [ -n "$HELP" ]; then
  sgdisk -n $N:0:+$((HS / 1048576 + 1))M -t $N:AF00 -c $N:anathema-helper "$OUT" >/dev/null
  HN=$N; N=$((N + 1))
fi
sgdisk -n $N:0:+$((PS / 1048576 + 1))M -t $N:AF00 -c $N:anathema-payload "$OUT" >/dev/null
PN=$N; N=$((N + 1))
sgdisk -n $N:0:0 -t $N:AF0A -c $N:anathema-apfs "$OUT" >/dev/null
AN=$N
sgdisk -p "$OUT" | tail -5
start() { sgdisk -i $1 "$OUT" | awk '/First sector/{print $3}'; }
truncate -s $((ESP_MB * 1024 * 1024)) /tmp/esp_part.img
mkfs.fat -F32 /tmp/esp_part.img >/dev/null
mmd -i /tmp/esp_part.img ::/EFI ::/EFI/BOOT ::/anathema
mcopy -i /tmp/esp_part.img "$EFI" ::/EFI/BOOT/BOOTX64.EFI
for f in xkernel xkc.bin xcp.bin xcp2.bin xcp3.bin; do
  [ -f "$FILES/$f" ] && mcopy -i /tmp/esp_part.img "$FILES/$f" ::/anathema/$f
done
printf '%s' "$XARGS" > /tmp/xargs_disk.txt
mcopy -i /tmp/esp_part.img /tmp/xargs_disk.txt ::/anathema/xargs.txt
dd if=/tmp/esp_part.img of="$OUT" bs=512 seek=$(start 1) conv=notrunc status=none
[ -n "$HELP" ] && dd if="$HELP" of="$OUT" bs=512 seek=$(start $HN) conv=notrunc status=none
dd if="$PAY" of="$OUT" bs=512 seek=$(start $PN) conv=notrunc status=none
dd if="$APFS" of="$OUT" bs=512 seek=$(start $AN) conv=notrunc status=none
echo "esp@$(start 1) apfs=p$AN payload=p$PN ${HELP:+helper=p$HN}"
ls -la "$OUT"
