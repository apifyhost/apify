#!/usr/bin/env bash

set -e

MODULE_EXTENSION="so"
if [[ "$OS" == "macos-latest" ]]; then
  MODULE_EXTENSION="dylib"
fi

if [ ! -d "./plugins-target" ]; then
    echo "📦 Create folder ./plugins-target"
    mkdir -p ./plugins-target
fi

echo "📦 Clean folder ./plugins-target"
rm -rf ./plugins-target/*

if ! command -v yq &> /dev/null; then
  echo "yq not found. Please install yq (https://github.com/mikefarah/yq)"
  exit 1
fi

package_plugin() {
    MODULE_DIR="$1"

    cd "$MODULE_DIR"

    if [ -f "apify.yaml" ]; then
      METADATA_FILE="apify.yaml"
    elif [ -f "apify.yml" ]; then
      METADATA_FILE="apify.yml"
    else
      echo "No apify.yaml/yml file found in $MODULE_DIR"
      exit 1
    fi

    echo "📄 Metadata file found: $METADATA_FILE"

    NAME=$(yq -r '.name' "$METADATA_FILE")
    VERSION=$(yq -r '.version' "$METADATA_FILE")
    REPOSITORY=$(yq -r '.repository' "$METADATA_FILE")
    LICENSE=$(yq -r '.license' "$METADATA_FILE")
    AUTHOR=$(yq -r '.author' "$METADATA_FILE")

    echo "🔎 Loaded metadata:"
    echo "  name: $NAME"
    echo "  version: $VERSION"
    echo "  repository: $REPOSITORY"
    echo "  license: $LICENSE"
    echo "  author: $AUTHOR"

    if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-a-zA-Z0-9\.]+)?(\+[a-zA-Z0-9\.]+)?$ ]]; then
      echo "❌ Invalid version format: $VERSION"
      exit 1
    fi

    KNOWN_LICENSES=("MIT" "Apache-2.0" "GPL-3.0" "BSD-3-Clause" "MPL-2.0" "LGPL-3.0" "CDDL-1.0" "EPL-2.0" "Unlicense")
    VALID_LICENSE=false
    for lic in "${KNOWN_LICENSES[@]}"; do
      if [ "$LICENSE" == "$lic" ]; then
        VALID_LICENSE=true
        break
      fi
    done

    if ! $VALID_LICENSE; then
      if ! [[ "$LICENSE" =~ ^https?://.*$ ]]; then
        echo "❌ Invalid license: $LICENSE"
        exit 1
      fi
    fi

    echo "⚙️ Building module..."

    if [[ "$CROSS" == "true" ]]; then
      cross build --target "$TARGET" --release --locked
    else
      cargo build --target "$TARGET" --release --locked
    fi

    TMP_DIR=".tmp/${NAME}"
    mkdir -p "$TMP_DIR"

    SO_NAME="lib${NAME}.${MODULE_EXTENSION}"
    RELEASE_PATH="../../target/$TARGET/release/$SO_NAME"

    if [ ! -f "$RELEASE_PATH" ]; then
      echo "❌ Missing built file: $RELEASE_PATH"
      exit 1
    fi

    cp "$RELEASE_PATH" "$TMP_DIR/module.${MODULE_EXTENSION}"
    cp "$METADATA_FILE" "$TMP_DIR/"

    ARCHIVE_NAME="${NAME}-${VERSION}.tar.gz"

    echo "📦 Creating archive: $ARCHIVE_NAME"
    tar -czf "$ARCHIVE_NAME" -C "$TMP_DIR" .

    rm -rf "$TMP_DIR"

    cd - > /dev/null

    if [ ! -d "./plugins-target/${NAME}/${VERSION}" ]; then
        echo "📦 Create folder ./plugins-target/${NAME}"
        mkdir -p ./plugins-target/${NAME}
    fi

    RENAMED_ARCHIVE="${NAME}-${VERSION}-${TARGET}.tar.gz"
    mv "$MODULE_DIR/$ARCHIVE_NAME" "./plugins-target/${NAME}/${VERSION}/$RENAMED_ARCHIVE"

    echo "✅ Plugin packaged: $RENAMED_ARCHIVE"
}

for dir in ./plugins/*/; do
    if [ -d "$dir" ]; then
        echo "🚀 Processing plugin: $dir"
        package_plugin "$dir"
    fi
done

echo "🎉 All plugins packaged successfully!"
