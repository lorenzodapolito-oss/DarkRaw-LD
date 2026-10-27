#!/usr/bin/env bash
# Regenerate every app icon from assets/app-icon/darkraw-source.png.
#
# Needs: python3 with Pillow (pip install pillow). The outputs are committed, so packaging never
# needs this. The work is done by packaging/icons.py (also runnable directly on Windows).
#
#   packaging/icons.sh
set -euo pipefail
exec python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/icons.py"
