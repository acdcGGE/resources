# Resources — LoongArch64 (loong64) 移植版

本仓库是 [nokyan/resources](https://github.com/nokyan/resources) **v1.6.0** 的
LoongArch64（龙芯）移植构建工程，目标是让 Resources 能在 Deepin 25 / loong64
等平台上编译运行，并正确识别龙芯机器的传感器。

> 上游已并入 GNOME Incubator，本仓库仅维护 loong64 的可用构建，不含上游后续开发。

---

## 与上游的差异（补丁）

相对 v1.6.0 仅做了**两处必要改动**，以便在本架构下编译并读取传感器：

### 1. `lib/process_data/Cargo.toml` — 修复依赖不支持龙芯

Resources 内置的 `lib/process_data` 依赖 `syscalls = "0.6.18"`，而该版本**完全没有
LoongArch64 支持**（全仓库无 loong 相关代码，`Sysno` 枚举按 `cfg(target_arch)`
分别导出，龙芯目标下无匹配项 → 编译失败）。

改为：

```toml
syscalls = { version = "0.8.1", features = ["loongarch64"] }
```

`syscalls 0.7.0 / 0.8.x` 已加入 loongarch64 支持；同时 `Cargo.lock` 已对应更新。

### 2. `src/utils/cpu.rs` — 适配龙芯传感器路径

原版只认 x86 的 hwmon 名（`zenpower`/`coretemp`/`k10temp`）和 thermal zone
（`x86_pkg_temp`/`acpitz`），并只从 `/sys/.../cpufreq/scaling_cur_freq` 读频率、
从 `lscpu` 的 `Virtualization:` / `CPU max MHz:` 读虚拟化与最大频率。龙芯机器
暴露的路径不同，导致这几项全部读不到。改动：

- **CPU 温度**：`KNOWN_HWMONS` 增加 `cpu_hwmon`、`loongson3`；新增兜底
  `search_for_any_cpu_hwmon()`，扫描任意非 GPU 的 hwmon 取 `temp1_input`。
- **CPU 频率**：`get_cpu_freq()` 在 cpufreq sysfs 缺失时，新增从
  `/proc/cpuinfo` 的 `CPU MHz` 读取的兜底 `get_cpu_freq_from_proc()`。
- **最大频率**：`cpu_info()` 的 `max_speed` 在 `lscpu` 无 `CPU max MHz` 时，
  复用 `get_cpu_freq_from_proc(0)`。
- **虚拟化**：`cpu_info()` 的 `virtualization` 在 `lscpu` 无 `Virtualization:`
  字段时，从 `Flags:` 检测——龙芯的 `lvz`（Loongson Virtualization）返回
  `"Loongson Virtualization (LVZ)"`，并兼容 Intel `vmx`、AMD `svm`。

---

## 构建依赖

| 类别 | 内容 |
|------|------|
| 编译器 | **Rust 1.87.0+**（关键：带龙芯支持的 `syscalls` 用到了 `usize::is_multiple_of` 这个 const fn，它在 **Rust 1.87.0** 才稳定；用 rustup 安装，不要用系统 apt 的 1.81） |
| 构建系统 | `meson` (≥0.63)、`ninja`、`gettext`（提供 `msgfmt`） |
| GTK 依赖 | `libgtk-4-dev` (≥4.10)、`libadwaita-1-dev` (≥1.5)、`libglib2.0-dev`、`libpolkit-gobject-1-dev`、`libgdk-pixbuf-2.0-dev` |
| 打包 | `fakeroot`、`dpkg-deb` |

### 安装 Rust（rustup，与系统 apt 的 1.81 互不干扰）

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install 1.87.0
rustup default 1.87.0
```

### （可选）国内 cargo 镜像

若在中国大陆，编译时会从 crates.io 拉取约 195 个 crate，建议配置
[rsproxy](https://rsproxy.cn) 镜像（注意：meson 会强制 `CARGO_HOME=builddir/cargo`，
需在 `meson setup` 之后把下面这段写入 `builddir/cargo/config.toml`）：

```toml
[source.crates-io]
replace-with = "rsproxy-sparse"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"
[net]
git-fetch-with-cli = true
```

---

## 构建步骤

```bash
# 0. 确认 Rust 版本
rustc --version        # 应为 1.87.0

# 1. 配置（必须用 --prefix=/usr，否则打包脚本的 sanity check 不通过）
meson setup builddir --prefix=/usr -Dprofile=default

#   （可选）注入 cargo 镜像，见上文
#   mkdir -p builddir/cargo && 写入上面的 config.toml

# 2. 编译（loong64 上约 7~8 分钟）
ninja -C builddir

# 3. 打包为 .deb
./build-deb.sh
```

产物：`resources_1.6.0_loong64.deb`（位于仓库根目录）。

### 安装 / 卸载

```bash
sudo dpkg -i resources_1.6.0_loong64.deb
sudo dpkg -r  resources          # 卸载
```

---

## 已知限制

- **CPU 频率为标称值**：龙芯 `loongson3_cpufreq` 驱动虽加载，但不暴露标准
  `scaling_cur_freq` sysfs 节点；本移植从 `/proc/cpuinfo` 的 `CPU MHz` 读取，
  多为恒定标称值（本机 2300 MHz）。若日后 cpufreq sysfs 出现，主路径仍优先用它。
- **`lvz` 仅代表 CPU 标志层支持虚拟化**：要真正跑虚拟机还需内核 `kvm` 模块与
  QEMU 在 loong64 上的支持，本仓库不涉及。
- GPU（amdgpu）的温度/频率走标准 hwmon，原版即可正常读取，未改动。

---

## 上游

- 上游仓库：<https://github.com/nokyan/resources>
- GNOME Incubator：<https://gitlab.gnome.org/GNOME/Incubator/resources>
- 本移植构建：<https://github.com/acdcGGE/resources>
