apt-get install glusterfs-server

# Enable and start GlusterFS SystemD Unit
systemctl enable glusterd
systemctl start glusterd

# Create directory for brick
mkdir -p /data/glusterfs/myvol1/brick1

# Create volume in gluster using parameter 'force' to skip verification to create from '/' 
gluster volume create myvol1 $HOSTNAME:/data/glusterfs/myvol1/brick1/brick force

# Start server
gluster volume start myvol1

# To mount on clients
mkdir -p /mnt/glusterfs
mount -t glusterfs $HOSTNAME:myvol1 /mnt/glusterfs

