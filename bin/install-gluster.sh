#!/bin/bash

set -euf -o pipefail

setup_gluster() {
  echo -e "setup gluster"
  echo -e "\tcreate vol"
  gluster vol create test $HOSTNAME:/mnt/gluster-brick force
  echo -e "\tstart vol"
  gluster vol start test
  gluster vol set test server.allow-insecure on
  mkdir /mnt/glusterfs
  mount -t glusterfs localhost:test /mnt/glusterfs
  mkdir /mnt/glusterfs/gfapi
  chmod 777 /mnt/glusterfs/gfapi
}

_gluster() {
  setup_gluster
}


if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root"
   exit 1
fi

mkdir -p $HOME/.config
_gluster
