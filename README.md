# Meta Dstack

(inspired by [flashbox](https://github.com/flashbots/flashbox/))

### Minimal dstackOS with virtualization!

This repo provides:
1. patches over the flashbots/yocto-manifests to enable flashbots/meta-confidential-compute to run on gcp by setting the [required kernel flags](https://cloud.google.com/confidential-computing/confidential-vm/docs/create-custom-confidential-vm-images#intel-tdx). Since meta-confidential-compute already had a branch with a more structured whay to handle kernel features on different targets we use that instead of writing these flags to a patch. We also remove some services that we don't want to default include (cvm-*).
2. virtualization through meta-virtualization + a small patch (a file rename mainly) over custom-podman from flashbots to enable podman. 
3. a simple yocto layer that contains a [minimal cvm server implementation](./server/) to manage pods and generate quotes without extending rtmrs (current gcp guest seems to not have merged the path to extend rtmrs through tsm).

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
```

Now you can run the `get-modular` script to apply the patches and add the dstack layer to the base minimal image:

```
./srcs/poky/meta-dstack/get-modular.sh
```

> If you're deploying production then set `PROD=true` before running the above script, it will apply the prod patches.

# Build

You can now build the `core-image-minimal` image with bitbake! 

```
cd srcs/poky/
bitbake core-image-minimal
```

# Usage

To deploy a new pod just post the pod.yml to {serveraddress}:3030/pods.

> It's important to specify the image source, else mini-server will hang (will change soon). For example, don't specify `image: alpine:latest` rather `image: docker.io/library/alpine:latest`.

### Example

This pod will just log the obtained quote:

```
echo -e "apiVersion: v1
kind: Pod
metadata:
  name: echo-pod
spec:
  hostNetwork: true
  restartPolicy: Never
  containers:
    - name: echo
      image: docker.io/library/alpine:latest
      imagePullPolicy: ifnotpresent
      command:
        - sh
        - -c
        - |
          while true; do
            wget -qO- http://0.0.0.0:3030/quote/2c5fd2a29c8ab5c4a329b1d26b225758
            echo
            sleep 5
          done
" > pod.yml
```

Then post the pod:

```
curl -X POST \
     -H "Content-Type: application/x-yaml" \
     --data-binary @pod.yml \
     http://serveraddress:3030/pods
```

# GCP Deployment

This is only tested on a gcp tdx deployment. To replicate:

1. From your image builder machine, push the wic to a bucket of your choice (if you don't have one create it `gcloud storage buckets create "gs://${tdx-gcp}"`):

```
gsutil cp core-image-minimal-tdx-gcp.rootfs-{latest time tag}.wic.tar.gz gs://tdx-gcp
```

2. Create custom image on gcp:

```
gcloud compute images create "tplus-dstack" \
    --source-uri="gs://tdx-gcp/core-image-minimal-tdx-gcp.rootfs-20250522164856.wic.tar.gz" \
    --guest-os-features=UEFI_COMPATIBLE,VIRTIO_SCSI_MULTIQUEUE,GVNIC,TDX_CAPABLE
```

3. Create the td vm instance:

```
gcloud compute instances create "tplus-dstack" \
        --zone="..." \
        --machine-type="..." \
        --image="tplus-dstack" \
        --confidential-compute-type=TDX \
        --maintenance-policy=TERMINATE \ 
        --no-shielded-secure-boot \
        --no-shielded-vtpm
```

> note: disable the secure boot + vtpm so google's firmware measures "according" to spec (even if vf doesn't seem to be public?).

# License

MIT license.
