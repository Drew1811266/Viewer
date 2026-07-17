#!/usr/bin/env bash

set -euo pipefail

OUTPUT_DIRECTORY="${1:-}"
if [[ -z "$OUTPUT_DIRECTORY" || "$OUTPUT_DIRECTORY" == "/" || "$OUTPUT_DIRECTORY" == "." || "$OUTPUT_DIRECTORY" == ".." ]]; then
  printf 'usage: %s <safe-output-directory>\n' "$0" >&2
  exit 2
fi
if [[ -e "$OUTPUT_DIRECTORY" && ! -d "$OUTPUT_DIRECTORY" ]]; then
  printf 'refusing to replace non-directory output: %s\n' "$OUTPUT_DIRECTORY" >&2
  exit 2
fi
if [[ -d "$OUTPUT_DIRECTORY" ]]; then
  FIRST_ENTRY="$(find "$OUTPUT_DIRECTORY" -mindepth 1 -maxdepth 1 -print -quit)"
  if [[ -n "$FIRST_ENTRY" && ! -f "$OUTPUT_DIRECTORY/.viewer-g3-generated-corpus" ]]; then
    printf 'refusing to replace unmarked non-empty directory: %s\n' "$OUTPUT_DIRECTORY" >&2
    exit 2
  fi
fi

rm -rf -- "$OUTPUT_DIRECTORY"
mkdir -p "$OUTPUT_DIRECTORY/.viewer" "$OUTPUT_DIRECTORY/.hidden/nested"
touch "$OUTPUT_DIRECTORY/.viewer-g3-generated-corpus"
printf 'viewer-portable-metadata-sentinel-v1\n' > "$OUTPUT_DIRECTORY/.viewer/metadata.sqlite"
printf 'excluded hidden image\n' > "$OUTPUT_DIRECTORY/.hidden/nested/hidden.jpg"

image_count=0
text_count=0
for category_number in {0..9}; do
  category="$(printf 'category-%02d' "$category_number")"
  for id_number in {0..9}; do
    global_id=$((category_number * 10 + id_number))
    id="$(printf 'id-%03d' "$global_id")"
    set_name="set-$((id_number % 5))"
    image_directory="$OUTPUT_DIRECTORY/$category/$id/$set_name"
    mkdir -p "$image_directory"
    for image_number in {0..9}; do
      global_image=$((global_id * 10 + image_number))
      extension="jpg"
      if ((global_image % 2 == 1)); then
        extension="png"
      fi
      image_name="$(printf 'image-%04d.%s' "$global_image" "$extension")"
      printf 'Viewer G3 placeholder %04d\n' "$global_image" > "$image_directory/$image_name"
      image_count=$((image_count + 1))
    done
    text_extension="md"
    if ((global_id % 2 == 1)); then
      text_extension="txt"
    fi
    text_path="$OUTPUT_DIRECTORY/$category/$id/$(printf 'description-%03d.%s' "$global_id" "$text_extension")"
    printf '产品说明 %03d\n白色陶瓷杯，正面产品图。\nEnglish product prompt and ordinary notes.\n' "$global_id" > "$text_path"
    text_count=$((text_count + 1))
  done
done

ln -s "$OUTPUT_DIRECTORY/category-00" "$OUTPUT_DIRECTORY/linked-category"
printf 'unsupported\n' > "$OUTPUT_DIRECTORY/ignored.pdf"
printf '{\n  "schema_version": 1,\n  "images": %d,\n  "text_files": %d,\n  "expected_folders": 210,\n  "excluded_hidden_files": 1,\n  "excluded_symlinks": 1\n}\n' \
  "$image_count" "$text_count" > "$OUTPUT_DIRECTORY/manifest.json"

printf 'Generated G3 corpus at %s (%d images, %d text files)\n' \
  "$OUTPUT_DIRECTORY" "$image_count" "$text_count"
