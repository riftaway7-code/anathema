#!/bin/bash
# build the final usb image from the patched apfs partition of /tmp/full.img, test-boot it once, copy it to E:
set -e
cd ~/anathema-work
export EFI=~/xboot-target/x86_64-unknown-uefi/release/anathema-xboot.efi FILES=$HOME/anathema-work
cp xkc_full.bin xkc.bin
P4S=$(sgdisk -i 4 /tmp/full.img | awk '/First sector/{print $3}')
P4E=$(sgdisk -i 4 /tmp/full.img | awk '/Last sector/{print $3}')
dd if=/tmp/full.img of=/tmp/apfs_final.raw bs=512 skip=$P4S count=$((P4E - P4S + 1)) status=none
export XARGS='-v debug=0x14e cpus=1 kt_fixed_zones=200 kt_var_heaps=6 rd=*799A9BDF-BE67-493C-9EA5-BF73A6EC3A0D nvfb=1 nvaccel=1 nvfbheads=4 -nvkmsnosmooth ipc_control_port_options=0'
bash mkdisk4.sh /tmp/final.img /tmp/apfs_final.raw /tmp/payload_hfs.img | tail -2
# one test boot of the final image as an emulated usb stick (serial log only for the check)
TAG=fin IMG=/tmp/final.img WAIT=420 bash rundisk2.sh
L=/tmp/serial_fin.log
tr -d '\r' < $L | sed 's/\x1b\[[0-9;]*m//g' > /tmp/fin_clean.log
echo "--- final image check:"
grep -a 'hello, launchd\|BaseSystem environment\|panic(cpu\|Still waiting\|anx sandbox\|loadkext' /tmp/fin_clean.log | head -8 | cut -c1-180
echo "--- diag:"
sed -n '/diag: start/,$p' /tmp/fin_clean.log | grep -a -v 'launchd|' | head -120 | cut -c1-200
mkdir -p /mnt/e/anathema-usb
cp /tmp/final.img /mnt/e/anathema-usb/anathema-test.img
ls -la /mnt/e/anathema-usb/
echo FINALIZE-DONE
