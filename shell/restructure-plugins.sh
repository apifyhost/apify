#!/bin/bash

# Define list of source architecture directories
SOURCE_DIRS=(
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
)

# Target root directory
TARGET_ROOT="apify-plugins"

# Iterate through each source architecture directory
for src_dir in "${SOURCE_DIRS[@]}"; do
    # Check if source directory exists
    if [ ! -d "$src_dir" ]; then
        echo "Warning: Source directory $src_dir does not exist, skipping..."
        continue
    fi

    echo "Processing directory: $src_dir"

    # Iterate through all plugin directories in the source directory
    for plugin_dir in "$src_dir"/*/; do
        # Check if it's a directory
        if [ ! -d "$plugin_dir" ]; then
            continue
        fi

        # Extract plugin name (get directory name)
        plugin_name=$(basename "$plugin_dir")
        echo "  Processing plugin: $plugin_name"

        # Iterate through version directories in the plugin directory
        for version_dir in "$plugin_dir"/*/; do
            # Check if it's a directory
            if [ ! -d "$version_dir" ]; then
                continue
            fi

            # Extract version name
            version_name=$(basename "$version_dir")
            echo "    Processing version: $version_name"

            # Define target directory with version
            target_dir="$TARGET_ROOT/$plugin_name/$version_name"

            # Create target directory if it doesn't exist
            mkdir -p "$target_dir"
            if [ $? -ne 0 ]; then
                echo "    Error: Failed to create target directory $target_dir, skipping this version..."
                continue
            fi

            # Move all tar.gz files to target directory
            # Use mv -n to ensure existing files are not overwritten
            tar_files=$(find "$version_dir" -maxdepth 1 -type f -name "*.tar.gz")
            if [ -z "$tar_files" ]; then
                echo "    Warning: No tar.gz files found in version directory $version_dir"
                continue
            fi

            for tar_file in $tar_files; do
                filename=$(basename "$tar_file")
                if [ -f "$target_dir/$filename" ]; then
                    echo "    Note: $target_dir/$filename already exists, not overwriting"
                else
                    mv "$tar_file" "$target_dir/"
                    if [ $? -eq 0 ]; then
                        echo "    Moved: $filename to $target_dir"
                    else
                        echo "    Error: Failed to move $filename"
                    fi
                fi
            done
        done
    done
done

echo "Directory structure conversion completed"
