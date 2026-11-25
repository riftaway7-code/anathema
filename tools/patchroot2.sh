#!/bin/sh
# helper stage 2: wire the payload partition (gpu bundle + firmware) into the apfs recovery root
exec >/dev/console 2>&1
echo "patcher2: start"
i=0
while [ ! -e /dev/disk1s1 ] && [ $i -lt 90 ]; do
	/bin/sleep 1
	i=$((i + 1))
done
/sbin/mount_apfs -o rw /dev/disk1s1 /mnt/apfs
echo "patcher2: mount_apfs rc=$?"
R=/mnt/apfs
if [ -d $R/usr/bin ]; then
	# pmset stub
	/bin/rm -f $R/usr/bin/pmset
	echo '#!/bin/sh' > $R/usr/bin/pmset
	echo 'exit 0' >> $R/usr/bin/pmset
	/bin/chmod 755 $R/usr/bin/pmset
	# strip _Conclave so launchd imports these plists
	for f in com.apple.WindowServer.plist com.apple.audiomxd.plist; do
		P=$R/System/Library/LaunchDaemons/$f
		if [ -f $P ]; then
			/usr/bin/sed -e '/_Conclave/{N;d;}' $P > $P.new
			/bin/mv -f $P.new $P
		fi
	done
	# payload mount point + links the nvidia driver expects
	/bin/mkdir -p $R/usr/local/anathema-payload
	/bin/mkdir -p $R/Library/NullMoth
	/bin/rm -rf $R/Library/GPUBundles $R/Library/NullMoth/nvfw $R/usr/share/nvfw
	/bin/ln -s /usr/local/anathema-payload/GPUBundles $R/Library/GPUBundles
	/bin/ln -s /usr/local/anathema-payload/NullMoth/nvfw $R/Library/NullMoth/nvfw
	/bin/ln -s /usr/local/anathema-payload/usr-share-nvfw $R/usr/share/nvfw
	# boot-time job that mounts the payload partition before anything draws
	/bin/cp /usr/local/bin/mountpayload.sh $R/usr/local/bin/mountpayload.sh 2>/dev/null || {
		/bin/mkdir -p $R/usr/local/bin
		/bin/cp /usr/local/bin/mountpayload.sh $R/usr/local/bin/mountpayload.sh
	}
	/bin/chmod 755 $R/usr/local/bin/mountpayload.sh
	/bin/cp /usr/local/bin/com.anathema.payload.plist $R/System/Library/LaunchDaemons/com.anathema.payload.plist
	/bin/chmod 644 $R/System/Library/LaunchDaemons/com.anathema.payload.plist
	/bin/cp /usr/local/bin/diag.sh $R/usr/local/bin/diag.sh
	/bin/chmod 755 $R/usr/local/bin/diag.sh
	/bin/cp /usr/local/bin/com.anathema.diag.plist $R/System/Library/LaunchDaemons/com.anathema.diag.plist
	/bin/chmod 644 $R/System/Library/LaunchDaemons/com.anathema.diag.plist
	echo "patcher2: recovery root patched"
else
	echo "patcher2: no root volume"
fi
/bin/sync
echo "patcher2: done, rebooting"
/sbin/reboot
