#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s [--check]\n' "$(basename "$0")"
  printf '\n'
  printf 'Synchronize backend/migration_sources into backend/migrations.\n'
  printf '  --check   verify the flat SQLx directory is in sync without copying\n'
}

mode="sync"
if [ "${1:-}" = "--check" ]; then
  mode="check"
elif [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
elif [ "${1:-}" != "" ]; then
  usage >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
source_root="$repo_root/backend/migration_sources"
flat_root="$repo_root/backend/migrations"

if [ ! -d "$source_root" ]; then
  printf 'Missing source directory: %s\n' "$source_root" >&2
  exit 1
fi

if [ ! -d "$flat_root" ]; then
  printf 'Missing SQLx migrations directory: %s\n' "$flat_root" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

source_paths="$tmp_dir/source_paths"
flat_paths="$tmp_dir/flat_paths"
source_names="$tmp_dir/source_names"
flat_names="$tmp_dir/flat_names"

find "$source_root" -type f -name '*.sql' | sort > "$source_paths"
find "$flat_root" -maxdepth 1 -type f -name '*.sql' | sort > "$flat_paths"
sed 's#.*/##' "$source_paths" | sort > "$source_names"
sed 's#.*/##' "$flat_paths" | sort > "$flat_names"

source_count="$(wc -l < "$source_names" | tr -d '[:space:]')"
flat_count="$(wc -l < "$flat_names" | tr -d '[:space:]')"

if [ "$source_count" = "0" ]; then
  printf 'No source migrations found under %s\n' "$source_root" >&2
  exit 1
fi

duplicate_sources="$(sort "$source_names" | uniq -d)"
if [ "$duplicate_sources" != "" ]; then
  printf 'Duplicate source migration basenames:\n%s\n' "$duplicate_sources" >&2
  exit 1
fi

duplicate_flat="$(sort "$flat_names" | uniq -d)"
if [ "$duplicate_flat" != "" ]; then
  printf 'Duplicate flat migration basenames:\n%s\n' "$duplicate_flat" >&2
  exit 1
fi

layout_errors="$tmp_dir/layout_errors"
: > "$layout_errors"
while IFS= read -r source_path; do
  name="$(basename "$source_path")"
  rel="${source_path#$source_root/}"
  parent="$(basename "$(dirname "$source_path")")"
  depth="$(printf '%s' "$rel" | awk -F/ '{print NF}')"
  description="${name#*_}"

  if [ "$depth" -lt 3 ]; then
    printf '%s must live under <module>/<kind>/%s\n' "$rel" "$name" >> "$layout_errors"
    continue
  fi

  case "$description" in
    create_*) expected_kind="schema" ;;
    seed_*) expected_kind="seed" ;;
    *) expected_kind="patch" ;;
  esac

  if [ "$parent" != "$expected_kind" ]; then
    printf '%s must live in %s, not %s\n' "$rel" "$expected_kind" "$parent" >> "$layout_errors"
  fi
done < "$source_paths"

if [ -s "$layout_errors" ]; then
  printf 'Migration source layout errors:\n' >&2
  cat "$layout_errors" >&2
  exit 1
fi

flat_without_source="$tmp_dir/flat_without_source"
source_without_flat="$tmp_dir/source_without_flat"
comm -23 "$flat_names" "$source_names" > "$flat_without_source"
comm -13 "$flat_names" "$source_names" > "$source_without_flat"

if [ -s "$flat_without_source" ]; then
  printf 'Flat SQLx migrations missing from migration_sources:\n' >&2
  cat "$flat_without_source" >&2
  exit 1
fi

if [ "$mode" = "check" ] && [ -s "$source_without_flat" ]; then
  printf 'Source migrations missing from flat SQLx directory:\n' >&2
  cat "$source_without_flat" >&2
  exit 1
fi

drift_file="$tmp_dir/drift"
changed_count=0
: > "$drift_file"

while IFS= read -r source_path; do
  name="$(basename "$source_path")"
  flat_path="$flat_root/$name"

  if [ "$mode" = "check" ]; then
    if [ ! -f "$flat_path" ]; then
      printf '%s is missing from backend/migrations\n' "$name" >> "$drift_file"
    elif ! cmp -s "$source_path" "$flat_path"; then
      printf '%s differs from backend/migrations/%s\n' "${source_path#$repo_root/}" "$name" >> "$drift_file"
    fi
    continue
  fi

  if [ ! -f "$flat_path" ] || ! cmp -s "$source_path" "$flat_path"; then
    cp "$source_path" "$flat_path"
    changed_count=$((changed_count + 1))
  fi
done < "$source_paths"

if [ "$mode" = "check" ]; then
  if [ -s "$drift_file" ]; then
    printf 'Migration source drift detected:\n' >&2
    cat "$drift_file" >&2
    exit 1
  fi
  printf 'Migration sources are in sync (%s files).\n' "$source_count"
else
  printf 'Synchronized %s migration source files into backend/migrations (%s changed).\n' "$source_count" "$changed_count"
  if [ "$flat_count" != "$source_count" ]; then
    printf 'Note: backend/migrations now has %s files after sync.\n' "$(find "$flat_root" -maxdepth 1 -type f -name '*.sql' | wc -l | tr -d '[:space:]')"
  fi
fi
