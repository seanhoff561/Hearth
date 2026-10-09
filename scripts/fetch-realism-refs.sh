#!/usr/bin/env bash
# The realism analysis's reference data (Amendment T §3.2), fetched into bench-out/realism/refs:
# never committed or shipped. Each source and its licence: docs/review/realism/references.md.
#
#   --dem      one-metre lidar (USGS 3DEP, public domain), one-arc-second SRTM tiles and
#              terrarium tiles at random over the land (the AWS terrain tiles), about 800 MB,
#              for `bench realism terrain` and `bench realism global`
#   --photos   openly licensed photographs from Wikimedia Commons matched to the realism suite's
#              shots, with their authors and licences in photos.tsv, and the contact sheet
#              (bench-out/realism/sheet.html) laying the suite's renders beside them
#
# Usage: scripts/fetch-realism-refs.sh [--dem] [--photos]   (both when neither is given)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

DEM=0
PHOTOS=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dem) DEM=1; shift ;;
    --photos) PHOTOS=1; shift ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
if [ "$DEM" = 0 ] && [ "$PHOTOS" = 0 ]; then DEM=1; PHOTOS=1; fi

REFS=bench-out/realism/refs

# A file fetched once (kept if already whole), retried with backoff on a dropped connection.
fetch() {
  local url=$1 out=$2
  [ -s "$out" ] && return 0
  mkdir -p "$(dirname "$out")"
  for wait in 2 4 8 16 0; do
    if curl -fsSL --retry 2 -o "$out.part" "$url"; then
      mv "$out.part" "$out"
      echo "  $(basename "$out")"
      return 0
    fi
    [ "$wait" = 0 ] && break
    sleep "$wait"
  done
  rm -f "$out.part"
  echo "could not fetch $url" >&2
  return 1
}

if [ "$DEM" = 1 ]; then
  echo "==> one-metre lidar (USGS 3DEP)"
  TNM=https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1m/Projects
  for t in \
    OR_SouthCoast_2019_A19/TIFF/USGS_1M_10_x41y481_OR_SouthCoast_2019_A19.tif \
    IA_SouthCentral_2020_D20/TIFF/USGS_1M_15_x45y454_IA_SouthCentral_2020_D20.tif \
    MT_GlacierNP_2016/TIFF/USGS_one_meter_x29y538_MT_GlacierNP_2016.tif \
    NV_ClarkCounty_2018_C19/TIFF/USGS_1M_11_x74y407_NV_ClarkCounty_2018_C19.tif \
    MN_LakeCounty_2018_C20/TIFF/USGS_1M_15_x61y530_MN_LakeCounty_2018_C20.tif; do
    fetch "$TNM/$t" "$REFS/lidar/$(basename "$t")"
  done

  echo "==> one-arc-second SRTM (AWS terrain tiles)"
  for t in N43W124 N41W094 N48W114 N36W115 N47W092 N46E007; do
    if [ ! -s "$REFS/srtm/$t.hgt" ]; then
      fetch "https://s3.amazonaws.com/elevation-tiles-prod/skadi/${t:0:3}/$t.hgt.gz" \
        "$REFS/srtm/$t.hgt.gz"
      gunzip -f "$REFS/srtm/$t.hgt.gz"
    fi
  done

  echo "==> terrain tiles for the planet-wide comparison (AWS terrain tiles)"
  # The land mask first, then the windows on land it picks.
  for round in 1 2; do
    cargo run --profile dev-opt -q -p bench -- realism missing-tiles --refs "$REFS" |
      while read -r url; do
        fetch "$url" "$REFS/terrarium/${url#*/terrarium/}" >/dev/null
      done
  done
fi

if [ "$PHOTOS" = 1 ]; then
  echo "==> photographs (Wikimedia Commons, free licences only)"
  # Wikimedia asks scripts to name themselves and to go gently.
  UA="hearth-realism-refs/1.0 (a game's local comparison with real places)"
  cargo run --profile dev-opt -q -p bench -- realism photo-queries |
    while IFS=$'\t' read -r key url; do
      out="$REFS/photos/$key/search.json"
      [ -s "$out" ] && continue
      mkdir -p "$(dirname "$out")"
      curl -fsSL -A "$UA" -o "$out" "$url" || echo "the search for $key failed" >&2
      sleep 1
    done
  cargo run --profile dev-opt -q -p bench -- realism photo-pick --refs "$REFS" |
    while IFS=$'\t' read -r url path; do
      [ -s "$path" ] && continue
      curl -fsSL -A "$UA" -o "$path" "$url" && echo "  $path"
      sleep 1
    done
  cargo run --profile dev-opt -q -p bench -- realism sheet --refs "$REFS"
  echo "Credits in $REFS/photos/photos.tsv; the sheet in bench-out/realism/sheet.html"
fi
echo "References in $REFS"
