# Moved https://codeberg.org/icewind/tf-asset-loader

# tf-asset-loader

Utility for loading assets from tf2 data files.

Supports loading assets like models and textures from the tf2 data directory. The tf2 data directory should be
automatically detected when installed to steam, or you can use the `TF_DIR` environment variable to overwrite the data
directory.

Supports loading both plain file data, data embedded in `vpk` files and data embedded in `bsp` maps.
