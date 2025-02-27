# Meta Dstack
### Get modular dstack!

This repo provides:
1. patches over the flashbots/yocto-manifests to enable flashbots/meta-confidential-compute to run on gcp by setting the [required kernel flags](https://cloud.google.com/confidential-computing/confidential-vm/docs/create-custom-confidential-vm-images#intel-tdx). Since meta-confidential-compute already had a branch with a more structured whay to handle kernel features on different targets we use that instead of writing these flags to a patch. We also remove some services that we don't want to default include (cvm-*).
2. a simple yocto layer that allows to boot images running the guest environment in https://github.com/heytdep/rs-modular-dstack. 

# Setup

Follow the guides (either yocto's or flashbots/yocto-manifests) to enable your image building os to work with yocto, if you're on ubuntu:

```
sudo apt update
sudo apt install gawk wget git diffstat unzip texinfo gcc build-essential chrpath socat cpio python3 python3-pip python3-pexpect xz-utils debianutils iputils-ping python3-git python3-jinja2 libegl1-mesa libsdl1.2-dev xterm python3-subunit mesa-common-dev zstd liblz4-tool chrpath diffstat lz4 mtools repo
sudo locale-gen en_US.UTF-8
```

Then create and initialize the multirepo directory:

```
mkdir yetanother; cd yetanother
repo init -u https://github.com/flashbots/yocto-manifests.git -b main -m tdx-base.xml
repo sync
source setup
cd srcs/poky;git clone https://github.com/tpluslabs/meta-dstack;cd ../../
chmod 777 srcs/poky/meta-dstack/get-modular.sh
./srcs/poky/meta-dstack/get-modular.sh
```
