#!/bin/sh
M=/usr/local/anathema-payload
n=0
while [ $n -lt 60 ]; do
	for d in /dev/disk0s2 /dev/disk0s1 /dev/disk0s3 /dev/disk1s1 /dev/disk2s2 /dev/disk2s1 /dev/disk2s3 /dev/disk3s2 /dev/disk3s1 /dev/disk3s3; do
		[ -e $d ] || continue
		/sbin/mount_hfs -o ro $d $M 2>/dev/null
		[ -e $M/.anathema-payload ] && exit 0
		/sbin/umount $M 2>/dev/null
	done
	/bin/sleep 1
	n=$((n + 1))
done
exit 1
