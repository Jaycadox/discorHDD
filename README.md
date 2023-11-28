# discorHDD
A Network Block Device plugin hosted on Discord.

Credit to tom7 for the mount and run scripts.

## Usage
1. Provde bot token and channel in config at `/etc/discorhdd/config.toml`
2. Compile in release
3. Copy libdiscorhdd.so from the release folder to the same directory as the bash scripts
4. Rename libdiscorhdd.so to discorhdd.~~
5. `chmod +x` the bash scripts
6. Run `run.sh` as root
7. After a little while, a FAT12 mount should be at /mnt/discorhdd/ 
