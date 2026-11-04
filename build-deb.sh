#!/usr/bin/env bash
# 将 Resources v1.6.0 打包为 loong64 .deb
#
# 前置条件（本脚本只负责打包，不负责编译）：
#   1. 已在本仓库根目录执行过：
#        meson setup builddir --prefix=/usr -Dprofile=default
#        ninja -C builddir
#   2. 已安装：fakeroot、dpkg-deb、meson、ninja、gettext(msgfmt)
#
# 产物：仓库根目录下的 resources_1.6.0_loong64.deb
set -euo pipefail

# 仓库根目录（即源码树本身），脚本可放在任意位置调用
SRC="$(cd "$(dirname "$0")" && pwd)"
STAGE="$SRC/pkgroot"
OUTDIR="$SRC"
VER=1.6.0
ARCH=loong64

echo "==> [1/5] meson install -> $STAGE"
rm -rf "$STAGE"
meson install --destdir="$STAGE" -C "$SRC/builddir"

echo "==> [2/5] sanity check 关键文件"
for f in \
  "$STAGE/usr/bin/resources" \
  "$STAGE/usr/libexec/resources/resources-kill" \
  "$STAGE/usr/libexec/resources/resources-processes" \
  "$STAGE/usr/libexec/resources/resources-adjust" \
  "$STAGE/usr/share/glib-2.0/schemas/net.nokyan.Resources.gschema.xml" \
  "$STAGE/usr/share/applications/net.nokyan.Resources.desktop" \
  "$STAGE/usr/share/metainfo/net.nokyan.Resources.metainfo.xml" \
  "$STAGE/usr/share/polkit-1/actions/net.nokyan.Resources.policy" ; do
  if [ ! -e "$f" ]; then echo "!! MISSING: $f"; exit 1; fi
done
echo "    OK, 关键文件齐全"

echo "==> [3/5] 写 DEBIAN/control"
mkdir -p "$STAGE/DEBIAN"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: resources
Version: $VER
Architecture: $ARCH
Maintainer: nokyan <nokyan@tuta.io>
Depends: libc6 (>= 2.34), libgtk-4-1 (>= 4.10.0), libadwaita-1-0 (>= 1.5.0), libglib2.0-0 (>= 2.66.0), libgio-2.0-0, libpolkit-gobject-1-0, libgdk-pixbuf-2.0-0
Section: utils
Priority: optional
Homepage: https://github.com/nokyan/resources
Description: Resources - 系统资源监视器
 用 GTK4 / libadwaita 构建的现代化系统资源监视器，
 可查看 CPU、GPU、内存、网络、磁盘、进程等系统资源占用。
 .
 本 loong64 构建来自 https://github.com/acdcGGE/resources
EOF

echo "==> [4/5] 写 postinst / postrm（编译 gschema、刷新图标与 desktop 缓存）"
cat > "$STAGE/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
  glib-compile-schemas /usr/share/glib-2.0/schemas 2>/dev/null || true
  gtk4-update-icon-cache -q -t /usr/share/icons/hicolor 2>/dev/null || true
  update-desktop-database /usr/share/applications 2>/dev/null || true
fi
EOF
cat > "$STAGE/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
  glib-compile-schemas /usr/share/glib-2.0/schemas 2>/dev/null || true
  gtk4-update-icon-cache -q -t /usr/share/icons/hicolor 2>/dev/null || true
  update-desktop-database /usr/share/applications 2>/dev/null || true
fi
EOF
chmod 0755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"

echo "==> [5/5] fakeroot dpkg-deb --build"
OUT="$OUTDIR/resources_${VER}_${ARCH}.deb"
rm -f "$OUT"
fakeroot dpkg-deb --build "$STAGE" "$OUT"
echo "=== BUILT: $OUT ==="
echo "--- control ---"; dpkg-deb -I "$OUT" 2>/dev/null | head -20
echo "--- 文件数 ---"; dpkg-deb -c "$OUT" 2>/dev/null | wc -l
echo "--- 前 40 个文件 ---"; dpkg-deb -c "$OUT" 2>/dev/null | head -40
