#!/bin/bash

if [[ $(id -u) -ne 0 ]]; then
	echo "Please run as root"
	exit -1
fi

echo "$1" "$2"

if [[ "$1" = "" ]]; then
	echo "Needs the unix socket; call from run.sh"
	exit -1
fi

if [[ "$2" = "" ]]; then
	echo "Needs the directory to mount, like /mnt/pingu; call from run.sh"
	exit -1
fi

echo "STARTED SERVER on $1"

# nbd-client localhost for TCP
# 100 hour timeout
nbd-client -timeout 360000 -unix "$1" /dev/nbd0 || exit -1

# It appears to be possible to make a FAT12 filesystem and
# mount it without a partition table (just /dev/nbd0 instead of
# /dev/nbd0p1), which makes it possible to have smaller disks.

# -a disables alignment, saving space
mkfs.vfat -F 12 -v -a -n "discorhdd" /dev/nbd0 || exit -1
# disable atime, or else reads produce writes
mount -o noatime,nodiratime /dev/nbd0 "$2" || exit -1

df -k "$2"
echo "OK"

# enter a prompt
bash

# when the prompt exits, try to clean up
nbd-client -d /dev/nbd0
