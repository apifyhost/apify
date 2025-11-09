#!/usr/bin/env bash
# save as: collect_rust.sh
# usage:   bash collect_rust.sh

# 输出文件
OUT_FILE="./all_rust_with_path.txt"
# 清空或创建输出文件
> "$OUT_FILE"

# 找到所有 .rs 文件（排除 OUT_FILE 自身，防止重复追加，同时排除 target 目录）
find . -type f -name '*.rs' ! -path "./${OUT_FILE}" ! -path "./target/*" | while IFS= read -r file; do
    # 把路径写成注释，避免破坏 Rust 语法
    echo "// ---------- File: $file ----------"
    cat "$file"
    echo
done >> "$OUT_FILE"

echo "✅ 已合并 $(find . -type f -name '*.rs' ! -path "./${OUT_FILE}" ! -path "./target/*" | wc -l) 个 rust 文件到 $OUT_FILE"
