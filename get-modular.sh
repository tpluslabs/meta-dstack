#!/bin/bash

cd srcs/poky
echo "Adding meta-dstack layer..."
bitbake-layers add-layer meta-dstack
echo "Replace confidential compute layer with gcp compatible patch and removing proxy"
rm -rf meta-confidential-compute
git clone https://github.com/flashbots/meta-confidential-compute
cd meta-confidential-compute
git checkout v3
rm -rf cvm-*
cd ..

echo "Applying dstack patches"

for patch_file in meta-dstack/patches/*; do
    if [ -f "$patch_file" ]; then
        base_patch=$(basename "$patch_file")
        # we want to disable the tweaks on prod
        if [ "${PROD:-false}" = "true" ] && [ "$base_patch" = "local.conf.patch" ]; then
            echo "PROD enabled: using production patch for local.conf"
            patch_file="meta-dstack/prod-patches/local.conf.patch"
        fi

        echo "Processing patch: $patch_file"
        header_line=$(grep '^--- ' "$patch_file" | head -n1)
        if [ -z "$header_line" ]; then
            echo "No header found in $patch_file; Shouldn't be happening, please report as a bug."
            continue
        fi
        target_path=$(echo "$header_line" | awk '{print $2}')
        target_path=${target_path%.orig}
        patch -N "$target_path" -i "$patch_file"
    fi
done

echo "You got modular dstack!"
