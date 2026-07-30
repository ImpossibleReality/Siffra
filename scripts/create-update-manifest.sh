#!/usr/bin/env bash
# Build cargo-packager-updater's multi-platform manifest from matrix metadata.
# The GitHub Release assets must use unique basenames because those are the URLs
# written into latest.json.
set -euo pipefail

tag="${1:?usage: create-update-manifest.sh <tag> <release-assets-directory>}"
assets_dir="${2:?usage: create-update-manifest.sh <tag> <release-assets-directory>}"
notes_file="$assets_dir/release-notes.md"

test -f "$notes_file"

platforms='{}'
while IFS= read -r -d '' metadata; do
    asset_dir="$(dirname "$metadata")"
    platform="$(jq -er '.platform' "$metadata")"
    asset="$(jq -er '.asset' "$metadata")"
    format="$(jq -er '.format' "$metadata")"
    package="$asset_dir/$asset"
    signature="$package.sig"

    test -f "$package"
    test -f "$signature"

    url="https://github.com/ImpossibleReality/Siffra/releases/download/$tag/$asset"
    platforms="$(jq \
        --arg platform "$platform" \
        --arg url "$url" \
        --arg signature "$(<"$signature")" \
        --arg format "$format" \
        '. + {($platform): {url: $url, signature: $signature, format: $format}}' \
        <<<"$platforms")"
done < <(find "$assets_dir" -name updater.json -print0)

test "$platforms" != '{}'
jq -n \
    --arg version "${tag#v}" \
    --rawfile notes "$notes_file" \
    --arg pub_date "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
    --argjson platforms "$platforms" \
    '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' \
    > "$assets_dir/latest.json"
