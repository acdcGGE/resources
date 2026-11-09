use anyhow::{Context, Result, anyhow, bail};
use glob::glob;
use lazy_regex::{Lazy, Regex, lazy_regex};
use log::{debug, trace, warn};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use crate::utils::read_parsed;

const PROC_STAT: &str = "/proc/stat";

const CORE_TEMP_LABEL: &str = "core";

const CPU_TEMP_LABEL: &str = "cpu";

const KNOWN_HWMONS: &[&str] = &[
    "zenpower",
    "coretemp",
    "k10temp",
    "ibmpowernv",
    "asusec",
    "cpu_hwmon",
    "loongson3",
];

const KNOWN_THERMAL_ZONES: &[&str] = &["cpu-thermal", "x86_pkg_temp", "acpitz"];

// Hwmons of GPUs and peripherals must not be mistaken for the CPU sensor by the
// generic fallback scan.
const NON_CPU_HWMONS: &[&str] = &[
    "amdgpu", "radeon", "nouveau", "nvidia", "iwlwifi", "ath", "mt76", "nvme", "rtl",
];

static RE_LSCPU_MODEL_NAME: Lazy<Regex> = lazy_regex!(r"Model name:\s*(.*)");

static RE_LSCPU_ARCHITECTURE: Lazy<Regex> = lazy_regex!(r"Architecture:\s*(.*)");

static RE_LSCPU_CPUS: Lazy<Regex> = lazy_regex!(r"CPU\(s\):\s*(.*)");

static RE_LSCPU_SOCKETS: Lazy<Regex> = lazy_regex!(r"Socket\(s\):\s*(.*)");

static RE_LSCPU_CORES: Lazy<Regex> = lazy_regex!(r"Core\(s\) per socket:\s*(.*)");

static RE_LSCPU_VIRTUALIZATION: Lazy<Regex> = lazy_regex!(r"Virtualization:\s*(.*)");

static RE_LSCPU_MAX_MHZ: Lazy<Regex> = lazy_regex!(r"CPU max MHz:\s*(.*)");

static RE_LSCPU_FLAGS: Lazy<Regex> = lazy_regex!(r"Flags:\s*(.*)");

static RE_PROC_STAT: Lazy<Regex> = lazy_regex!(
    r"cpu\d+ *(?P<user>\d*) *(?P<nice>\d*) *(?P<system>\d*) *(?P<idle>\d*) *(?P<iowait>\d*) *(?P<irq>\d*) *(?P<softirq>\d*) *(?P<steal>\d*) *(?P<guest>\d*) *(?P<guest_nice>\d*)"
);

static CPU_TEMPERATURE_PATH: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    search_for_hwmons(KNOWN_HWMONS, CORE_TEMP_LABEL)
        .or_else(|| search_for_thermal_zones(KNOWN_THERMAL_ZONES))
        .or_else(|| search_for_hwmons(KNOWN_HWMONS, CPU_TEMP_LABEL))
        .or_else(search_for_any_cpu_hwmon)
        .or_else(|| {
            warn!("No CPU temperature sensor found!");
            None
        })
});

fn search_for_cpu_temp<S: AsRef<str>>(base_path: S, temp_label: &str) -> Option<PathBuf> {
    let base = base_path.as_ref();
    let temp_label = temp_label.to_lowercase();

    // First, try to find a sensor labeled as a CPU core (e.g. ibmpowernv
    // exposes "Chip N Core M" alongside DIMM and VRM sensors)
    for label_path in glob(&format!("{base}/temp*_label")).unwrap().flatten() {
        if let Ok(label) = read_parsed::<String>(&label_path)
            && label.to_lowercase().contains(&temp_label)
        {
            let input_path = label_path.with_file_name(
                label_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .replace("_label", "_input"),
            );
            if input_path.exists() {
                trace!("Preferring labeled CPU core sensor: {input_path:?} ({label})");
                return Some(input_path);
            }
        }
    }

    // Fall back to the first available temp input
    glob(&format!("{base}/temp*_input"))
        .unwrap()
        .flatten()
        .next()
}

fn search_for_hwmons(names: &[&'static str], temp_label: &str) -> Option<PathBuf> {
    let mut hwmon_map = HashMap::new();

    trace!("Collecting hwmons for CPU temperature…");

    for path in (glob("/sys/class/hwmon/hwmon*").unwrap()).flatten() {
        trace!("Found hwmon {path:?}");
        if let Ok(read_name) = read_parsed::<String>(path.join("name")) {
            let read_name = read_name.to_string();
            trace!("{path:?} is a hwmon for {read_name}");
            if let Some(first_temp) = search_for_cpu_temp(path.to_string_lossy(), temp_label) {
                hwmon_map.insert(read_name, first_temp);
            }
        }
    }

    for name in names {
        if let Some(hwmon) = hwmon_map.remove(*name) {
            debug!("CPU temperature sensor located at {hwmon:?} ({name}, type: hwmon)");
            return Some(hwmon);
        }
    }

    trace!("No hwmon CPU temperature sensor found");
    None
}

fn search_for_thermal_zones(types: &[&'static str]) -> Option<PathBuf> {
    let mut thermal_zone_map = HashMap::new();

    trace!("Collecting thermal zones for CPU temperature…");

    for path in (glob("/sys/class/thermal/thermal_zone*").unwrap()).flatten() {
        trace!("Found thermal zone {path:?}");
        if let Ok(read_type) = read_parsed::<String>(path.join("type")) {
            let read_type = read_type.trim_end().to_string();
            trace!("{path:?} is a thermal zone for {read_type}");
            thermal_zone_map.insert(read_type, path.join("temp"));
        }
    }

    for r#type in types {
        if let Some(hwmon) = thermal_zone_map.remove(*r#type) {
            debug!("CPU temperature sensor located at {hwmon:?} ({type}, type: thermal zone)");
            return Some(hwmon);
        }
    }

    trace!("No thermal zone CPU temperature sensor found");
    None
}

// Loongson boards expose the CPU sensor under hwmon names absent from
// KNOWN_HWMONS, so accept any non-peripheral hwmon as a last resort.
fn search_for_any_cpu_hwmon() -> Option<PathBuf> {
    for path in glob("/sys/class/hwmon/hwmon*").unwrap().flatten() {
        let Ok(name) = read_parsed::<String>(path.join("name")) else {
            continue;
        };
        if NON_CPU_HWMONS.iter().any(|non_cpu| name.starts_with(non_cpu)) {
            continue;
        }
        if let Some(temp) = search_for_cpu_temp(path.to_string_lossy(), CPU_TEMP_LABEL) {
            debug!("CPU temperature sensor located at {temp:?} ({name}, type: hwmon fallback)");
            return Some(temp);
        }
    }

    trace!("No fallback hwmon CPU temperature sensor found");
    None
}

#[derive(Debug)]
pub struct CpuData {
    pub new_thread_usages: Vec<Result<(u64, u64)>>,
    pub temperature: Result<f32, anyhow::Error>,
    pub frequencies: Vec<Option<u64>>,
}

impl CpuData {
    pub fn new(logical_cpus: usize) -> Self {
        trace!("Gathering CPU data…");
        let new_thread_usages = get_cpu_usage();

        let temperature = get_temperature();

        let mut frequencies = Vec::with_capacity(logical_cpus);

        for i in 0..logical_cpus {
            let freq = get_cpu_freq(i);
            frequencies.push(freq.ok());
        }

        let cpu_data = Self {
            new_thread_usages,
            temperature,
            frequencies,
        };

        trace!("Gathered CPU data: {cpu_data:?}");

        cpu_data
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuInfo {
    pub model_name: Option<String>,
    pub architecture: Option<String>,
    pub logical_cpus: Option<usize>,
    pub physical_cpus: Option<usize>,
    pub sockets: Option<usize>,
    pub virtualization: Option<String>,
    pub max_speed: Option<f64>,
}

impl CpuInfo {
    fn parse_lscpu<S: AsRef<str>>(lscpu_output: S) -> Self {
        let lscpu_output = lscpu_output.as_ref();

        let model_name = RE_LSCPU_MODEL_NAME
            .captures(lscpu_output)
            .and_then(|captures| {
                captures
                    .get(1)
                    .map(|capture| Self::trade_mark_symbols(capture.as_str()))
            });

        let architecture = RE_LSCPU_ARCHITECTURE
            .captures(lscpu_output)
            .and_then(|captures| captures.get(1).map(|capture| capture.as_str().into()));

        let sockets = RE_LSCPU_SOCKETS
            .captures(lscpu_output)
            .and_then(|captures| {
                captures
                    .get(1)
                    .and_then(|capture| capture.as_str().parse().ok())
            });

        let logical_cpus = RE_LSCPU_CPUS.captures(lscpu_output).and_then(|captures| {
            captures
                .get(1)
                .and_then(|capture| capture.as_str().parse().ok())
        });

        let physical_cpus = RE_LSCPU_CORES.captures(lscpu_output).and_then(|captures| {
            captures
                .get(1)
                .and_then(|capture| capture.as_str().parse::<usize>().ok())
                .map(|int| int.saturating_mul(sockets.unwrap_or(1)))
        });

        let virtualization = RE_LSCPU_VIRTUALIZATION
            .captures(lscpu_output)
            .and_then(|captures| captures.get(1).map(|capture| capture.as_str().into()))
            .or_else(|| Self::virtualization_from_flags(lscpu_output));

        let max_speed = RE_LSCPU_MAX_MHZ
            .captures(lscpu_output)
            .and_then(|captures| {
                captures.get(1).and_then(|capture| {
                    capture
                        .as_str()
                        .parse::<f64>()
                        .ok()
                        .map(|float| float * 1_000_000.0)
                })
            });

        Self {
            model_name,
            architecture,
            logical_cpus,
            physical_cpus,
            sockets,
            virtualization,
            max_speed,
        }
    }

    // lscpu emits no `Virtualization:` field on LoongArch; the capability is
    // only visible as a CPU flag there.
    fn virtualization_from_flags(lscpu_output: &str) -> Option<String> {
        let flags = RE_LSCPU_FLAGS.captures(lscpu_output)?.get(1)?.as_str();
        let flag = ["lvz", "vmx", "svm"]
            .into_iter()
            .find(|candidate| flags.split_whitespace().any(|word| word == *candidate))?;

        Some(
            match flag {
                "lvz" => "Loongson Virtualization (LVZ)",
                "vmx" => "VT-x",
                _ => "AMD-V",
            }
            .into(),
        )
    }

    fn trade_mark_symbols<S: AsRef<str>>(s: S) -> String {
        s.as_ref()
            .replace("(R)", "®")
            .replace("(tm)", "™")
            .replace("(TM)", "™")
    }

    /// Returns a `CPUInfo` struct populated with values gathered from `lscpu`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the are problems during reading or parsing
    /// of the `lscpu` command
    pub fn get() -> Result<Self> {
        let lscpu_output = String::from_utf8(
            std::process::Command::new("lscpu")
                .env("LC_ALL", "C")
                .output()
                .context("unable to run lscpu, is util-linux installed?")?
                .stdout,
        )
        .context("unable to parse lscpu output to UTF-8")?;

        let mut info = Self::parse_lscpu(lscpu_output);
        if info.max_speed.is_none() {
            info.max_speed = get_cpu_freq_from_proc(0).ok().map(|hz| hz as f64);
        }
        Ok(info)
    }
}

/// Returns the frequency of the given CPU `core`
///
/// # Errors
///
/// Will return `Err` if the are problems during reading or parsing
/// of the corresponding file in sysfs
pub fn get_cpu_freq(core: usize) -> Result<u64> {
    trace!("Finding CPU frequency for core {core}…");

    read_parsed::<u64>(format!(
        "/sys/devices/system/cpu/cpu{core}/cpufreq/cpuinfo_avg_freq"
    ))
    .or_else(|_| {
        read_parsed::<u64>(format!(
            "/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_cur_freq"
        ))
    })
    .map(|x| x * 1000)
    .or_else(|_| get_cpu_freq_from_proc(core))
    .inspect(|freq| trace!("Frequency of core {core}: {freq} Hz"))
}

// Loongson's cpufreq driver may be loaded without binding, leaving no cpufreq
// sysfs; /proc/cpuinfo then is the only source of the nominal clock.
fn get_cpu_freq_from_proc(core: usize) -> Result<u64> {
    let cpuinfo =
        read_parsed::<String>("/proc/cpuinfo").context("unable to read /proc/cpuinfo")?;
    parse_proc_cpuinfo_freq(&cpuinfo, core)
}

fn parse_proc_cpuinfo_freq(cpuinfo: &str, core: usize) -> Result<u64> {
    for block in cpuinfo.split("\n\n") {
        let mut proc_id = None;
        let mut mhz = None;

        for line in block.lines() {
            let line = line.trim_start();
            if let Some(rest) = line.strip_prefix("processor") {
                proc_id = rest.split(':').nth(1).and_then(|v| v.trim().parse().ok());
            } else if let Some(rest) = line.strip_prefix("CPU MHz") {
                mhz = rest
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().parse::<f64>().ok());
            }
        }

        if proc_id == Some(core)
            && let Some(mhz) = mhz
        {
            return Ok((mhz * 1_000_000.0).round() as u64);
        }
    }

    bail!("no CPU MHz field for core {core} in /proc/cpuinfo")
}

fn parse_proc_stat_line<S: AsRef<str>>(line: S) -> Result<(u64, u64)> {
    let captures = RE_PROC_STAT
        .captures(line.as_ref())
        .context("using regex to parse /proc/stat failed")?;

    let idle = captures
        .name("idle")
        .and_then(|x| x.as_str().parse::<u64>().ok())
        .context("unable to get idle time")?;

    let iowait = captures
        .name("iowait")
        .and_then(|x| x.as_str().parse::<u64>().ok())
        .context("unable to get idle time")?;

    let total = captures
        .iter()
        .skip(1)
        .flat_map(|cap| {
            cap.and_then(|x| x.as_str().parse::<u64>().ok())
                .ok_or_else(|| anyhow!("unable to sum CPU times from /proc/stat"))
        })
        .sum();

    Ok((idle.saturating_add(iowait), total))
}

fn parse_proc_stat<S: AsRef<str>>(stat: S) -> Vec<Result<(u64, u64)>> {
    trace!("Parsing {PROC_STAT}…");

    stat.as_ref()
        .lines()
        .skip(1)
        .filter(|line| line.starts_with("cpu"))
        .map(parse_proc_stat_line)
        .collect()
}

/// Returns the CPU usage of either all cores combined (if supplied argument is `None`),
/// or of a specific thread (taken from the supplied argument starting at 0)
/// Please keep in mind that this is the total CPU time since boot, you have to do delta
/// calculations yourself. The tuple's layout is: `(idle_time, total_time)`
///
/// # Errors
///
/// Will return `Err` if the are problems during reading or parsing
/// of /proc/stat
pub fn get_cpu_usage() -> Vec<Result<(u64, u64)>> {
    trace!("Reading {PROC_STAT}…");

    let raw = read_parsed::<String>("/proc/stat")
        .context("unable to read /proc/stat")
        .unwrap_or_default();

    parse_proc_stat(raw)
}

/// Returns the CPU temperature.
///
/// # Errors
///
/// Will return `Err` if there was no way to read the CPU temperature.
pub fn get_temperature() -> Result<f32> {
    if let Some(path) = CPU_TEMPERATURE_PATH.as_ref() {
        read_sysfs_thermal(path)
    } else {
        bail!("no CPU temperature sensor found")
    }
}

fn read_sysfs_thermal<P: AsRef<Path>>(path: P) -> Result<f32> {
    let path = path.as_ref();

    read_parsed::<f32>(path)
        .map(|t| t / 1000f32)
        .with_context(|| format!("unable to read {}", path.display()))
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use crate::devices::cpu::{CpuInfo, parse_proc_cpuinfo_freq};

    const LSCPU_OUTPUT: &str = concat!(
        "Architecture:             x86_64\n",
        "  CPU op-mode(s):         32-bit, 64-bit\n",
        "  Address sizes:          48 bits physical, 48 bits virtual\n",
        "  Byte Order:             Little Endian\n",
        "CPU(s):                   16\n",
        "  On-line CPU(s) list:    0-7\n",
        "Vendor ID:                UnauthenticIngenuineManufacturer\n",
        "  Model name:             UIM(R) Abacus(tm) 10\n",
        "    CPU family:           1\n",
        "    Model:                2\n",
        "    Thread(s) per core:   2\n",
        "    Core(s) per socket:   4\n",
        "    Socket(s):            2\n",
        "    Stepping:             2\n",
        "    Frequency boost:      enabled\n",
        "    CPU(s) scaling MHz:   100%\n",
        "    CPU max MHz:          3.0000\n",
        "    CPU min MHz:          2.0000\n",
        "    BogoMIPS:             0.0\n",
        "Virtualization features:  \n",
        "  Virtualization:         Abacus-V\n",
        "Caches (sum of all):      \n",
        "  L1d:                    256 KiB (8 instances)\n",
        "  L1i:                    256 KiB (8 instances)\n",
        "  L2:                     4 MiB (8 instances)\n",
        "  L3:                     32 MiB (1 instance)\n",
        "NUMA:                     \n",
        "  NUMA node(s):           1\n",
        "  NUMA node0 CPU(s):      0-15\n",
    );

    #[test]
    fn lscpu_complex() {
        let parsed = CpuInfo::parse_lscpu(LSCPU_OUTPUT);

        let expected = CpuInfo {
            model_name: Some("UIM® Abacus™ 10".into()),
            architecture: Some("x86_64".into()),
            logical_cpus: Some(16),
            physical_cpus: Some(8),
            sockets: Some(2),
            virtualization: Some("Abacus-V".into()),
            max_speed: Some(3000000.0),
        };

        assert_eq!(parsed, expected)
    }

    const LSCPU_LOONGARCH: &str = concat!(
        "Architecture:                            loongarch64\n",
        "CPU(s):                                  4\n",
        "Model name:                              Loongson-3A5000\n",
        "Core(s) per socket:                      4\n",
        "Socket(s):                               1\n",
        "Flags:                                   cpucfg lam ual fpu lsx lasx crc32 lspw lvz lbt_x86 lbt_arm lbt_mips\n",
    );

    #[test]
    fn lscpu_loongarch() {
        let parsed = CpuInfo::parse_lscpu(LSCPU_LOONGARCH);

        assert_eq!(parsed.model_name.as_deref(), Some("Loongson-3A5000"));
        assert_eq!(parsed.architecture.as_deref(), Some("loongarch64"));
        assert_eq!(parsed.logical_cpus, Some(4));
        assert_eq!(parsed.physical_cpus, Some(4));
        assert_eq!(parsed.sockets, Some(1));
        assert_eq!(
            parsed.virtualization.as_deref(),
            Some("Loongson Virtualization (LVZ)")
        );
        assert_eq!(parsed.max_speed, None);
    }

    const PROC_CPUINFO_LOONGARCH: &str = concat!(
        "system type\t\t: generic-loongson-machine\n\n",
        "processor\t\t: 0\n",
        "CPU MHz\t\t\t: 2300.00\n\n",
        "processor\t\t: 1\n",
        "CPU MHz\t\t\t: 2300.00\n",
    );

    #[test]
    fn proc_cpuinfo_freq_loongarch() {
        assert_eq!(
            parse_proc_cpuinfo_freq(PROC_CPUINFO_LOONGARCH, 0).unwrap(),
            2_300_000_000
        );
        assert_eq!(
            parse_proc_cpuinfo_freq(PROC_CPUINFO_LOONGARCH, 1).unwrap(),
            2_300_000_000
        );
        assert!(parse_proc_cpuinfo_freq(PROC_CPUINFO_LOONGARCH, 2).is_err());
    }
}
